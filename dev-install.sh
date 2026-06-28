#!/usr/bin/env bash
# dev-install.sh — build from source, install kg, wire MCP + skills to Claude Code
# Usage: ./dev-install.sh [--vault ~/path/to/vault]
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${HOME}/.local/bin"
VAULT_DIR="${HOME}/kg-vault"

for arg in "$@"; do
  case "$arg" in
    --vault) shift; VAULT_DIR="$1"; shift ;;
    --vault=*) VAULT_DIR="${arg#--vault=}" ;;
  esac
done

step() { printf '\n\033[1;36m==> %s\033[0m\n' "$*"; }
ok()   { printf '\033[1;32m  ✓ %s\033[0m\n' "$*"; }
info() { printf '\033[0;90m    %s\033[0m\n' "$*"; }

# ── 1. Build ──────────────────────────────────────────────────────────────────
step "Building kg from source (release)"
info "repo: $REPO_DIR"
cd "$REPO_DIR"
cargo build --release -p kgx-cli 2>&1 | tail -5
ok "build complete → target/release/kg"

# ── 2. Install binary ─────────────────────────────────────────────────────────
step "Installing kg binary"
mkdir -p "$BIN_DIR"
cp target/release/kg "$BIN_DIR/kg"
chmod +x "$BIN_DIR/kg"
ok "installed → $BIN_DIR/kg"

# Ensure BIN_DIR is on PATH for the remainder of this script
export PATH="$BIN_DIR:$PATH"

kg --version
ok "kg is runnable"

# ── 3. Init vault ─────────────────────────────────────────────────────────────
step "Initializing vault at $VAULT_DIR"
mkdir -p "$VAULT_DIR"
KGX_LLM=mock kg init \
  --vault "$VAULT_DIR" \
  --template research \
  --with-skills \
  --okf
ok "vault scaffolded"

# ── 4. Wire MCP + skill to Claude Code (global) ───────────────────────────────
step "Wiring kgx MCP server to Claude Code"

if ! command -v claude &>/dev/null; then
  printf '\033[0;33m  ! claude CLI not found — skipping MCP registration\033[0m\n'
  printf '    Run manually later: claude mcp add --transport stdio kgx -- kg mcp-server --transport stdio\n'
else
  # Remove stale entry if it exists, then re-add (idempotent)
  claude mcp remove kgx 2>/dev/null || true
  claude mcp add --transport stdio kgx -- "$BIN_DIR/kg" mcp-server --transport stdio
  ok "registered kgx in Claude Code global MCP config"

  # Install the skill file so Claude Code sees it in /skills:kgx
  SKILL_SRC="$REPO_DIR/skills/claude/.claude/skills/kgx/SKILL.md"
  SKILL_DST="${HOME}/.claude/skills/kgx/SKILL.md"
  if [ -f "$SKILL_SRC" ]; then
    mkdir -p "$(dirname "$SKILL_DST")"
    cp "$SKILL_SRC" "$SKILL_DST"
    ok "installed skill → $SKILL_DST"
  fi
fi

# ── 5. Smoke-check the MCP server starts ──────────────────────────────────────
step "Smoke-checking MCP server (send initialize, expect response)"
VAULT_DIR_ABS="$(cd "$VAULT_DIR" && pwd)"
RESPONSE=$(
  printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"dev-install-test","version":"0"}}}\n' \
  | timeout 5 env KGX_LLM=mock "$BIN_DIR/kg" mcp-server --transport stdio 2>/dev/null \
  || true
)
if echo "$RESPONSE" | grep -q '"result"'; then
  ok "MCP server responds to initialize"
else
  printf '\033[0;33m  ! MCP server did not return expected JSON (may need a vault in cwd)\033[0m\n'
  info "response: $RESPONSE"
fi

# ── 6. Summary ────────────────────────────────────────────────────────────────
step "Done"
cat <<EOF

  Binary  : $BIN_DIR/kg
  Vault   : $VAULT_DIR
  MCP     : kg mcp-server --transport stdio

  To verify Claude Code sees it:
    claude mcp list

  To test the vault:
    cd $VAULT_DIR
    KGX_LLM=mock kg status --json
    KGX_LLM=mock kg ask "What is this vault?" --cite --json

  Add $BIN_DIR to your PATH if not already there:
    echo 'export PATH="\$HOME/.local/bin:\$PATH"' >> ~/.zshrc && source ~/.zshrc

EOF
