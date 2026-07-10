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

# Verify the freshly installed binary actually launches. A downloaded,
# adhoc-signed binary can stall forever in dyld on macOS due to a cached
# Gatekeeper assessment (the inner installer clears this, but verify anyway so
# we never tell the user "installed" while the binary hangs).
BIN_DIR="${KGX_BIN_DIR:-$HOME/.local/bin}"
INSTALLED_BIN="$BIN_DIR/kg"
[ -f "$BIN_DIR/kg.exe" ] && INSTALLED_BIN="$BIN_DIR/kg.exe"

run_with_timeout() {
  local secs="$1"; shift
  if command -v timeout >/dev/null 2>&1; then timeout "$secs" "$@"
  elif command -v gtimeout >/dev/null 2>&1; then gtimeout "$secs" "$@"
  else "$@"; fi
}

if [ -x "$INSTALLED_BIN" ]; then
  if run_with_timeout 10 "$INSTALLED_BIN" --version >/dev/null 2>&1; then
    echo "Verified: $INSTALLED_BIN --version responds."
  else
    echo "WARNING: $INSTALLED_BIN did not respond to --version within 10s." >&2
    echo "         On macOS this is usually a Gatekeeper stall on an unnotarized binary." >&2
    echo "         Fix: xattr -c \"$INSTALLED_BIN\" && \"$INSTALLED_BIN\" --version" >&2
  fi
fi

# Install pandoc for document conversion (PDF/Word/PPTX/etc.)
PANDOC_VERSION="3.1.11"
PANDOC_BIN="$BIN_DIR/pandoc-kgx"
if [ ! -x "$PANDOC_BIN" ]; then
  case "$TARGET" in
    macos-x86_64)  PANDOC_PLATFORM="x86_64-apple-darwin" ;;
    macos-aarch64) PANDOC_PLATFORM="aarch64-apple-darwin" ;;
    linux-x86_64)  PANDOC_PLATFORM="x86_64-linux-gnu" ;;
    linux-aarch64) PANDOC_PLATFORM="aarch64-linux-gnu" ;;
    windows-x86_64) PANDOC_PLATFORM="x86_64-windows" ;;
    *) echo "No pandoc bundle for $TARGET — install pandoc manually if you need document conversion." >&2 ;;
  esac
  if [ -n "$PANDOC_PLATFORM" ]; then
    PANDOC_URL="https://github.com/jgm/pandoc/releases/download/${PANDOC_VERSION}/pandoc-${PANDOC_VERSION}-${PANDOC_PLATFORM}.zip"
    echo "Downloading pandoc ${PANDOC_VERSION}..."
    if curl -fsSL "$PANDOC_URL" -o "$TMP_DIR/pandoc.zip" 2>/dev/null; then
      if command -v unzip >/dev/null 2>&1; then
        unzip -q -o "$TMP_DIR/pandoc.zip" -d "$TMP_DIR/pandoc-extract"
      else
        python3 - "$TMP_DIR/pandoc.zip" "$TMP_DIR/pandoc-extract" <<'PY'
import sys, zipfile, os
os.makedirs(sys.argv[2], exist_ok=True)
with zipfile.ZipFile(sys.argv[1]) as zf:
    zf.extractall(sys.argv[2])
PY
      fi
      PANDOC_SRC=$(find "$TMP_DIR/pandoc-extract" -name "pandoc" -o -name "pandoc.exe" | head -1)
      if [ -n "$PANDOC_SRC" ]; then
        cp "$PANDOC_SRC" "$PANDOC_BIN"
        chmod +x "$PANDOC_BIN"
        echo "Installed pandoc to $PANDOC_BIN"
      fi
    else
      echo "Could not download pandoc — install manually if you need document conversion." >&2
    fi
  fi
fi

for arg in "$@"; do
  case "$arg" in
    --with-rtk) echo "RTK setup: run kg init --with-rtk inside a vault." ;;
    --with-html-graph) echo "HTML graph support is included in kg graph --format html." ;;
    --with-docs) echo "Docs support is included in kg docs usecase <name>." ;;
    --with-cron) echo "Cron support is included in kg cron." ;;
  esac
done

echo "Next: cd <vault> && kg init --with-skills --with-rtk && kg capture --from <file>"
