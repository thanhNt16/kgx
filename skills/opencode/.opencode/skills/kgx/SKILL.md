---
name: kgx
description: Use when working with a KGX knowledge vault: capture sources, extract atomic facts, ask graph questions, run dream consolidation, and review staged diffs.
---

# KGX Knowledge Graph

Use `kg` for Markdown vault work. The MCP server exposes the same six tools.

## Workflows
- Capture: `kg capture --from <file|-> --type doc`
- Extract: `kg extract --source <id> --intensity full`
- Ask: `kg ask "<question>" --cite [--scope global]`
- Consolidate: `kg dream --max-iterations 3`, then `kg review --approve all --ponytail-audit`
- Rebuild: `kg index --full --communities`

## Rules
- Never edit `raw/` destructively.
- Supersede or archive notes; never delete knowledge.
- Cite note ids in answers.

## MCP Tools
`search_notes`, `get_note`, `upsert_note`, `ask_question`, `capture_raw`, `dream_step`
