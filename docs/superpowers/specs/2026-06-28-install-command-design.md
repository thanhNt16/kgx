# Single Install Command for AI Agent Integration

**Date:** 2026-06-28  
**Status:** Approved

## Problem

Setting up kgx for a new AI coding agent requires multiple manual steps: build from
source, copy the binary, configure MCP, copy skills/rules, and each agent (Claude Code,
OpenCode, Codex, Cursor) has different config file formats and locations. The existing
`dev-install.sh` only supports Claude Code.

## Goal

A single shell command `./dev-install.sh [--agent <agent>]` that:

1. Builds `kg` from source
2. Installs the binary to `~/.local/bin/kg`
3. Initializes a vault at `--vault <path>` (default `~/kg-vault`)
4. Wires MCP, skills/rules, and agent config for the selected AI tool
5. Smoke-checks the MCP server
6. Defaults to Claude Code; supports claude, opencode, codex, cursor

## Per-Agent Wiring

| Agent | MCP Config | Skill / Rules | Config Files |
|-------|-----------|---------------|--------------|
| **claude** | `claude mcp add` (auto CLI) + `~/.claude/settings.json` | `~/.claude/skills/kgx/SKILL.md` | `.mcp.json` (root, optional) |
| **opencode** | `opencode.json` (root) | `.opencode/skills/kgx/SKILL.md` | `.opencode/plugins/kgx-verify-finished.js` |
| **codex** | `config.toml` (root) | `AGENTS.md` (root) | `hooks.json` (root) |
| **cursor** | `.cursor/mcp.json` (merge with existing) | `.cursor/rules/kgx.mdc` | — |

## Approach

Update `dev-install.sh` with:
- `--agent` flag (enum: claude, opencode, codex, cursor; default: claude)
- Per-agent case block for MCP registration + config file copying
- Directory creation as needed for each agent's config paths
- Existing build/install/smoke-check steps remain unchanged

## Files Changed

- `dev-install.sh` — rewrite with multi-agent support
- `README.md` — update install instructions to mention `--agent` flag
