#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${KGX_RELEASE_VERSION:-${GITHUB_REF_NAME:-dev}}"
TARGET="${KGX_RELEASE_TARGET:-$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)}"
OUT_DIR="${KGX_RELEASE_OUT_DIR:-$ROOT_DIR/dist}"
BIN_SRC="${KGX_RELEASE_BIN:-$ROOT_DIR/target/release/kg}"

if [[ ! -f "$BIN_SRC" ]]; then
  printf 'kg binary not found: %s\n' "$BIN_SRC" >&2
  exit 1
fi

case "$(basename "$BIN_SRC")" in
  *.exe) BIN_NAME="kg.exe" ;;
  *) BIN_NAME="kg" ;;
esac

PKG_NAME="kgx-${VERSION}-${TARGET}"
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

PKG_DIR="$WORK_DIR/$PKG_NAME"
mkdir -p "$PKG_DIR/bin" "$OUT_DIR"

cp "$BIN_SRC" "$PKG_DIR/bin/$BIN_NAME"
chmod +x "$PKG_DIR/bin/$BIN_NAME" 2>/dev/null || true
cp -R "$ROOT_DIR/skills" "$PKG_DIR/skills"
cp "$ROOT_DIR/README.md" "$PKG_DIR/README.md"

cat >"$PKG_DIR/install.sh" <<'INSTALL'
#!/usr/bin/env bash
set -euo pipefail

PKG_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${KGX_BIN_DIR:-$HOME/.local/bin}"
SHARE_DIR="${KGX_SHARE_DIR:-$HOME/.kgx}"

mkdir -p "$BIN_DIR" "$SHARE_DIR"

if [[ -f "$PKG_DIR/bin/kg.exe" ]]; then
  cp "$PKG_DIR/bin/kg.exe" "$BIN_DIR/kg.exe"
  chmod +x "$BIN_DIR/kg.exe" 2>/dev/null || true
  INSTALLED_BIN="$BIN_DIR/kg.exe"
else
  cp "$PKG_DIR/bin/kg" "$BIN_DIR/kg"
  chmod +x "$BIN_DIR/kg"
  INSTALLED_BIN="$BIN_DIR/kg"
fi

rm -rf "$SHARE_DIR/skills"
cp -R "$PKG_DIR/skills" "$SHARE_DIR/skills"

printf 'Installed kg binary: %s\n' "$INSTALLED_BIN"
printf 'Installed skill templates: %s\n' "$SHARE_DIR/skills"
printf 'Next: cd <vault> && kg init --with-skills --with-rtk\n'
printf 'MCP server command: kg mcp-server --transport stdio\n'

if command -v claude >/dev/null 2>&1; then
  claude mcp remove kgx 2>/dev/null || true
  claude mcp add --transport stdio kgx -- "$INSTALLED_BIN" mcp-server --transport stdio
fi
if command -v codex >/dev/null 2>&1; then
  codex mcp remove kgx 2>/dev/null || true
fi
INSTALL
chmod +x "$PKG_DIR/install.sh"

cat >"$PKG_DIR/MANIFEST.txt" <<MANIFEST
KGX release package
version: $VERSION
target: $TARGET

Contents:
- bin/$BIN_NAME: kg CLI binary. The same binary runs the MCP server via 'kg mcp-server --transport stdio'.
- skills/: Claude Code, Codex, Cursor, OpenCode, and shared hook templates.
- install.sh: local installer for the binary and bundled skill templates.
- README.md: project usage reference.
MANIFEST

ARCHIVE="$OUT_DIR/$PKG_NAME.zip"
PKG_NAME="$PKG_NAME" WORK_DIR="$WORK_DIR" ARCHIVE="$ARCHIVE" python3 - <<'PY'
import os
import pathlib
import zipfile

work_dir = pathlib.Path(os.environ["WORK_DIR"])
pkg_name = os.environ["PKG_NAME"]
archive = pathlib.Path(os.environ["ARCHIVE"])
pkg_dir = work_dir / pkg_name

with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as zf:
    for path in pkg_dir.rglob("*"):
        zf.write(path, path.relative_to(work_dir))
PY

if command -v shasum >/dev/null 2>&1; then
  (cd "$OUT_DIR" && shasum -a 256 "$(basename "$ARCHIVE")" >"$(basename "$ARCHIVE").sha256")
else
  (cd "$OUT_DIR" && sha256sum "$(basename "$ARCHIVE")" >"$(basename "$ARCHIVE").sha256")
fi

printf '%s\n' "$ARCHIVE"
