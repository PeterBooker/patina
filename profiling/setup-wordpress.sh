#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
COMPOSE="docker compose -f $SCRIPT_DIR/docker-compose.yml"

# WP-CLI needs extra memory for extraction
WP="php -d memory_limit=512M /usr/local/bin/wp"

echo "=== Waiting for services ==="
$COMPOSE up -d
sleep 5

echo "=== Downloading WordPress ==="
$COMPOSE exec php-fpm $WP core download --path=/var/www/html

echo "=== Creating wp-config.php ==="
$COMPOSE exec php-fpm $WP config create \
    --dbname=wordpress \
    --dbuser=wordpress \
    --dbpass=wordpress \
    --dbhost=mariadb \
    --path=/var/www/html

echo "=== Installing WordPress ==="
$COMPOSE exec php-fpm $WP core install \
    --url=http://localhost:8080 \
    --title="Patina Profiling" \
    --admin_user=admin \
    --admin_password=admin \
    --admin_email=admin@example.com \
    --path=/var/www/html \
    --skip-email

echo "=== Configuring WordPress ==="
$COMPOSE exec php-fpm $WP rewrite structure '/%postname%/' --path=/var/www/html
$COMPOSE exec php-fpm $WP rewrite flush --path=/var/www/html

echo "=== Seeding benchmark content corpus ==="
$COMPOSE exec php-fpm bash /app/profiling/seed-benchmark-content.sh

echo ""
echo "=== Setup complete ==="
echo "WordPress:  http://localhost:8080/"
echo "Admin:      http://localhost:8080/wp-admin/ (admin/admin)"
echo "SPX UI:     http://localhost:8080/?SPX_UI_URI=/&SPX_KEY=dev"
