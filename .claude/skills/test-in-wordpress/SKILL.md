---
name: test-in-wordpress
description: Build the extension for PHP 8.3, install in the WordPress profiling stack, and verify it works end-to-end.
disable-model-invocation: true
---

# Test in WordPress

## 1. Build for PHP 8.3

```bash
docker build --build-arg PHP_VERSION=8.3 -f docker/Dockerfile.build --target builder -t patina-build-8.3 .
docker run --rm patina-build-8.3 cat /src/target/release/libpatina.so > /tmp/patina-8.3.so
```

## 2. Start WordPress and install extension

```bash
cd profiling && docker compose up -d
EXT_DIR=$(docker compose exec php-fpm php -r "echo ini_get('extension_dir');")
docker compose cp /tmp/patina-8.3.so php-fpm:${EXT_DIR}/patina.so
docker compose exec php-fpm bash -c 'echo "extension=patina.so" > /usr/local/etc/php/conf.d/patina.ini'
docker compose exec php-fpm php -r "echo patina_version();"
```

## 3. Install mu-plugin and restart

```bash
docker compose exec php-fpm mkdir -p /var/www/html/wp-content/mu-plugins/
docker compose cp php/bridge/patina-bridge.php php-fpm:/var/www/html/wp-content/mu-plugins/
docker compose restart php-fpm && sleep 2
```

## 4. Verify

```bash
# Pages render without errors
curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/
curl -s http://localhost:8080/ | grep -c "Fatal error\|Warning:" || echo "0 errors"

# Test activation in WordPress context
docker compose exec php-fpm php -d memory_limit=512M -r "
require '/var/www/html/wp-load.php';
echo 'Status: ' . implode(', ', patina_status()) . PHP_EOL;
"
```

## 5. Clean up

```bash
cd profiling && docker compose down
```
