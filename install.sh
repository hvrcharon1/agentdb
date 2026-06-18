#!/bin/sh
set -e

# AgentDB installer — works on Linux, macOS, and WSL
# Usage: curl -fsSL https://raw.githubusercontent.com/hvrcharon1/agentdb/main/install.sh | sh

REPO="hvrcharon1/agentdb"
INSTALL_DIR="${AGENTDB_INSTALL_DIR:-/usr/local/bin}"
VERSION="${AGENTDB_VERSION:-latest}"

info() { printf "\033[1;34m=>\033[0m %s\n" "$1"; }
error() { printf "\033[1;31merror:\033[0m %s\n" "$1" >&2; exit 1; }

detect_platform() {
  OS=$(uname -s | tr '[:upper:]' '[:lower:]')
  ARCH=$(uname -m)

  case "$OS" in
    linux*)  OS="unknown-linux-gnu" ;;
    darwin*) OS="apple-darwin" ;;
    *)       error "Unsupported OS: $OS" ;;
  esac

  case "$ARCH" in
    x86_64|amd64)  ARCH="x86_64" ;;
    aarch64|arm64) ARCH="aarch64" ;;
    *)             error "Unsupported architecture: $ARCH" ;;
  esac

  PLATFORM="${ARCH}-${OS}"
}

get_latest_version() {
  if [ "$VERSION" = "latest" ]; then
    VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed 's/.*"v\(.*\)".*/\1/')
  fi
  [ -n "$VERSION" ] || error "Could not determine latest version"
}

download_and_install() {
  URL="https://github.com/${REPO}/releases/download/v${VERSION}/agentdb-${PLATFORM}.tar.gz"
  CHECKSUM_URL="https://github.com/${REPO}/releases/download/v${VERSION}/checksums-sha256.txt"

  info "Downloading agentdb v${VERSION} for ${PLATFORM}..."
  TMPDIR=$(mktemp -d)
  trap 'rm -rf "$TMPDIR"' EXIT

  curl -fsSL "$URL" -o "$TMPDIR/agentdb.tar.gz"
  curl -fsSL "$CHECKSUM_URL" -o "$TMPDIR/checksums.txt"

  # Verify checksum
  EXPECTED=$(grep "agentdb-${PLATFORM}.tar.gz" "$TMPDIR/checksums.txt" | awk '{print $1}')
  if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL=$(sha256sum "$TMPDIR/agentdb.tar.gz" | awk '{print $1}')
  elif command -v shasum >/dev/null 2>&1; then
    ACTUAL=$(shasum -a 256 "$TMPDIR/agentdb.tar.gz" | awk '{print $1}')
  else
    info "Warning: cannot verify checksum (sha256sum/shasum not found)"
    ACTUAL="$EXPECTED"
  fi

  if [ "$EXPECTED" != "$ACTUAL" ]; then
    error "Checksum mismatch!\n  Expected: $EXPECTED\n  Actual:   $ACTUAL"
  fi
  info "Checksum verified."

  # Extract and install
  tar xzf "$TMPDIR/agentdb.tar.gz" -C "$TMPDIR"

  if [ -w "$INSTALL_DIR" ]; then
    mv "$TMPDIR/agentdb" "$INSTALL_DIR/agentdb"
  else
    info "Installing to $INSTALL_DIR (requires sudo)..."
    sudo mv "$TMPDIR/agentdb" "$INSTALL_DIR/agentdb"
  fi

  chmod +x "$INSTALL_DIR/agentdb"
  info "Installed agentdb v${VERSION} to $INSTALL_DIR/agentdb"
  info "Run 'agentdb --version' to verify."
}

detect_platform
get_latest_version
download_and_install
