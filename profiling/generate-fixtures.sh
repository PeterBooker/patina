#!/bin/bash
set -euo pipefail

# Generate test fixtures from the WordPress instance in the profiling stack.
# Must be run after setup-wordpress.sh.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
COMPOSE="docker compose -f $SCRIPT_DIR/docker-compose.yml"

echo "=== Generating fixtures from WordPress ==="

$COMPOSE exec php-fpm php /app/php/fixture-generator/generate.php \
    --all \
    --output=/app/fixtures/

echo ""
echo "Fixtures written to fixtures/"
ls -la "$SCRIPT_DIR/../fixtures/"*.json 2>/dev/null || echo "(no .json files found — check Docker volume mounts)"
