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
#   ITERATIONS=100    samples per scenario per config (default 100)
#   WARMUP=5          k6 warmup iterations, dropped from metrics
#   CONFIGS="..."     comma-separated config names to run (default: all 5)
#   RUN_DIR=...       output directory (default: /tmp/patina-bench/<ts>)
#   PROFILE=1         capture one SPX profile per scenario per config
#                     (Phase 5 — deliverables in $RUN_DIR/<config>/spx/)
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
PROF="docker compose -f ${REPO_DIR}/profiling/docker-compose.yml"
DEV="docker compose -f ${REPO_DIR}/docker/docker-compose.dev.yml run --rm dev"

ITERATIONS="${ITERATIONS:-100}"
WARMUP="${WARMUP:-5}"
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
RUN_DIR="${RUN_DIR:-/tmp/patina-bench/${TIMESTAMP}}"
CONFIGS_LIST="${CONFIGS:-stock,esc_only,kses_only,parse_blocks_only,full_patina}"
PROFILE="${PROFILE:-0}"
# SPX's HTTP key must match spx.http_key in profiling/conf/spx.ini.
SPX_HTTP_KEY="dev"

mkdir -p "${RUN_DIR}"
echo "=== Patina bench-runner ==="
echo "  run dir:    ${RUN_DIR}"
echo "  iterations: ${ITERATIONS} (+ ${WARMUP} warmup, dropped)"
echo "  configs:    ${CONFIGS_LIST}"
echo "  profiling:  $([ "${PROFILE}" = "1" ] && echo "SPX enabled (1 profile/scenario/config)" || echo "off")"
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
  "warmup_per_scenario": ${WARMUP},
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
for config in "${CONFIGS[@]}"; do
    flags="$(config_flags "$config")"
    if [ "$flags" = "UNKNOWN" ]; then
        echo "!! unknown config '${config}' — skipping"
        continue
    fi
    active="$(config_active_overrides "$config")"

    echo ""
    echo "=== config: ${config} ==="
    [ -n "$flags" ] && echo "  flags: ${flags}" || echo "  flags: (none — full patina)"
    echo "  active: ${active:-(none)}"

    write_bench_mu_plugin "$flags"
    $PROF restart php-fpm > /dev/null
    sleep 2
    warm_cache

    CONFIG_DIR="${RUN_DIR}/${config}"
    mkdir -p "${CONFIG_DIR}"

    # Clear any stale SPX profiles before this config runs so we only
    # collect the ones produced during the k6 invocation below. SPX writes
    # its profiles under /tmp/spx inside the php-fpm container.
    if [ "${PROFILE}" = "1" ]; then
        $PROF exec -T php-fpm bash -c 'rm -rf /tmp/spx && mkdir -p /tmp/spx'
    fi

    K6_ENV=(-e BASE_URL=http://nginx -e "ITERATIONS=${ITERATIONS}" -e "WARMUP=${WARMUP}")
    if [ "${PROFILE}" = "1" ]; then
        K6_ENV+=(-e "SPX_KEY=${SPX_HTTP_KEY}")
    fi

    $PROF exec -T \
        "${K6_ENV[@]}" \
        php-fpm k6 run \
            --quiet \
            --out json=/tmp/k6-output.json \
            /app/profiling/k6-workloads.js
    $PROF exec -T php-fpm cat /tmp/k6-output.json > "${CONFIG_DIR}/k6-output.json"

    # Pull SPX profiles out of the container if profiling was enabled.
    # SPX writes one directory per profile under /tmp/spx (name includes
    # a timestamp and the SPX_REPORT_KEY we set from the k6 cookie).
    if [ "${PROFILE}" = "1" ]; then
        mkdir -p "${CONFIG_DIR}/spx"
        $PROF exec -T php-fpm bash -c '\
            if [ -d /tmp/spx ] && [ -n "$(ls -A /tmp/spx 2>/dev/null)" ]; then \
                tar -C /tmp/spx -cf - . ; \
            fi' | tar -C "${CONFIG_DIR}/spx" -xf - 2>/dev/null || true
        profile_count=$(find "${CONFIG_DIR}/spx" -mindepth 1 -maxdepth 2 -type f 2>/dev/null | wc -l | tr -d ' ')
        echo "  spx: captured ${profile_count} profile file(s) → ${CONFIG_DIR}/spx/"
    fi

    # Build env map JSON the aggregator records in the summary.
    env_json="{}"
    if [ -n "$flags" ]; then
        env_json=$(python3 -c "import json,sys; print(json.dumps({k:'1' for k in sys.argv[1].split(',')}))" "$flags")
    fi

    python3 "${REPO_DIR}/scripts/bench-aggregate.py" \
        --k6-json "${CONFIG_DIR}/k6-output.json" \
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
        --iterations "${ITERATIONS}"
done

# ------------------------------------------------------------------
# Cleanup: leave the stack in a full-patina state so interactive use
# after the run gets the default behavior.
# ------------------------------------------------------------------
remove_bench_mu_plugin
$PROF restart php-fpm > /dev/null

echo ""
echo "=== bench-runner complete ==="
echo "  results: ${RUN_DIR}"
echo "  compare: scripts/bench-compare.py ${RUN_DIR}"
