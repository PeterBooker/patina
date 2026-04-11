# Deployment Guide

## Supported Platforms

| PHP | x86_64 | aarch64 |
|---|---|---|
| 8.1 | Yes | Yes |
| 8.2 | Yes | Yes |
| 8.3 | Yes | Yes |
| 8.4 | Yes | Yes |

Linux only. macOS and Windows are not supported.

## Install from GitHub Releases

### Automatic

```bash
curl -sSL https://raw.githubusercontent.com/<org>/patina/main/install.sh | bash
```

The script auto-detects your PHP version and CPU architecture.

### Manual

```bash
# 1. Determine your PHP version and architecture
PHP_VERSION=$(php -r "echo PHP_MAJOR_VERSION.'.'.PHP_MINOR_VERSION;")
ARCH=$(uname -m)  # x86_64 or aarch64
echo "PHP $PHP_VERSION on $ARCH"

# 2. Download the correct binary
wget "https://github.com/<org>/patina/releases/latest/download/patina-php${PHP_VERSION}-linux-${ARCH}.so"

# 3. Verify checksum
wget "https://github.com/<org>/patina/releases/latest/download/SHA256SUMS"
sha256sum -c SHA256SUMS --ignore-missing

# 4. Install the extension
EXT_DIR=$(php -r "echo ini_get('extension_dir');")
sudo cp "patina-php${PHP_VERSION}-linux-${ARCH}.so" "$EXT_DIR/patina.so"

# 5. Enable the extension
INI_DIR=$(php --ini | grep 'Scan for' | cut -d: -f2 | tr -d ' ')
echo "extension=patina.so" | sudo tee "$INI_DIR/99-patina.ini"

# 6. Restart PHP-FPM
sudo systemctl restart "php${PHP_VERSION}-fpm"
```

## Verify Installation

```bash
# Check the extension is loaded
php -m | grep patina-ext

# Check registered functions
php -r "print_r(get_extension_funcs('patina-ext'));"

# Smoke test
php -r "echo patina_esc_html('<script>alert(1)</script>');"
# Expected: &lt;script&gt;alert(1)&lt;/script&gt;
```

## Build from Source

Prerequisites:
- Rust stable toolchain
- PHP development headers (`php-dev` / `php-devel`)
- `clang` and `libclang-dev`
- `pkg-config`

```bash
git clone https://github.com/<org>/patina.git
cd patina
cargo build --release -p patina-ext

# The extension is at:
ls -la target/release/libpatina.so

# Install it:
sudo cp target/release/libpatina.so $(php -r "echo ini_get('extension_dir');")/patina.so
echo "extension=patina.so" | sudo tee /etc/php/$(php -r "echo PHP_MAJOR_VERSION.'.'.PHP_MINOR_VERSION;")/mods-available/patina.ini
sudo phpenmod patina
sudo systemctl restart php*-fpm
```

### Docker Build (specific PHP version)

```bash
# Build for PHP 8.3
docker build --build-arg PHP_VERSION=8.3 -f docker/Dockerfile.build -o dist/ .
ls dist/patina.so
```

## Bridge mu-plugin (Future)

For non-pluggable function replacement (Phase 9+), install the bridge mu-plugin:

```bash
cp php/bridge/patina-bridge.php /path/to/wordpress/wp-content/mu-plugins/
```

This is NOT needed for the current release — pluggable functions work without it.

## Rollback

```bash
# Disable the extension
sudo rm /etc/php/*/mods-available/patina.ini 2>/dev/null
# Or more precisely:
INI_DIR=$(php --ini | grep 'Scan for' | cut -d: -f2 | tr -d ' ')
sudo rm "$INI_DIR/99-patina.ini"
sudo systemctl restart php*-fpm
```

WordPress works exactly as before. No data changes, nothing to undo.

## Troubleshooting

**Extension doesn't load:**
```bash
# Check PHP error log
php -d extension=patina.so -r "echo 'ok';" 2>&1
# Common issues:
# - Wrong PHP version (each .so is compiled for a specific PHP API version)
# - Missing dependencies (unlikely — patina has no runtime deps beyond libc)
# - Permission issues on the .so file
```

**Functions not registered:**
```bash
php -r "print_r(get_extension_funcs('patina-ext'));"
# Should list: patina_version, patina_loaded, patina_esc_html, patina_esc_attr,
#              wp_sanitize_redirect, wp_validate_redirect
```

**Performance not improved:**
- Patina currently replaces only pluggable functions directly. The high-impact functions (`esc_html`, `wp_kses`) need the bridge mu-plugin (Phase 9).
- Run `php php/benchmarks/run.php` to measure actual speedup on your system.

## Configuration

Patina has no configuration. It either loads or it doesn't.

To temporarily disable without uninstalling:

```bash
# Via environment variable (requires bridge mu-plugin)
PATINA_DISABLE=1 php your-script.php

# Via PHP CLI flag
php -d "extension=" your-script.php  # omit patina from loaded extensions
```
