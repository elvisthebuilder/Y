#!/bin/sh
set -e

REPO="elvisthebuilder/Y"
INSTALL_DIR="/usr/local/bin"

get_latest_version() {
    curl -sL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | head -1 \
        | sed 's/.*"tag_name": *"//;s/".*//'
}

detect_platform() {
    os=$(uname -s)
    arch=$(uname -m)

    case "$os" in
        Linux)
            case "$arch" in
                x86_64) echo "y-linux-x86_64.tar.gz" ;;
                *) echo "unsupported" ;;
            esac
            ;;
        Darwin)
            case "$arch" in
                x86_64) echo "y-macos-x86_64.tar.gz" ;;
                arm64)  echo "y-macos-aarch64.tar.gz" ;;
                *) echo "unsupported" ;;
            esac
            ;;
        *) echo "unsupported" ;;
    esac
}

main() {
    echo "  Installing Y..."
    echo ""

    VERSION=$(get_latest_version)
    if [ -z "$VERSION" ]; then
        echo "  Error: could not fetch latest release."
        exit 1
    fi

    ARCHIVE=$(detect_platform)
    if [ "$ARCHIVE" = "unsupported" ]; then
        echo "  Error: unsupported platform ($(uname -s) $(uname -m))."
        echo "  Build from source: cargo install --path ."
        exit 1
    fi

    URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE}"

    echo "  Version:  $VERSION"
    echo "  Platform: $(uname -s) $(uname -m)"
    echo "  Archive:  $ARCHIVE"
    echo ""

    TMP=$(mktemp -d)
    trap 'rm -rf "$TMP"' EXIT

    echo "  Downloading..."
    curl -sL "$URL" -o "$TMP/$ARCHIVE"

    echo "  Extracting..."
    tar xzf "$TMP/$ARCHIVE" -C "$TMP"

    echo "  Installing to $INSTALL_DIR..."
    if [ -w "$INSTALL_DIR" ]; then
        mv "$TMP/y" "$INSTALL_DIR/y"
    else
        sudo mv "$TMP/y" "$INSTALL_DIR/y"
    fi
    chmod +x "$INSTALL_DIR/y"

    echo ""
    echo "  Done. Run: y open"
}

main
