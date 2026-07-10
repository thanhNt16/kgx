# KGX Agent Instructions

Use the `kg` CLI and the `kgx` MCP server when working in a KGX vault.

> **LLM work is delegated to the agent harness.** Retrieval, indexing, and
> graph math run locally via `kg`/MCP; but **extraction, Q&A synthesis, and
> dream consolidation are done by you (the in-session model)** following the
> `kgx:` skills — never shell out to an LLM-calling `kg` command. Those
> commands (`kg ask`/`kg dream`/`kg refine`/`kg extract`, `--communities`,
> `--rerank-llm`) have been removed; their reasoning is the harness's job.

## Composite Verbs (kgx:)

| Verb | What it does |
|------|-------------|
| `kgx:ingest` | Capture source (file/folder/URL/conversation) + extract atomic facts (harness-driven) |
| `kgx:capture` | Capture a raw source verbatim (file/folder/URL/conversation) |
| `kgx:extract` | Extract facts/entities/decisions from a captured source (harness-driven) |
| `kgx:pole` | Extract POLE (Person/Object/Location/Event) graph from a captured source — harness-driven, reusable post-ingest step |
| `kgx:index` | Rebuild the brain index |
| `kgx:search` | Hybrid keyword + semantic search |
| `kgx:ask` | Answer a question with citations (harness-driven synthesis over retrieval) |
| `kgx:recall` | Retrieve an entity's graph neighborhood |
| `kgx:dream` | Consolidation (dedup/contradiction/supersession/staleness) — harness-driven, staged diffs applied via `kgx:review` |
| `kgx:review` | Apply staged dream diffs |
| `kgx:link` | Analyze and repair wikilinks |
| `kgx:graph` | Export graph as an interactive 3D HTML visualization |
| `kgx:status` | Show vault and brain status |
| `kgx:cron` | Manage scheduler jobs, including remove |
| `kgx:init` | Scaffold a new vault |
| `kgx:ship` | Create an OKF bundle for sharing |
| `kgx:sync` | Pull and merge remote changes |
| `kgx:codebase` | Search, trace, inspect codebase graph |
| `kgx:codebase-index` | Index repo into codebase-memory-mcp graph |

### kgx:ingest
Capture a raw source and extract atomic facts. **Extraction is harness-driven**
— the agent reads the captured source and writes facts via `upsert_note`; do
not shell out to `kg extract`.
```
kg capture --from <file|folder|-> [--ext md,txt] --type doc   # or ingest_file({path|content})
# then: agent extracts facts -> upsert_note per atomic fact (see kgx:extract)
kg index --full
```

### kgx:capture
Capture raw source material verbatim (immutable). Supports a file, a folder
(walked recursively, `--ext` filters extensions), `-` for stdin.
```
kg capture --from <file|folder|-> [--ext md,txt,markdown,mdx] [--type doc]
```

### kgx:extract
Extract atomic facts, entities, and decisions from a captured source.
**Harness-driven**: the agent is the extractor (no external LLM provider
needed in-session). See the `kgx:extract` skill for the full methodology.

### kgx:index
Build or rebuild the SQLite brain with embeddings.
Semantic search is on by default; set `KGX_EMBED=off` to disable vectors. The first `kg index` may download the embedding model.
```
kg index --full
```

### kgx:search
Search the brain with keyword, semantic, or hybrid mode.
```
kg search <query> [--mode keyword|semantic|hybrid] [--limit <n>] [--rerank-graph]
```

| Env var | Values | Default |
|---|---|---|
| `KGX_EMBED` | `fastembed` / `minilm` / `mock` / `off` | `fastembed` |
| `KGX_SPARSE` | on / `off` / `mock` | on |
| `KGX_RERANK` | on / `off` / `mock` | off |
| `KGX_RERANK_MODEL` | `jina-turbo` / `bge-base` | `jina-turbo` |
| `KGX_RERANK_TOPK` | integer | `30` |

### kgx:ask
Answer a question using hybrid retrieval with citations. **Synthesis is
harness-driven** — the agent reasons over retrieved context; `kg ask` was removed.
See the `kgx:ask` skill.

### kgx:recall
Retrieve notes within 1-2 hops of a named entity.
```
kg recall --entity "<entity name>"
```

### kgx:dream
Consolidation (dedup, contradiction, supersession, stale archival).
**Judgment passes are harness-driven** — the agent computes diffs and writes
`.brain/.kg/staged_diffs.json`, then `kg review` applies them. `kg dream` was
removed; see the `kgx:dream` skill for the staged-diff schema. Pure-heuristic
candidate surfacing (orphans/stale/open-questions) is available via the
`dream_step` MCP tool.

### kgx:review
Apply staged dream diffs from `.brain/.kg/staged_diffs.json`.
```
kg review [--approve all|--reject] [--ponytail-audit]
```

### kgx:link
Analyze note links and repair broken wikilinks.
```
kg link [--fix]
```

### kgx:graph
Export the vault graph. The `html` format produces an interactive 3D WebGL visualization.
```
kg graph --format html|cytoscape|graphml|mermaid|dot|obsidian
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
Scaffold a new KGX vault with templates.
```
kg init [--template research|code|pkm|team] [--with-skills] [--okf] [--vault <path>] [--migrate]
```
Knowledge content (`raw/`, `notes/`, `index.md`, `log.md`, `.kg/`, `CLAUDE.md`) is created
inside a `.brain/` directory; agent/tooling config (`.mcp.json`, `.claude/`, `.codex/`,
`.cursor/`, `.opencode/`, `.kgx/`, `AGENTS.md`, `config.toml`, `opencode.json`, `.gitignore`)
stays at the enclosing project root so agents can discover it. Use `--migrate` to relocate
a legacy root-level vault into `.brain/`.

### kgx:ship
Create a portable OKF bundle.
```
kg ship --out <bundle.okf.tar.gz>
```

### kgx:sync
Pull remote changes and reindex.
```
kg sync
```

## Commands
- `kg capture --from <file|folder|-> [--ext md,txt] --type doc`
- `kg review --approve all --ponytail-audit`
- `kg index --full`
- `kg graph --format cytoscape|graphml`
- `kg cron remove <name>`
- `kg query --entity-type person`

Removed (LLM work now done by the agent harness, not an external provider):
`kg ask`, `kg dream`, `kg refine`, `kg extract`, `kg index --communities`,
`kg search --rerank-llm`. Use the corresponding `kgx:` skills instead.

## Rules
- `raw/` (under `.brain/`) is immutable.
- Supersede or archive; never delete notes.
- Cite note ids.

MCP tools: nl_query_memory, query_memory, deep_search_memory, get_note, ingest_conversation, ingest_file, ingest_url, upsert_note, dream_step.
