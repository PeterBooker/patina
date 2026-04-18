#!/bin/bash
# Patina bench runner — Phase 4 of docs/BENCHMARK_PLAN.md.
#
# Drives the profiling stack through a sequence of named configurations,
# each of which disables a different subset of overrides via the PHP
# constants the bridge reads at boot. For each config we:
#
#   1. Drop a mu-plugin file that defines PATINA_DISABLE_* constants
#      BEFORE patina-bridge.php loads (alphabetical mu-plugin load order
#      puts `000-patina-bench-config.php` first).
#   2. Restart php-fpm so the new constants + a fresh opcache take effect.
#   3. Warm the cache with a handful of curl hits so k6 doesn't measure
#      opcache cold starts.
#   4. Run k6 against every scenario, writing raw per-sample JSON.
#   5. Aggregate the raw JSON into the summary schema from the plan.
#
# The mu-plugin file is removed at end-of-run so the stack is left in a
# clean "full patina" state for interactive use afterwards.
#
# Env overrides:
#   ITERATIONS=100    measured samples per scenario per config (default 100)
#   CHUNKS=5          split ITERATIONS into this many interleaved chunks,
#                     cycling configs per chunk so stock and candidate share
#                     the same time slices. Set CHUNKS=1 for the legacy
#                     batched-per-config layout.
#   WARMUP=3          k6 warmup iterations per chunk, dropped from metrics.
#                     Paid once per chunk to reheat opcache after restart.
#   CPUSET=2,3        docker cpuset pinned onto php-fpm for the duration of
#                     the run. Empty string disables pinning.
#   CONFIGS="..."     comma-separated config names to run (default: all 5)
#   RUN_DIR=...       output directory (default: /tmp/patina-bench/<ts>)
#   PROFILE=1         capture one SPX profile per scenario per config
#                     (Phase 5 — deliverables in $RUN_DIR/<config>/spx/)
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
PROF="docker compose -f ${REPO_DIR}/profiling/docker-compose.yml"
DEV="docker compose -f ${REPO_DIR}/docker/docker-compose.dev.yml run --rm dev"

ITERATIONS="${ITERATIONS:-100}"
CHUNKS="${CHUNKS:-5}"
WARMUP="${WARMUP:-3}"
CPUSET="${CPUSET-2,3}"
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
RUN_DIR="${RUN_DIR:-/tmp/patina-bench/${TIMESTAMP}}"
CONFIGS_LIST="${CONFIGS:-stock,esc_only,kses_only,parse_blocks_only,full_patina}"
PROFILE="${PROFILE:-0}"
# SPX's HTTP key must match spx.http_key in profiling/conf/spx.ini.
SPX_HTTP_KEY="dev"

if [ "${CHUNKS}" -lt 1 ]; then
    echo "error: CHUNKS must be >= 1" >&2
    exit 2
fi
CHUNK_ITERS=$(( ITERATIONS / CHUNKS ))
if [ "$CHUNK_ITERS" -lt 1 ]; then
    echo "error: ITERATIONS (${ITERATIONS}) must be >= CHUNKS (${CHUNKS})" >&2
    exit 2
fi
# Round ITERATIONS up to a multiple of CHUNKS so per-chunk sizes are equal
# and paired stats in bench-compare can assume matching indices per chunk.
ITERATIONS=$(( CHUNK_ITERS * CHUNKS ))

mkdir -p "${RUN_DIR}"
echo "=== Patina bench-runner ==="
echo "  run dir:    ${RUN_DIR}"
echo "  iterations: ${ITERATIONS} measured (${CHUNK_ITERS}/chunk × ${CHUNKS} chunks, + ${WARMUP} warmup/chunk)"
echo "  configs:    ${CONFIGS_LIST}"
echo "  cpuset:     ${CPUSET:-(unpinned)}"
echo "  profiling:  $([ "${PROFILE}" = "1" ] && echo "SPX enabled (1 profile/scenario/config)" || echo "off")"

