#!/bin/bash
# Open the php-spx web UI pointed at the profiles produced by the last
# `PROFILE=1 make bench-full` run.
#
# Two modes:
#
#   ./scripts/spx-ui.sh
#       Boots the profiling stack (if not running), drops all collected
#       SPX profiles from the given run directory back into /tmp/spx
#       inside the php-fpm container, and prints the URL for the SPX UI.
#
#   ./scripts/spx-ui.sh <run-dir>/<config>
#       Same, but restores profiles from a specific run+config directory
#       so you can browse an older bench run's flame graphs.
#
# The SPX UI runs inside the php-fpm container on the same port as
# WordPress — it's gated behind a magic query string + cookie that
# php-spx intercepts before php-fpm serves a request.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
PROF="docker compose -f ${REPO_DIR}/profiling/docker-compose.yml"
TARGET="${1:-}"

$PROF up -d > /dev/null

if [ -n "$TARGET" ]; then
    if [ ! -d "$TARGET" ]; then
        echo "error: $TARGET is not a directory" >&2
        exit 1
    fi
    SPX_SRC="${TARGET%/}/spx"
    if [ ! -d "$SPX_SRC" ]; then
        # Maybe they pointed at the spx dir directly.
        SPX_SRC="$TARGET"
    fi
    echo "=== Restoring profiles from ${SPX_SRC} to /tmp/spx ==="
    $PROF exec -T php-fpm bash -c 'rm -rf /tmp/spx && mkdir -p /tmp/spx'
    tar -C "$SPX_SRC" -cf - . | $PROF exec -T php-fpm tar -C /tmp/spx -xf -
fi

PROFILE_COUNT=$($PROF exec -T php-fpm find /tmp/spx -mindepth 1 -maxdepth 2 -type f 2>/dev/null | wc -l | tr -d ' ')

cat <<EOF
=== SPX UI ready ===
  profiles on disk: ${PROFILE_COUNT}
  URL:              http://localhost:8080/?SPX_UI_URI=/&SPX_KEY=dev

Open that URL in a browser. The SPX UI lists every profile under /tmp/spx
and lets you click into flame graphs, call trees, and timelines.

To reset before the next bench run:
  $PROF exec php-fpm rm -rf /tmp/spx
EOF
