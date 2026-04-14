#!/bin/bash
set -e

VERSION="1.0.0"
REPO="vornyx-rs/VeD-LaNg"

detect_platform() {
    OS=$(uname -s | tr '[:upper:]' '[:lower:]')
    ARCH=$(uname -m)

    case "$OS" in
        linux)  PLATFORM="linux"  ;;
        darwin) PLATFORM="macos"  ;;
        *) echo "Unsupported OS: $OS" >&2; exit 1 ;;
    esac

    case "$ARCH" in
        x86_64)       ARCH_NAME="x86_64" ;;
        arm64|aarch64) ARCH_NAME="arm64" ;;
        *) echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
    esac

    echo "vedc-${PLATFORM}-${ARCH_NAME}"
}

main() {
    ARTIFACT=$(detect_platform)
    FILENAME="${ARTIFACT}.tar.gz"
    URL="https://github.com/${REPO}/releases/download/v${VERSION}/${FILENAME}"

    echo "Installing VED ${VERSION} (${ARTIFACT})..."

    TMP_DIR=$(mktemp -d)
    trap "rm -rf ${TMP_DIR}" EXIT

    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "${URL}" -o "${TMP_DIR}/${FILENAME}"
    elif command -v wget >/dev/null 2>&1; then
        wget -q "${URL}" -O "${TMP_DIR}/${FILENAME}"
    else
        echo "Error: curl or wget is required" >&2
        exit 1
    fi

    tar -xzf "${TMP_DIR}/${FILENAME}" -C "${TMP_DIR}"

    if [ ! -f "${TMP_DIR}/vedc" ]; then
        echo "Error: vedc binary not found in archive" >&2
        exit 1
    fi

    chmod +x "${TMP_DIR}/vedc"

    INSTALL_DIR="${HOME}/.local/bin"
    mkdir -p "${INSTALL_DIR}"

    if mv "${TMP_DIR}/vedc" "${INSTALL_DIR}/vedc" 2>/dev/null; then
        echo "Installed to ${INSTALL_DIR}/vedc"
        INSTALL_PATH="${INSTALL_DIR}/vedc"
    elif sudo mv "${TMP_DIR}/vedc" /usr/local/bin/vedc 2>/dev/null; then
        echo "Installed to /usr/local/bin/vedc"
        INSTALL_PATH="/usr/local/bin/vedc"
    else
        echo "Error: could not install binary. Try: sudo mv ${TMP_DIR}/vedc /usr/local/bin/vedc" >&2
        exit 1
    fi

    echo ""
    echo "VED ${VERSION} installed."
    echo ""
    echo "Quick start:"
    echo "  vedc --version"
    echo "  vedc new my-app"
    echo "  vedc run examples/hello.ved"
    echo ""

    if ! command -v vedc >/dev/null 2>&1; then
        echo "Note: add ${INSTALL_DIR} to your PATH if vedc is not found:"
        echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
        echo ""
    fi
}

main "$@"