# Noise sanity check — cpufreq turbo boost is the single biggest source of
# per-sample jitter on a Ryzen. Don't fail the run, just warn; the user may
# be running the bench on a laptop or shared box where toggling boost is
# impractical.
if [ -r /sys/devices/system/cpu/cpufreq/boost ]; then
    if [ "$(cat /sys/devices/system/cpu/cpufreq/boost 2>/dev/null)" = "1" ]; then
        echo ""
        echo "  WARN: /sys/devices/system/cpu/cpufreq/boost = 1"
        echo "        cpufreq boost adds ~5–10% sample jitter. To disable:"
        echo "            echo 0 | sudo tee /sys/devices/system/cpu/cpufreq/boost"
    fi
fi
echo ""

# ------------------------------------------------------------------
# Config table. Each entry is:
#   <name>|<comma-separated PATINA_DISABLE_* flags>|<comma-separated active overrides>
#
# "stock" uses the master kill switch so the whole bridge mu-plugin
# early-returns — no shims, no swaps, no patina overhead whatsoever.
# ------------------------------------------------------------------
config_flags() {
    case "$1" in
        stock)              echo "PATINA_DISABLE" ;;
        esc_only)           echo "PATINA_DISABLE_KSES,PATINA_DISABLE_PARSE_BLOCKS" ;;
        kses_only)          echo "PATINA_DISABLE_ESC,PATINA_DISABLE_PARSE_BLOCKS" ;;
        parse_blocks_only)  echo "PATINA_DISABLE_ESC,PATINA_DISABLE_KSES" ;;
        full_patina)        echo "" ;;
        *) echo "UNKNOWN" ;;
    esac
}

config_active_overrides() {
    case "$1" in
        stock)              echo "" ;;
        esc_only)           echo "esc_html,esc_attr" ;;
        kses_only)          echo "wp_kses" ;;
        parse_blocks_only)  echo "parse_blocks" ;;
        full_patina)        echo "esc_html,esc_attr,wp_kses,parse_blocks" ;;
        *) echo "" ;;
    esac
}

# ------------------------------------------------------------------
# One-time setup: build the .so, boot the stack, install patina, seed.
# ------------------------------------------------------------------
echo "=== Building extension (PHP 8.3) ==="
docker build \
    --build-arg PHP_VERSION=8.3 \
    -f "${REPO_DIR}/docker/Dockerfile.build" \
    --target builder \
    -t patina-build-8.3 "${REPO_DIR}" > /dev/null
docker run --rm patina-build-8.3 cat /src/target/release/libpatina.so \
    > /tmp/patina-8.3.so

echo "=== Ensuring profiling stack is up ==="
$PROF up -d

# Pin php-fpm to a fixed cpuset. With vus=1 in k6 we're measuring a single
# PHP worker at a time; pinning it away from noisy cores (kworker threads,
# kernel interrupts on core 0, the k6 process, etc.) noticeably lowers
# sample stddev on desktop-class hardware. Save the pre-bench value and
# restore it on exit so interactive use after the run isn't affected.
PHP_FPM_CID="$($PROF ps -q php-fpm | head -n1)"
ORIG_CPUSET=""
if [ -n "${PHP_FPM_CID}" ] && [ -n "${CPUSET}" ]; then
    ORIG_CPUSET="$(docker inspect --format '{{.HostConfig.CpusetCpus}}' "${PHP_FPM_CID}" 2>/dev/null || echo '')"
    if docker update --cpuset-cpus "${CPUSET}" "${PHP_FPM_CID}" > /dev/null 2>&1; then
        echo "  pinned php-fpm (${PHP_FPM_CID:0:12}) to cpuset ${CPUSET} (was '${ORIG_CPUSET}')"
    else
        echo "  WARN: failed to pin cpuset on php-fpm; continuing unpinned"
        ORIG_CPUSET=""
    fi
fi
restore_cpuset() {
    if [ -n "${PHP_FPM_CID}" ] && [ -n "${CPUSET}" ]; then
        docker update --cpuset-cpus "${ORIG_CPUSET}" "${PHP_FPM_CID}" > /dev/null 2>&1 || true
    fi
}
trap 'restore_cpuset' EXIT

EXT_DIR="$($PROF exec -T php-fpm php -r 'echo ini_get("extension_dir");')"
$PROF cp /tmp/patina-8.3.so "php-fpm:${EXT_DIR}/patina.so"
$PROF exec -T php-fpm bash -c \
    'echo "extension=patina.so" > /usr/local/etc/php/conf.d/patina.ini'
