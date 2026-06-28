#!/usr/bin/env bash
set -euo pipefail

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"
BIN_DIR="${KGX_BIN_DIR:-$HOME/.local/bin}"
mkdir -p "$BIN_DIR"

case "$ARCH" in
  x86_64|amd64) ARCH="x86_64" ;;
  arm64|aarch64) ARCH="aarch64" ;;
esac

case "$OS" in
  darwin) TARGET="macos-$ARCH" ;;
  linux) TARGET="linux-$ARCH" ;;
  mingw*|msys*|cygwin*) TARGET="windows-$ARCH" ;;
  *) echo "Unsupported OS: $OS" >&2; exit 1 ;;
esac

REPO="${KGX_REPO:-thanhNt16/kgx}"
VERSION="${KGX_VERSION:-latest}"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

if [ "$VERSION" = "latest" ]; then
  VERSION="$(
    curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
      | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
      | head -1
  )"
fi

if [ -z "$VERSION" ]; then
  echo "Could not determine latest KGX release version" >&2
  exit 1
fi

case "$VERSION" in
  v*) ;;
  *) VERSION="v$VERSION" ;;
esac

ARCHIVE="kgx-${VERSION}-${TARGET}.zip"
URL="${KGX_INSTALL_URL:-https://github.com/$REPO/releases/download/$VERSION/$ARCHIVE}"

echo "Downloading KGX ${VERSION} (${TARGET})..."
curl -fsSL "$URL" -o "$TMP_DIR/$ARCHIVE"
if command -v unzip >/dev/null 2>&1; then
  unzip -q "$TMP_DIR/$ARCHIVE" -d "$TMP_DIR"
else
  python3 - "$TMP_DIR/$ARCHIVE" "$TMP_DIR" <<'PY'
import sys
import zipfile

with zipfile.ZipFile(sys.argv[1]) as zf:
    zf.extractall(sys.argv[2])
PY
fi
"$TMP_DIR/kgx-${VERSION}-${TARGET}/install.sh"

for arg in "$@"; do
  case "$arg" in
    --with-rtk) echo "RTK setup: run kg init --with-rtk inside a vault." ;;
    --with-html-graph) echo "HTML graph support is included in kg graph --format html." ;;
    --with-docs) echo "Docs support is included in kg docs usecase <name>." ;;
    --with-cron) echo "Cron support is included in kg cron." ;;
  esac
done

echo "Next: cd <vault> && kg init --with-skills --with-rtk && kg capture --from <file>"
