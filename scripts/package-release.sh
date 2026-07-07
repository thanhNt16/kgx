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

# install_bin <src> <dst>
# Install a freshly-downloaded binary so it actually launches.
#
# macOS Gatekeeper can cache a blocking assessment for an unnotarized,
# adhoc-signed binary on a *per-inode* basis: the same bytes that run fine
# elsewhere hang forever in dyld (_dyld_start) at a path that was previously
# assessed. The fix is (1) remove any stale target so the copy lands on a fresh
# inode, (2) clear quarantine/provenance xattrs, and (3) verify the binary
# responds to --version within a few seconds so we never silently install a
# binary that will hang the user's shell.
run_with_timeout() {
  # Run a command, killing it if it runs longer than N seconds. Falls back to a
  # plain run if no timeout(1)/gtimeout(1) is available.
  local secs="$1"; shift
  if command -v timeout >/dev/null 2>&1; then
    timeout "$secs" "$@"
  elif command -v gtimeout >/dev/null 2>&1; then
    gtimeout "$secs" "$@"
  else
    "$@"
  fi
}

install_bin() {
  local src="$1" dst="$2"
  rm -f "$dst"
  cp "$src" "$dst"
  chmod +x "$dst" 2>/dev/null || true
  if [ "$(uname -s)" = "Darwin" ]; then
    xattr -c "$dst" 2>/dev/null || true
  fi
  # Sanity check: the binary must start up. Use a hard timeout so a hang fails
  # the install loudly instead of leaving a broken binary on PATH.
  if ! run_with_timeout 10 "$dst" --version >/dev/null 2>&1; then
    echo "ERROR: installed binary at $dst did not respond to --version within 10s." >&2
    echo "       This is usually a macOS Gatekeeper stall on an unnotarized binary." >&2
    echo "       Try: xattr -c \"$dst\" && \"$dst\" --version" >&2
    # Fall back: a forced fresh inode + xattr clear sometimes clears a cached
    # rejection. If that also fails, surface the error.
    rm -f "$dst"
    cp "$src" "$dst"
    chmod +x "$dst" 2>/dev/null || true
    [ "$(uname -s)" = "Darwin" ] && xattr -c "$dst" 2>/dev/null || true
    if ! run_with_timeout 10 "$dst" --version >/dev/null 2>&1; then
      echo "ERROR: binary at $dst still unresponsive after re-install." >&2
      exit 1
    fi
  fi
}

if [[ -f "$PKG_DIR/bin/kg.exe" ]]; then
  install_bin "$PKG_DIR/bin/kg.exe" "$BIN_DIR/kg.exe"
  INSTALLED_BIN="$BIN_DIR/kg.exe"
else
  install_bin "$PKG_DIR/bin/kg" "$BIN_DIR/kg"
  INSTALLED_BIN="$BIN_DIR/kg"
fi

rm -rf "$SHARE_DIR/skills"
cp -R "$PKG_DIR/skills" "$SHARE_DIR/skills"

printf 'Installed kg binary: %s\n' "$INSTALLED_BIN"
printf 'Installed skill templates: %s\n' "$SHARE_DIR/skills"
printf 'Next: cd <vault> && kg init --with-skills --with-rtk\n'
printf 'MCP server command: kg mcp-server --transport stdio\n'

# Install Claude Code global skills + commands
if [ -d "$PKG_DIR/skills/claude" ]; then
  CLAUDE_DIR="$HOME/.claude"
  mkdir -p "$CLAUDE_DIR/skills/kgx" "$CLAUDE_DIR/commands"
  cp "$PKG_DIR/skills/claude/.claude/skills/kgx/SKILL.md" "$CLAUDE_DIR/skills/kgx/SKILL.md"
  for tmpl in "$PKG_DIR/skills/claude/.claude/commands/"*.md; do
    verb=$(basename "$tmpl" .md)
    cp "$tmpl" "$CLAUDE_DIR/commands/kgx:${verb}.md"
  done
  printf 'Installed KGX skills+commands to ~/.claude/\n'
fi

# Install OpenCode global skills + commands
if [ -d "$PKG_DIR/skills/opencode" ]; then
  OPENCODE_DIR="$HOME/.config/opencode"
  mkdir -p "$OPENCODE_DIR/skills/kgx" "$OPENCODE_DIR/command"
  cp "$PKG_DIR/skills/opencode/.opencode/skills/kgx/SKILL.md" "$OPENCODE_DIR/skills/kgx/SKILL.md"
  if [ -d "$PKG_DIR/skills/opencode/.opencode/skills/kgx-codebase" ]; then
    mkdir -p "$OPENCODE_DIR/skills/kgx-codebase"
    cp "$PKG_DIR/skills/opencode/.opencode/skills/kgx-codebase/SKILL.md" "$OPENCODE_DIR/skills/kgx-codebase/SKILL.md"
  fi
  if [ -d "$PKG_DIR/skills/opencode/.opencode/skills/kgx-codebase-index" ]; then
    mkdir -p "$OPENCODE_DIR/skills/kgx-codebase-index"
    cp "$PKG_DIR/skills/opencode/.opencode/skills/kgx-codebase-index/SKILL.md" "$OPENCODE_DIR/skills/kgx-codebase-index/SKILL.md"
  fi
  if [ -d "$PKG_DIR/skills/opencode/.opencode/command" ]; then
    for tmpl in "$PKG_DIR/skills/opencode/.opencode/command/"*.md; do
      cp "$tmpl" "$OPENCODE_DIR/command/$(basename "$tmpl")"
    done
  fi
  printf 'Installed KGX skills+commands to ~/.config/opencode/\n'
fi

if command -v claude >/dev/null 2>&1; then
  # Reinstall must be authoritative. A stale kgx entry in any scope conflicts
  # with the new one — Claude Code refuses to connect when an MCP server is
  # defined in multiple scopes with different endpoints ("defined in multiple
  # scopes ... ✘ Failed to connect"). `claude mcp remove` only reaches user
  # and the current project's local scope, so we ALSO strip kgx from every
  # project entry in ~/.claude.json directly, then register once in user scope
  # (the cross-project home for a globally-installed CLI server).
  for scope in user project; do
    claude mcp remove kgx -s "$scope" >/dev/null 2>&1 || true
  done
  python3 - "$HOME/.claude.json" <<'PY'
import json, sys
path = sys.argv[1]
try:
    d = json.loads(open(path).read())
except Exception:
    sys.exit(0)
changed = False
for proj, cfg in (d.get("projects") or {}).items():
    mcp = cfg.get("mcpServers") or {}
    if "kgx" in mcp:
        del mcp["kgx"]
        cfg["mcpServers"] = mcp
        changed = True
if changed:
    open(path, "w").write(json.dumps(d, indent=2))
PY
  echo "y" | claude mcp add -s user --transport stdio kgx -- "$INSTALLED_BIN" mcp-server --transport stdio
fi
if command -v codex >/dev/null 2>&1; then
  codex mcp remove kgx &>/dev/null || true
  echo "y" | codex mcp add kgx -- "$INSTALLED_BIN" mcp-server --transport stdio
fi
if command -v opencode >/dev/null 2>&1; then
  echo "y" | opencode mcp add kgx -- "$INSTALLED_BIN" mcp-server --transport stdio &>/dev/null || true
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
