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
| `kgx:ingest` | Capture source + extract atomic facts |
| `kgx:capture` | Capture a raw source verbatim |
| `kgx:extract` | Extract facts/entities/decisions from a source |
| `kgx:index` | Rebuild the brain index with communities |
| `kgx:search` | Hybrid keyword + semantic search |
| `kgx:ask` | Answer a question with citations |
| `kgx:recall` | Retrieve an entity's graph neighborhood |
| `kgx:dream` | Run consolidation + review approved diffs |
| `kgx:refine` | Targeted dream: same passes, scoped subgraph, same review gate |
| `kgx:review` | Review staged dream diffs |
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
Capture a raw source and extract atomic facts from it.
```
kg capture --from <file|-> --type <doc|transcript|article|code>
kg extract --source <source_note_id> --intensity full
```

### kgx:capture
Capture raw source material verbatim (immutable).
```
kg capture --from <file|-> --type <doc|transcript|article|code>
```

### kgx:extract
Extract atomic facts, entities, and decisions from a captured source.
With a real LLM provider, extraction classifies entities as person/object/location/event and emits typed relations; `KGX_LLM=mock` yields deterministic untyped output.
```
kg extract --source <source_note_id> --intensity full
```

### kgx:index
Build or rebuild the SQLite brain index with vector embeddings and community detection.
Semantic search is on by default; set `KGX_EMBED=off` to disable vectors. The first `kg index` may download the embedding model.
```
kg index --full --communities
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
| `KGX_RERANK` | on / `off` / `mock` | on |
| `KGX_RERANK_MODEL` | `jina-turbo` / `bge-base` | `jina-turbo` |
| `KGX_RERANK_TOPK` | integer | `30` |

### kgx:ask
Ask a question using hybrid retrieval with note ID citations.
```
kg ask "<question>" --cite [--scope global]
```

### kgx:recall
Retrieve all notes within 1-2 hops of a named entity.
```
kg recall --entity "<entity name>"
```

### kgx:dream
Run full consolidation: dedup, contradiction detection, supersession, stale archival.
```
kg dream --max-iterations 3
kg review --approve all --ponytail-audit
```

### kgx:refine
Run targeted dream passes over a query, note, or tag scope; same passes, scoped subgraph, same review gate.
```
kg refine <query>|--note <id>|--tag <tag>
kg review --approve all --ponytail-audit
```

### kgx:review
Review staged dream diffs without running consolidation.
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
kg init [--template research|code|pkm|team] [--with-skills] [--okf] [--vault <path>]
```

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
- Capture: `kg capture --from <file|-> --type doc`
- Extract: `kg extract --source <id> --intensity full`
- Ask: `kg ask "<question>" --cite [--scope global]`
- Consolidate: `kg dream --max-iterations 3`, then `kg review --approve all --ponytail-audit`
- Refine: `kg refine <query>|--note <id>|--tag <tag>`, then review
- Rebuild: `kg index --full --communities`
- Graph: `kg graph --format cytoscape|graphml`
- Cron: `kg cron remove <name>`

## Rules
- Never edit `raw/` destructively.
- Supersede or archive notes; never delete knowledge.
- Cite note ids in answers.

## MCP Tools
`nl_query_memory`, `query_memory`, `deep_search_memory`, `get_note`, `ingest_conversation`, `ingest_file`, `ingest_url`, `upsert_note`, `dream_step`
