#!/bin/bash
set -euo pipefail

# Patina installer — downloads the correct pre-built extension for your system.
#
# Usage:
#   curl -sSL https://raw.githubusercontent.com/PeterBooker/patina/main/install.sh | bash
#   ./install.sh                    # Latest release
#   ./install.sh v0.1.0             # Specific version

REPO="PeterBooker/patina"
VERSION="${1:-latest}"

# --- Detect environment ---

if ! command -v php &>/dev/null; then
    echo "Error: php not found in PATH" >&2
    exit 1
fi

PHP_VERSION=$(php -r "echo PHP_MAJOR_VERSION.'.'.PHP_MINOR_VERSION;")
ARCH=$(uname -m)

case "$ARCH" in
    x86_64|amd64) ARCH="x86_64" ;;
    aarch64|arm64) ARCH="aarch64" ;;
    *)
        echo "Error: unsupported architecture: $ARCH" >&2
        echo "Patina supports x86_64 and aarch64 only." >&2
        exit 1
        ;;
esac

OS=$(uname -s)
if [ "$OS" != "Linux" ]; then
    echo "Error: unsupported OS: $OS" >&2
    echo "Patina supports Linux only." >&2
    exit 1
fi

case "$PHP_VERSION" in
    8.1|8.2|8.3|8.4) ;;
    *)
        echo "Error: unsupported PHP version: $PHP_VERSION" >&2
        echo "Patina supports PHP 8.1, 8.2, 8.3, and 8.4." >&2
        exit 1
        ;;
esac

ARTIFACT="patina-php${PHP_VERSION}-linux-${ARCH}.so"

echo "Patina installer"
echo "  PHP version: $PHP_VERSION"
echo "  Architecture: $ARCH"
echo "  Artifact: $ARTIFACT"
echo ""

# --- Download ---

if [ "$VERSION" = "latest" ]; then
    URL="https://github.com/${REPO}/releases/latest/download/${ARTIFACT}"
    SUMS_URL="https://github.com/${REPO}/releases/latest/download/SHA256SUMS"
else
    URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARTIFACT}"
    SUMS_URL="https://github.com/${REPO}/releases/download/${VERSION}/SHA256SUMS"
fi

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

echo "Downloading $ARTIFACT..."
if ! curl -fSL "$URL" -o "$TMPDIR/patina.so"; then
    echo "Error: download failed. Check that the release exists and has artifacts for PHP $PHP_VERSION / $ARCH." >&2
    exit 1
fi

echo "Downloading checksums..."
if curl -fSL "$SUMS_URL" -o "$TMPDIR/SHA256SUMS" 2>/dev/null; then
    echo "Verifying checksum..."
    (cd "$TMPDIR" && grep "$ARTIFACT" SHA256SUMS | sed "s|${ARTIFACT}|patina.so|" | sha256sum -c -)
else
    echo "Warning: could not download checksums, skipping verification." >&2
fi

# --- Install ---

EXT_DIR=$(php -r "echo ini_get('extension_dir');")
INI_SCAN_DIR=$(php --ini | grep 'Scan for' | cut -d: -f2 | tr -d ' ')

echo ""
echo "Installing to $EXT_DIR/patina.so"

if [ -w "$EXT_DIR" ]; then
    cp "$TMPDIR/patina.so" "$EXT_DIR/patina.so"
else
    sudo cp "$TMPDIR/patina.so" "$EXT_DIR/patina.so"
fi

# Enable if not already enabled
if ! php -m 2>/dev/null | grep -q 'patina-ext'; then
    echo "Enabling extension..."
    if [ -n "$INI_SCAN_DIR" ] && [ -d "$INI_SCAN_DIR" ]; then
        if [ -w "$INI_SCAN_DIR" ]; then
            echo "extension=patina.so" > "$INI_SCAN_DIR/99-patina.ini"
        else
            echo "extension=patina.so" | sudo tee "$INI_SCAN_DIR/99-patina.ini" >/dev/null
        fi
    else
        echo "Warning: could not find INI scan directory. Add 'extension=patina.so' to your php.ini manually." >&2
    fi
fi

# --- Verify ---

echo ""
echo "Verifying..."
LOADED_VERSION=$(php -r "echo patina_version();" 2>/dev/null || true)
if [ -n "$LOADED_VERSION" ]; then
    echo "Patina $LOADED_VERSION installed successfully."
    echo ""
    echo "Restart PHP-FPM to activate:"
    echo "  sudo systemctl restart php${PHP_VERSION}-fpm"
else
    echo "Warning: extension installed but could not verify. You may need to restart PHP-FPM." >&2
    echo "  sudo systemctl restart php${PHP_VERSION}-fpm"
fi
