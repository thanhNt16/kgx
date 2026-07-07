#!/usr/bin/env bash
# dev-install.sh — build from source, install binary, wire MCP + skills + rules
# Usage: ./dev-install.sh [--agent claude|opencode|codex|cursor|zcode] [--vault ~/path/to/vault]
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${HOME}/.local/bin"
VAULT_DIR="${HOME}/kg-vault"
AGENT="claude"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --agent) AGENT="$2"; shift 2 ;;
    --agent=*) AGENT="${1#--agent=}"; shift ;;
    --vault) VAULT_DIR="$2"; shift 2 ;;
    --vault=*) VAULT_DIR="${1#--vault=}"; shift ;;
    *) echo "Unknown: $1"; exit 1 ;;
  esac
done

case "$AGENT" in
  claude|opencode|codex|cursor|zcode) ;;
  *) echo "Invalid --agent: $AGENT (choose: claude, opencode, codex, cursor, zcode)"; exit 1 ;;
esac

step() { printf '\n\033[1;36m==> %s\033[0m\n' "$*"; }
ok()   { printf '\033[1;32m  \xe2\x9c\x93 %s\033[0m\n' "$*"; }
info() { printf '\033[0;90m    %s\033[0m\n' "$*"; }
warn() { printf '\033[0;33m  ! %s\033[0m\n' "$*"; }

KGX_VERBS="ingest capture extract index search ask recall dream review link status init ship sync codebase codebase-index"

install_subskills() {
  local src_dir="$1" dst_dir="$2"
  for verb in $KGX_VERBS; do
    local src="${src_dir}/kgx-${verb}/SKILL.md"
    local dst="${dst_dir}/kgx-${verb}/SKILL.md"
    if [ -f "$src" ]; then
      mkdir -p "$(dirname "$dst")"
      cp "$src" "$dst"
    fi
  done
  ok "installed $(echo $KGX_VERBS | wc -w | tr -d ' ') kgx-* sub-skills -> ${dst_dir}"
}

# ── 1. Build ──────────────────────────────────────────────────────────────────
step "Building kg and rtk from source (release)"
info "repo: $REPO_DIR"
cd "$REPO_DIR"
cargo build --release -p kgx-cli 2>&1 | tail -5
ok "build complete -> target/release/kg"
cargo build --release -p kgx-rtk 2>&1 | tail -3
ok "build complete -> target/release/rtk"

# ── 2. Install binaries ───────────────────────────────────────────────────────
step "Installing kg, rtk, and codebase-memory-mcp"
mkdir -p "$BIN_DIR"
cp target/release/kg "$BIN_DIR/kg"
chmod +x "$BIN_DIR/kg"
cp target/release/rtk "$BIN_DIR/rtk"
chmod +x "$BIN_DIR/rtk"
export PATH="$BIN_DIR:$PATH"
kg --version
rtk --version

if ! command -v codebase-memory-mcp &>/dev/null; then
  info "installing codebase-memory-mcp..."
  kg codebase install 2>&1 | tail -3
  ok "codebase-memory-mcp installed"
else
  ok "codebase-memory-mcp already installed"
fi
codebase-memory-mcp --version 2>/dev/null || true

# ── 3. Init vault ─────────────────────────────────────────────────────────────
step "Initializing vault at $VAULT_DIR"
mkdir -p "$VAULT_DIR"
KGX_LLM=mock kg init \
  --vault "$VAULT_DIR" \
  --template research \
  --with-skills \
  --okf
ok "vault scaffolded"

# ── 4. Wire MCP + skills/rules for selected agent ──────────────────────────────
step "Wiring kgx for agent: $AGENT"

