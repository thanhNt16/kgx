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

URL="${KGX_INSTALL_URL:-https://get.kgx.sh/bin/kg-${OS}-${ARCH}}"
echo "Downloading kg (${OS}/${ARCH})..."
curl -fsSL "$URL" -o "$BIN_DIR/kg"
chmod +x "$BIN_DIR/kg"
echo "Installed kg -> $BIN_DIR/kg"

for arg in "$@"; do
  case "$arg" in
    --with-rtk) echo "RTK setup: run kg init --with-rtk inside a vault." ;;
    --with-html-graph) echo "HTML graph support is included in kg graph --format html." ;;
    --with-docs) echo "Docs support is included in kg docs usecase <name>." ;;
    --with-cron) echo "Cron support is included in kg cron." ;;
  esac
done

if command -v claude >/dev/null 2>&1; then
  claude mcp add --transport stdio kgx -- "$BIN_DIR/kg" mcp-server --transport stdio || true
  echo "Registered kgx MCP server with Claude Code"
fi

echo "Next: cd <vault> && kg init --with-skills && kg capture --from <file>"
