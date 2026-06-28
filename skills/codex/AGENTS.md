# KGX Agent Instructions

Use the `kg` CLI and the `kgx` MCP server when working in a KGX vault.

Commands:
- `kg capture --from - --type doc`
- `kg extract --source <id> --intensity full`
- `kg ask "<q>" --cite [--scope global]`
- `kg dream --max-iterations 3`
- `kg review --approve all --ponytail-audit`
- `kg index --full --communities`

Rules:
- `raw/` is immutable.
- Supersede or archive; never delete notes.
- Cite note ids.

MCP tools: search_notes, get_note, upsert_note, ask_question, capture_raw, dream_step.