case "$AGENT" in
  claude)
    if command -v claude &>/dev/null; then
      claude mcp remove kgx -s local 2>/dev/null || true
      claude mcp remove kgx -s project 2>/dev/null || true
      echo "y" | claude mcp add --transport stdio kgx -- "$BIN_DIR/kg" mcp-server --transport stdio
      ok "registered kgx in Claude Code MCP config"

      claude mcp remove codebase-memory-mcp -s local 2>/dev/null || true
      claude mcp remove codebase-memory-mcp -s project 2>/dev/null || true
      echo "y" | claude mcp add codebase-memory-mcp -- "$BIN_DIR/codebase-memory-mcp"
      ok "registered codebase-memory-mcp in Claude Code MCP config"
    else
      warn "claude CLI not found -- run manually:"
      info "  claude mcp add --transport stdio kgx -- kg mcp-server --transport stdio"
      info "  claude mcp add codebase-memory-mcp -- codebase-memory-mcp"
    fi

    SKILL_SRC="$REPO_DIR/skills/claude/.claude/skills/kgx/SKILL.md"
    SKILL_DST="${HOME}/.claude/skills/kgx/SKILL.md"
    if [ -f "$SKILL_SRC" ]; then
      mkdir -p "$(dirname "$SKILL_DST")"
      cp "$SKILL_SRC" "$SKILL_DST"
      ok "installed skill -> $SKILL_DST"
    fi

    install_subskills \
      "$REPO_DIR/skills/claude/.claude/skills" \
      "${HOME}/.claude/skills"
    ;;

  opencode)
    mkdir -p "$VAULT_DIR/.opencode/skills"
    mkdir -p "$VAULT_DIR/.opencode/plugins"
    mkdir -p "${HOME}/.config/opencode"

    cp "$REPO_DIR/skills/opencode/opencode.json" "$VAULT_DIR/opencode.json"
    ok "copied opencode.json -> $VAULT_DIR/opencode.json"

    cat > "${HOME}/.config/opencode/opencode.json" << OPENCODE_EOF
{
  "\$schema": "https://opencode.ai/config.json",
  "mcp": {
    "kgx": {
      "type": "local",
      "command": ["$BIN_DIR/kg", "mcp-server", "--transport", "stdio"]
    },
    "codebase-memory-mcp": {
      "type": "local",
      "command": ["$BIN_DIR/codebase-memory-mcp"]
    }
  }
}
OPENCODE_EOF
    ok "registered kgx + codebase-memory-mcp in ~/.config/opencode/opencode.json"

    SKILLBASE="$VAULT_DIR/.opencode/skills"
    cp "$REPO_DIR/skills/opencode/.opencode/skills/kgx/SKILL.md" \
       "$SKILLBASE/kgx/SKILL.md"
    ok "installed skill -> $SKILLBASE/kgx/SKILL.md"

    install_subskills \
      "$REPO_DIR/skills/opencode/.opencode/skills" \
      "$SKILLBASE"

    cp "$REPO_DIR/skills/opencode/.opencode/plugins/kgx-verify-finished.js" \
       "$VAULT_DIR/.opencode/plugins/kgx-verify-finished.js"
    ok "installed plugin -> $VAULT_DIR/.opencode/plugins/kgx-verify-finished.js"

    if [ -d "$REPO_DIR/skills/opencode/.opencode/command" ]; then
      mkdir -p "$VAULT_DIR/.opencode/command"
      cp "$REPO_DIR/skills/opencode/.opencode/command/"*.md "$VAULT_DIR/.opencode/command/"
      ok "installed commands -> $VAULT_DIR/.opencode/command/"
    fi

    cp "$REPO_DIR/skills/opencode/.opencode/skills/kgx-codebase/SKILL.md" \
       "$SKILLBASE/kgx-codebase/SKILL.md"
    ok "installed skill -> $SKILLBASE/kgx-codebase/SKILL.md"
    ;;

  codex)
    cp "$REPO_DIR/skills/codex/config.toml" "$VAULT_DIR/config.toml"
    ok "copied config.toml -> $VAULT_DIR/config.toml"
    cp "$REPO_DIR/skills/codex/AGENTS.md" "$VAULT_DIR/AGENTS.md"
    ok "copied AGENTS.md -> $VAULT_DIR/AGENTS.md"
    cp "$REPO_DIR/skills/codex/hooks.json" "$VAULT_DIR/hooks.json"
    ok "copied hooks.json -> $VAULT_DIR/hooks.json"
    ;;

  cursor)
    mkdir -p "$VAULT_DIR/.cursor/rules"

    cp "$REPO_DIR/skills/cursor/.cursor/rules/kgx.mdc" \
       "$VAULT_DIR/.cursor/rules/kgx.mdc"
    ok "copied rules -> $VAULT_DIR/.cursor/rules/kgx.mdc"

    TARGET_MCP="$VAULT_DIR/.cursor/mcp.json"
    if [ -f "$TARGET_MCP" ]; then
      if command -v jq &>/dev/null; then
        jq '.mcpServers.kgx = {"command": "kg", "args": ["mcp-server", "--transport", "stdio"]}' \
          "$TARGET_MCP" > "${TARGET_MCP}.tmp" && mv "${TARGET_MCP}.tmp" "$TARGET_MCP"
        ok "merged kgx entry into $TARGET_MCP"
      else
        warn "jq not found -- add kgx entry manually to $TARGET_MCP"
        info "  { \"mcpServers\": { \"kgx\": { \"command\": \"kg\", \"args\": [\"mcp-server\", \"--transport\", \"stdio\"] } } }"
      fi
    else
      mkdir -p "$(dirname "$TARGET_MCP")"
      cp "$REPO_DIR/skills/cursor/.cursor/mcp.json" "$TARGET_MCP"
      ok "created $TARGET_MCP"
    fi
    ;;

  zcode)
    cp "$REPO_DIR/skills/zcode/.mcp.json" "$VAULT_DIR/.mcp.json"
    ok "copied .mcp.json -> $VAULT_DIR/.mcp.json (kgx + codebase-memory-mcp, stdio)"
    for s in kgx kgx-codebase kgx-codebase-index; do
      mkdir -p "${HOME}/.zcode/skills/$s"
      cp "$REPO_DIR/skills/zcode/.zcode/skills/$s/SKILL.md" "${HOME}/.zcode/skills/$s/SKILL.md"
    done
    ok "mirrored kgx skills -> ~/.zcode/skills/"
    ;;
esac

# ── 5. Smoke-check MCP server ─────────────────────────────────────────────────
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
  warn "MCP server did not return expected JSON (may need a vault in cwd)"
  info "response: $RESPONSE"
fi

# ── 6. Summary ────────────────────────────────────────────────────────────────
step "Done -- kgx installed for agent: $AGENT"
cat <<EOF

  Binaries   : $BIN_DIR/kg, $BIN_DIR/rtk
  Vault      : $VAULT_DIR
  Agent      : $AGENT
  MCP        : kg mcp-server --transport stdio

  To verify config:
    cd $VAULT_DIR

  To test the vault:
    KGX_LLM=mock kg status --json
    KGX_LLM=mock kg ask "What is this vault?" --cite --json

  Add $BIN_DIR to your PATH if not already there:
    echo 'export PATH="\$HOME/.local/bin:\$PATH"' >> ~/.zshrc && source ~/.zshrc

EOF
