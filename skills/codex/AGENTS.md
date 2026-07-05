# KGX Agent Instructions

Use the `kg` CLI and the `kgx` MCP server when working in a KGX vault.

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
Capture a raw source and extract atomic facts.
```
kg capture --from <file|-> --type doc
kg extract --source <source_note_id> --intensity full
```

### kgx:capture
Capture raw source material verbatim (immutable).
```
kg capture --from <file|-> --type doc
```

### kgx:extract
Extract atomic facts, entities, and decisions from a captured source.
With a real LLM provider, extraction classifies entities as person/object/location/event and emits typed relations; `KGX_LLM=mock` yields deterministic untyped output.
```
kg extract --source <source_note_id> --intensity full
```

### kgx:index
Build or rebuild the SQLite brain with embeddings and communities.
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
| `KGX_RERANK` | on / `off` / `mock` | off |
| `KGX_RERANK_MODEL` | `jina-turbo` / `bge-base` | `jina-turbo` |
| `KGX_RERANK_TOPK` | integer | `30` |

### kgx:ask
Ask a question using hybrid retrieval with citations.
```
kg ask "<question>" --cite [--scope global]
```

### kgx:recall
Retrieve notes within 1-2 hops of a named entity.
```
kg recall --entity "<entity name>"
```

### kgx:dream
Run full consolidation: dedup, contradiction, supersession, stale archival.
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
Scaffold a new KGX vault with templates.
```
kg init [--template research|code|pkm|team] [--with-skills] [--okf] [--vault <path>]
```

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
- `kg capture --from - --type doc`
- `kg extract --source <id> --intensity full`
- `kg ask "<q>" --cite [--scope global]`
- `kg dream --max-iterations 3`
- `kg refine <query>|--note <id>|--tag <tag>`
- `kg review --approve all --ponytail-audit`
- `kg index --full --communities`
- `kg graph --format cytoscape|graphml`
- `kg cron remove <name>`

## Rules
- `raw/` is immutable.
- Supersede or archive; never delete notes.
- Cite note ids.

MCP tools: nl_query_memory, query_memory, deep_search_memory, get_note, ingest_conversation, ingest_file, ingest_url, upsert_note, dream_step.
