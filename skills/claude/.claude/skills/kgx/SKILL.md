---
name: kgx
description: Use when working with a KGX knowledge vault: capture sources, extract atomic facts, ask graph questions, run dream consolidation, and review staged diffs.
disable-model-invocation: true
---

# KGX Knowledge Graph

Use `kg` for Markdown vault work. The MCP server exposes the same MCP tools.

## Composite Verbs (kgx:)

| Verb | What it does |
|------|-------------|
| `kgx:ingest` | Capture source (file/folder/URL/conversation) + extract atomic facts (harness-driven) |
| `kgx:capture` | Capture a raw source verbatim |
| `kgx:extract` | Extract facts/entities/decisions from a source (harness-driven) |
| `kgx:index` | Rebuild the brain index |
| `kgx:search` | Hybrid keyword + semantic search |
| `kgx:ask` | Answer a question with citations (harness-driven synthesis over retrieval) |
| `kgx:recall` | Retrieve an entity's graph neighborhood |
| `kgx:dream` | Consolidation (dedup/contradiction/supersession/staleness) — harness-driven, applied via `kgx:review` |
| `kgx:review` | Apply staged consolidation diffs |
| `kgx:link` | Analyze and repair wikilinks |
| `kgx:graph` | Export graph as HTML, Cytoscape, GraphML, Mermaid, DOT, or Obsidian Canvas |
| `kgx:status` | Show vault and brain status |
| `kgx:cron` | Manage scheduler jobs, including remove |
| `kgx:init` | Scaffold a new vault |
| `kgx:ship` | Create an OKF bundle for sharing |
| `kgx:sync` | Pull and merge remote changes |
| `kgx:codebase` | Search, trace, inspect codebase graph |
| `kgx:codebase-index` | Index repo into codebase-memory-mcp graph |

### kgx:ingest
Capture a raw source and extract atomic facts from it. **Extraction is
harness-driven** — see the `kgx:ingest` command skill (capture, then the agent
writes facts via `upsert_note`; do not shell out to `kg extract`).
```
kg capture --from <file|folder|-> [--ext md,txt] --type <doc|transcript|article|code>
# then agent extracts facts -> upsert_note per atomic fact
kg index --full
```

### kgx:capture
Capture raw source material verbatim (immutable). Accepts a file, a folder
(walked recursively), or `-` for stdin.
```
kg capture --from <file|folder|-> [--ext md,txt,markdown,mdx] --type <doc|transcript|article|code>
```

### kgx:extract
Extract atomic facts, entities, and decisions from a captured source.
**Harness-driven**: the agent is the extractor (no external LLM provider
needed in-session). See the `kgx:extract` command skill for the methodology.

### kgx:index
Build or rebuild the SQLite brain index with vector embeddings.
Semantic search is on by default; set `KGX_EMBED=off` to disable vectors. The first `kg index` may download the embedding model.
```
kg index --full
```

### kgx:search
Search the brain with keyword, semantic, or hybrid mode.
```
kg search <query> [--mode keyword|semantic|hybrid] [--limit <n>]
```

| Env var | Values | Default |
|---|---|---|
| `KGX_EMBED` | `fastembed` / `minilm` / `mock` / `off` | `fastembed` |
| `KGX_SPARSE` | on / `off` / `mock` | on |
| `KGX_RERANK` | on / `off` / `mock` | off |
| `KGX_RERANK_MODEL` | `jina-turbo` / `bge-base` | `jina-turbo` |
| `KGX_RERANK_TOPK` | integer | `30` |

### kgx:ask
Ask a question using hybrid retrieval with note ID citations. **Synthesis is
harness-driven** — retrieve via `nl_query_memory`/`deep_search_memory` and
answer yourself; `kg ask` was removed. See the `kgx:ask` command skill.

### kgx:recall
Retrieve all notes within 1-2 hops of a named entity.
```
kg recall --entity "<entity name>"
```

### kgx:dream
Consolidation (dedup, contradiction, supersession, stale archival).
**Judgment passes are harness-driven** — you compute the diffs and write
`.brain/.kg/staged_diffs.json`, then `kg review` applies them. `kg dream` was
removed; see the `kgx:dream` command skill for the staged-diff schema.
LLM-free candidate surfacing (orphans/stale/open-questions) is available via
the `dream_step` MCP tool.

### kgx:review
Apply staged consolidation diffs from `.brain/.kg/staged_diffs.json`.
```
kg review [--approve all|--reject] [--ponytail-audit]
```

### kgx:link
Analyze note links and repair broken wikilinks.
```
kg link [--fix]
```

### kgx:graph
Export the vault graph.
```
kg graph --format cytoscape|graphml
```

### kgx:status
Show vault structure, brain size, and index freshness.
```
kg status [--json]
```

### kgx:cron
Manage scheduled jobs.
```
kg cron list
kg cron remove <name>
```

### kgx:init
Scaffold a new KGX vault with templates and optional OKF conformance.
```
kg init [--template research|code|pkm|team] [--with-skills] [--okf] [--vault <path>] [--migrate]
```
Knowledge content (`raw/`, `notes/`, `index.md`, `log.md`, `.kg/`, `CLAUDE.md`) is created
inside `.brain/`; agent config (`.mcp.json`, `.claude/`, etc.) stays at the project root.
`--migrate` relocates a legacy root-level vault into `.brain/`.

### kgx:ship
Create a portable OKF bundle from the vault.
```
kg ship --out <bundle.okf.tar.gz>
```

### kgx:sync
Pull remote changes and reindex.
```
kg sync
```

## Quick Workflows
- Capture: `kg capture --from <file|folder|-> [--ext md,txt] --type doc`
- Extract: run the `kgx:ingest` / `kgx:extract` skill — the agent writes facts via `upsert_note`
- Ask: run the `kgx:ask` skill — retrieve (`nl_query_memory`/`deep_search_memory`), then synthesize
- Consolidate: run the `kgx:dream` skill (write `.brain/.kg/staged_diffs.json`), then `kg review --approve all --ponytail-audit`
- Rebuild: `kg index --full`
- Graph: `kg graph --format cytoscape|graphml`
- Cron: `kg cron remove <name>`

## Rules
- Never edit `raw/` destructively.
- Supersede or archive notes; never delete knowledge.
- Cite note ids in answers.

## MCP Tools
`nl_query_memory`, `query_memory`, `deep_search_memory`, `get_note`, `ingest_conversation`, `ingest_file`, `ingest_url`, `upsert_note`, `dream_step`