$PROF exec -T php-fpm bash -c \
    'mkdir -p /var/www/html/wp-content/mu-plugins && cp /app/php/bridge/patina-bridge.php /var/www/html/wp-content/mu-plugins/'

# Collect metadata once — it's identical across configs.
GIT_SHA="$(git -C "${REPO_DIR}" rev-parse --short HEAD 2>/dev/null || echo unknown)"
PATINA_VERSION="$($PROF exec -T php-fpm php -r 'echo patina_version();' 2>/dev/null | tr -d '\n\r' || echo unknown)"
PHP_VERSION="$($PROF exec -T php-fpm php -r 'echo PHP_VERSION;')"
WP_VERSION="$($PROF exec -T php-fpm php -d memory_limit=512M /usr/local/bin/wp --path=/var/www/html core version 2>/dev/null || echo unknown)"
HOST_NAME="$(hostname)"
CPU_NAME="$(grep -m1 '^model name' /proc/cpuinfo 2>/dev/null | sed 's/.*: //' || echo unknown)"

cat > "${RUN_DIR}/manifest.json" <<EOF
{
  "timestamp": "${TIMESTAMP}",
  "git_sha": "${GIT_SHA}",
  "patina_version": "${PATINA_VERSION}",
  "php_version": "${PHP_VERSION}",
  "wp_version": "${WP_VERSION}",
  "host": "${HOST_NAME}",
  "cpu": "${CPU_NAME}",
  "iterations_per_scenario": ${ITERATIONS},
  "chunks": ${CHUNKS},
  "chunk_iters": ${CHUNK_ITERS},
  "warmup_per_chunk": ${WARMUP},
  "cpuset": "${CPUSET}",
  "configs_run": "${CONFIGS_LIST}"
}
EOF

# ------------------------------------------------------------------
# Per-config loop
# ------------------------------------------------------------------
write_bench_mu_plugin() {
    local flags="$1"
    local php="<?php\n"
    if [ -n "$flags" ]; then
        IFS=',' read -ra FLAG_ARR <<< "$flags"
        for f in "${FLAG_ARR[@]}"; do
            php="${php}if (!defined('${f}')) define('${f}', true);\n"
        done
    fi
    $PROF exec -T php-fpm bash -c "printf '%b' \"${php}\" > /var/www/html/wp-content/mu-plugins/000-patina-bench-config.php"
}

remove_bench_mu_plugin() {
    $PROF exec -T php-fpm rm -f /var/www/html/wp-content/mu-plugins/000-patina-bench-config.php
}

warm_cache() {
    # Warm via nginx on the compose network — php-fpm's "localhost" has no
    # HTTP listener, requests must go through the nginx service. The Host
    # header forces WP's canonical-URL logic to accept the request instead
    # of 301-redirecting to the admin-configured siteurl.
    for path in / /a-short-block-post/ /a-long-block-post/ /a-classic-html-post/ /a-commented-post/ /category/announcements/ '/?s=lorem' /wp-json/wp/v2/posts; do
        $PROF exec -T php-fpm curl -s -H 'Host: localhost:8080' "http://nginx${path}" > /dev/null 2>&1 || true
    done
}

IFS=',' read -ra CONFIGS <<< "${CONFIGS_LIST}"

# Validate every config up-front so we don't discover a typo on chunk 3.
for config in "${CONFIGS[@]}"; do
    if [ "$(config_flags "$config")" = "UNKNOWN" ]; then
        echo "error: unknown config '${config}'" >&2
        exit 2
    fi
    mkdir -p "${RUN_DIR}/${config}"
    if [ "${PROFILE}" = "1" ]; then
        mkdir -p "${RUN_DIR}/${config}/spx"
    fi
done

# Chunked interleaving: for every chunk we cycle through the full config
# list, running CHUNK_ITERS measured samples per scenario per config. This
# keeps stock and each candidate on the same minute-scale time slice, which
# is what makes the paired-sample analysis in bench-compare actually work
# — any thermal/host drift between chunks is shared by every config and
# cancels out of the delta. Legacy behavior (all-stock-then-all-candidate)
# is CHUNKS=1.
for (( chunk=1; chunk<=CHUNKS; chunk++ )); do
    echo ""
    echo "=== chunk ${chunk}/${CHUNKS} ==="
    for config in "${CONFIGS[@]}"; do
        flags="$(config_flags "$config")"
        active="$(config_active_overrides "$config")"
        echo "  -- config: ${config}  (flags: ${flags:-none})"

        write_bench_mu_plugin "$flags"
        $PROF restart php-fpm > /dev/null
        sleep 1
        warm_cache

        CONFIG_DIR="${RUN_DIR}/${config}"

        # Clear any stale SPX profiles before the first chunk so the per-
        # scenario profiles in $CONFIG_DIR/spx/ only reflect this run. Only
        # chunk 1 profiles are captured — SPX adds ~10% overhead and we
        # want the rest of the samples clean.
        if [ "${PROFILE}" = "1" ] && [ "${chunk}" = "1" ]; then
            $PROF exec -T php-fpm bash -c 'rm -rf /tmp/spx && mkdir -p /tmp/spx && chmod 777 /tmp/spx'
        fi

        K6_ENV=(-e BASE_URL=http://nginx -e "ITERATIONS=${CHUNK_ITERS}" -e "WARMUP=${WARMUP}")
        if [ "${PROFILE}" = "1" ] && [ "${chunk}" = "1" ]; then
            K6_ENV+=(-e "SPX_KEY=${SPX_HTTP_KEY}")
        fi

        $PROF exec -T \
            "${K6_ENV[@]}" \
            php-fpm k6 run \
                --quiet \
                --out json=/tmp/k6-output.json \
                /app/profiling/k6-workloads.js
        printf -v chunk_file "%s/k6-chunk-%02d.json" "${CONFIG_DIR}" "${chunk}"
        $PROF exec -T php-fpm cat /tmp/k6-output.json > "${chunk_file}"

        if [ "${PROFILE}" = "1" ] && [ "${chunk}" = "1" ]; then
            $PROF exec -T php-fpm bash -c '\
                if [ -d /tmp/spx ] && [ -n "$(ls -A /tmp/spx 2>/dev/null)" ]; then \
                    tar -C /tmp/spx -cf - . ; \
                fi' | tar -C "${CONFIG_DIR}/spx" -xf - 2>/dev/null || true
            profile_count=$(find "${CONFIG_DIR}/spx" -mindepth 1 -maxdepth 2 -type f 2>/dev/null | wc -l | tr -d ' ')
            echo "     spx: captured ${profile_count} profile file(s)"
        fi
    done
done

echo ""
echo "=== aggregating ==="
for config in "${CONFIGS[@]}"; do
    flags="$(config_flags "$config")"
    active="$(config_active_overrides "$config")"
    CONFIG_DIR="${RUN_DIR}/${config}"

    env_json="{}"
    if [ -n "$flags" ]; then
        env_json=$(python3 -c "import json,sys; print(json.dumps({k:'1' for k in sys.argv[1].split(',')}))" "$flags")
    fi

    # Chunk files are in numerical order on disk, so glob expansion gives
    # the aggregator sample streams in chunk order. That's essential for
    # the paired analysis — index i across configs must map to the same
    # chunk.
    python3 "${REPO_DIR}/scripts/bench-aggregate.py" \
        --k6-json "${CONFIG_DIR}"/k6-chunk-*.json \
        --output "${CONFIG_DIR}/summary.json" \
        --config-name "${config}" \
        --config-env "${env_json}" \
        --active-overrides "${active}" \
        --git-sha "${GIT_SHA}" \
        --patina-version "${PATINA_VERSION}" \
        --php-version "${PHP_VERSION}" \
        --wp-version "${WP_VERSION}" \
        --host "${HOST_NAME}" \
        --cpu "${CPU_NAME}" \
        --timestamp "${TIMESTAMP}" \
        --iterations "${ITERATIONS}" \
        --chunks "${CHUNKS}"
done

# ------------------------------------------------------------------
# Cleanup: leave the stack in a full-patina state so interactive use
# after the run gets the default behavior.
# ------------------------------------------------------------------
remove_bench_mu_plugin
$PROF restart php-fpm > /dev/null
restore_cpuset
trap - EXIT

echo ""
echo "=== bench-runner complete ==="
echo "  results: ${RUN_DIR}"
echo "  compare: scripts/bench-compare.py ${RUN_DIR}"
