# KGX User Manual

Local-first AI-managed knowledge graph. Turn a folder of Markdown files into a queryable, linkable, version-controlled knowledge base.

---

## 1. Installation

### From source (macOS / Linux)

```bash
# Prerequisites: Rust 1.78+, Python 3.9+
git clone https://github.com/thanhNt16/kgx
cd kgx
cargo build --release

# Binary at target/release/kg — add to PATH:
export PATH="$PWD/target/release:$PATH"
```

### Via install script

```bash
curl -fsSL https://raw.githubusercontent.com/thanhNt16/kgx/main/install.sh | bash
# Installs to ~/.local/bin/kg
```

### Verify

```bash
kg --version
kg --help
```

---

## 2. Quick Start

### Create a vault

```bash
kg init --template research --vault ~/my-vault
```

Templates: `research` | `code` | `pkm` (default) | `team`

This creates a vault at `~/my-vault/.brain/` with the canonical structure:

```
.brain/
├── CLAUDE.md          ← agent contract
├── index.md           ← map of maps
├── log.md             ← changelog
├── raw/               ← immutable sources
├── notes/
│   ├── facts/
│   ├── entities/
│   ├── decisions/
│   ├── experiences/
│   ├── moc/
│   ├── sources/
│   ├── questions/
│   └── archived/
└── .kg/               ← git-ignored, derived
    ├── brain.sqlite
    ├── meta.json
    └── metrics.log
```

### Add content

```bash
# Capture a raw source (immutable copy)
kg capture --from meeting-notes.md --type doc

# Or capture a URL (BFS crawl)
kg capture --from https://example.com/docs --depth 1 --max-pages 20
```

### Index

```bash
kg index --full
```

### Query

```bash
kg query --entity-type person
kg query --tag infra
kg query --note-type decision
```

### Search

```bash
kg search "connection pooling" --mode hybrid
kg search "Postgres migration" --rerank-graph
```

### Explore

```bash
# Entity neighborhood
kg recall --entity "Postgres" --relations

# Graph visualization
kg graph --format html --out graph.html
# Opens an interactive 3D WebGL graph in your browser
```

### Status

```bash
kg status
```

---

## 3. Vault Structure

### Note types

Each note is a Markdown file with YAML frontmatter. Place them in the corresponding subdirectory under `.brain/notes/`.

| Type | Directory | Purpose | Example |
|------|-----------|---------|---------|
| `fact` | `notes/facts/` | Atomic facts (Zettelkasten: one fact per note) | `f-postgres-primary.md` |
| `entity` | `notes/entities/` | People, systems, concepts | `postgres.md` |
| `decision` | `notes/decisions/` | Architecture Decision Records | `adr-001-datastore.md` |
| `experience` | `notes/experiences/` | Retrospectives, lessons learned | — |
| `moc` | `notes/moc/` | Maps of Content (curated indexes) | `datastore-moc.md` |
| `source` | `notes/sources/` | Captured source references | `s-pgdocs.md` |
| `question` | `notes/questions/` | Open questions, unknowns | `q-sync-strategy.md` |

### Frontmatter conventions

```yaml
---
type: fact                      # Required: one of the note types above
id: 01FACT01POSTGRESPRIMARY00   # Recommended: ULID or descriptive ID
title: "Postgres is the primary datastore"  # Required
status: active                  # Optional: active | archived | superseded
valid_from: "2026-01-15"        # Optional: ISO date for bitemporality
valid_to:                        # Optional: supersession date
source: "[[raw/2026-01-15-arch-review]]"  # Provenance wikilink
confidence: high                # Optional: high | medium | low
tags: [infra, datastore]        # Optional: filtering
links: ["[[Postgres]]"]         # Optional: explicit links
entity_type: system             # Required for entity notes
---
```

### Links

Use `[[Wikilinks]]` to connect notes:

```markdown
The primary datastore is [[Postgres]]. [[Billing Service]] depends on it.
```

The indexer resolves wikilinks to edges in the graph:
- `[[Title]]` → resolved by note title
- `[[id]]` → resolved by note ID
- `[[path/to/note]]` → resolved by relative path stem
- `[[raw/2026-01-15-arch-review]]` → resolved with raw/ prefix stripping

### MOC notes

Maps of Content are curated indexes that group related notes:

```markdown
---
type: moc
id: 01MOC01DATASTOREMOC000000
title: "Datastore MOC"
---
## Facts
- [[Postgres is the primary datastore]]
- [[CockroachDB is the primary datastore]]

## Entities
- [[Postgres]]
- [[CockroachDB]]

## Decisions
- [[ADR-001: Postgres as primary datastore]]
```

---

## 4. Daily Workflows

### Ingest → Index → Query

```
1. Capture raw source     kg capture --from doc.md
2. Write entity/fact notes    (edit files in notes/)
3. Rebuild index           kg index --full
4. Query / Search          kg query --tag infra
```

### Capture sources

```bash
# File
kg capture --from meeting-notes.md

# Directory (walks recursively, filters by --ext)
kg capture --from ./docs/ --ext md,pdf,docx --type doc

# URL (BFS crawl with depth limit)
kg capture --from https://docs.example.com --depth 2 --max-pages 50

# Stdin
cat notes.md | kg capture --from -
```

Supported extensions: `md,txt,markdown,mdx,pdf,docx,pptx,odt,epub,html,htm,xlsx,xls`

### Index

```bash
# Full rebuild (fastest for large changes)
kg index --full

# Incremental (only changed notes by mtime)
kg index --incremental

# Rebuild vectors only
kg index --rebuild-vectors
```

The indexer:
1. Scans all `.md` files in `notes/` and `raw/`
2. Derives edges from wikilinks, frontmatter `links`, `supersedes`, `source`
3. Generates embeddings (384-dim via configurable embedder)
4. Populates SQLite brain with: FTS5 full-text, vec0 vector KNN, BM25, tags
5. Runs PageRank for graph-aware relevance

### Query

```bash
# By entity type
kg query --entity-type person
kg query --entity-type system

# By note type
kg query --note-type decision

# By tag
kg query --tag infra

# By status
kg query --status active

# Combined
kg query --entity-type person --tag engineering --limit 50
```

### Search

```bash
# Keyword (BM25/FTS5)
kg search "connection pooling" --mode keyword

# Semantic (vector KNN)
kg search "database migration strategy" --mode semantic

# Hybrid (fused RRF of keyword + semantic + tags)
kg search "Postgres migration CockroachDB" --mode hybrid

# Graph-reranked (two-stage: retrieve → PageRank rerank)
kg search "billing service" --rerank-graph
```

### Recall (entity neighborhood)

```bash
# 1-hop neighbors
kg recall --entity "Postgres"

# With typed relationship edges
kg recall --entity "Postgres" --relations
```

### Graph visualization

```bash
# Interactive 3D HTML (WebGL, Three.js)
kg graph --format html --out graph.html

# Other formats
kg graph --format cytoscape   # Cytoscape.js JSON
kg graph --format graphml     # GEXF-compatible XML
kg graph --format mermaid      # Mermaid.js flowchart
kg graph --format dot          # Graphviz DOT
kg graph --format obsidian     # Obsidian graph JSON
```

### Link analysis

```bash
# Find broken wikilinks
kg link

# Show orphan notes
kg link --orphans

# Suggest repairs
kg link --suggest

# Auto-fix with confidence > 0.8
kg link --fix
```

### Validate vault

```bash
kg validate                  # Basic integrity
kg validate --okf            # OKF conformance
kg validate --links          # Link health
kg validate --frontmatter    # Frontmatter schema
kg validate --bitemporal     # Temporal consistency
```

---

## 5. Consolidation (Dream)

Dream is KGX's consolidation pass — deduplication, contradiction detection, supersession, and archival. It is **agent-harness driven**, meaning the AI agent computes diffs and stages them for human review.

### Workflow

```bash
# 1. Run dream_step via MCP (agent-driven)
#    Produces staged diffs in .brain/.kg/staged_diffs.json

# 2. Review staged diffs
kg review
kg review --approve all        # Approve everything
kg review --approve <diff-id>  # Approve one diff
kg review --reject <diff-id>   # Reject one diff

# 3. Interactive review
kg review --interactive --ponytail-audit   # guided step-through
```

What dream checks:
- **Dedup**: Two notes asserting the same fact with different IDs
- **Contradiction**: Conflicting facts (e.g., "Postgres is primary" vs "CockroachDB is primary")
- **Supersession**: Newer fact replaces older fact (sets `valid_to` on old)
- **Staleness**: Facts past their `valid_to` date, or sources that haven't been referenced
- **Open questions**: Questions with related facts that may answer them

---

## 6. Environment Variables

| Variable | Values | Default | Purpose |
|----------|--------|---------|---------|
| `KGX_LLM` | `mock` / provider | `mock` | LLM provider for extraction/QA |
| `KGX_EMBED` | `fastembed` / `minilm` / `mock` / `off` | `fastembed` | Embedding model |
| `KGX_SPARSE` | `on` / `off` / `mock` | `on` | Sparse retrieval (SPLADEv2) |
| `KGX_RERANK` | `on` / `off` / `mock` | `off` | Cross-encoder reranking |
| `KGX_RERANK_MODEL` | `jina-turbo` / `bge-base` | `jina-turbo` | Reranker model |
| `KGX_RERANK_TOPK` | integer | `30` | Documents sent to reranker |
| `KGX_BIN_DIR` | path | `~/.local/bin` | Binary install directory |
| `KGX_REPO` | user/repo | `thanhNt16/kgx` | GitHub repo for install |

### Environment for benchmarks / headless

```bash
export KGX_LLM=mock
export KGX_EMBED=mock   # No real embeddings, FNV hash 384d
kg index --full
```

---

## 7. MCP Server

KGX exposes an MCP (Model Context Protocol) server for AI agent integration:

```bash
# Start the MCP stdio server
kg mcp-server --transport stdio

# HTTP transport
kg serve --transport http --port 8765
```

### MCP tools

| Tool | Purpose |
|------|---------|
| `nl_query_memory` | Natural language query with hybrid search |
| `query_memory` | Structured query by type/tag/status |
| `deep_search_memory` | Progressive disclosure: first pass → cluster → drill |
| `get_note` | Fetch a note by ID |
| `ingest_conversation` | Incremental conversation capture |
| `ingest_file` | Ingest file/folder into vault |
| `ingest_url` | Fetch URL and ingest |
| `upsert_note` | Create or update a note |
| `dream_step` | Run one bounded dream iteration |

### AI agent integration

KGX ships with skill files and MCP configs for Claude, OpenCode, Codex, Cursor, and ZCode:

```bash
# Auto-install agent integration
./dev-install.sh --agent claude --vault ~/my-vault
# or --agent opencode | codex | cursor | zcode
```

---

## 8. Agent Skills (kgx: verbs)

When working in a KGX vault from an AI agent (OpenCode, Claude Code, Codex, Cursor), use these composite verbs. **LLM work is delegated to the agent harness** — extraction, Q&A synthesis, and dream consolidation run in-session; retrieval, indexing, and graph math run locally via `kg`/MCP.

| Verb | What it does |
|------|-------------|
| `kgx:ingest` | Capture source + extract atomic facts (harness-driven) |
| `kgx:capture` | Capture a raw source verbatim |
| `kgx:extract` | Extract facts/entities/decisions from a captured source |
| `kgx:pole` | Extract POLE (Person/Object/Location/Event) graph |
| `kgx:index` | Rebuild the brain index |
| `kgx:search` | Hybrid keyword + semantic search |
| `kgx:ask` | Answer a question with citations (harness-driven synthesis) |
| `kgx:recall` | Retrieve an entity's graph neighborhood |
| `kgx:dream` | Consolidation (dedup/contradiction/supersession) — staged diffs |
| `kgx:review` | Apply staged dream diffs |
| `kgx:link` | Analyze and repair wikilinks |
| `kgx:graph` | Export graph as interactive 3D HTML |
| `kgx:status` | Show vault and brain status |
| `kgx:cron` | Manage scheduler jobs |
| `kgx:init` | Scaffold a new vault |
| `kgx:ship` | Create an OKF bundle |
| `kgx:sync` | Pull and merge remote changes |

### kgx:ingest

Capture a raw source and extract atomic facts. The agent reads the captured source and writes facts via `upsert_note` — do not shell out to `kg extract`.

```
kg capture --from <file|folder|-> [--ext md,txt] --type doc
# agent: read source → upsert_note per atomic fact
kg index --full
```

### kgx:capture

Capture raw source material verbatim (immutable). Supports file, folder, stdin.

```
kg capture --from <file|folder|-> [--ext md,txt,markdown,mdx] [--type doc]
```

### kgx:extract

Extract atomic facts, entities, and decisions from a captured source. **Harness-driven**: the agent is the extractor. Read the source note, derive atomic facts (one claim per note with confidence and links), write each via `upsert_note`.

### kgx:pole

Extract a structured POLE (Person/Object/Location/Event) graph from a captured source:
1. Agent reads the captured source
2. Identifies persons, objects, locations, events
3. Creates entity notes with `entity_type` and typed relationship links
4. Runs `kg index --full`

### kgx:index

Build or rebuild the SQLite brain.

```
kg index --full
```

Semantic search is on by default. Set `KGX_EMBED=off` to disable vectors.

### kgx:search

```
kg search <query> [--mode keyword|semantic|hybrid] [--limit <n>] [--rerank-graph]
```

### kgx:ask

Answer a question using hybrid retrieval with citations. **Synthesis is harness-driven** — the agent retrieves context via `nl_query_memory` / `deep_search_memory`, then synthesizes the answer itself. `kg ask` was removed.

### kgx:recall

```
kg recall --entity "<entity name>"
```

Retrieves notes within 1-2 hops of a named entity.

### kgx:dream

Consolidation (dedup, contradiction, supersession, stale archival). **Judgment passes are harness-driven** — the agent computes diffs and writes `.brain/.kg/staged_diffs.json`, then `kg review` applies them. `kg dream` was removed. Use `dream_step` MCP tool for pure-heuristic candidate surfacing (orphans, stale, open questions).

### kgx:review

Apply staged dream diffs.

```
kg review [--approve all|--reject] [--ponytail-audit]
```

### kgx:link

```
kg link [--fix]
```

### kgx:graph

```
kg graph --format html|cytoscape|graphml|mermaid|dot|obsidian
```

### kgx:status

```
kg status [--json]
```

### kgx:cron

```
kg cron list
kg cron remove <name>
```

### kgx:init

```
kg init [--template research|code|pkm|team] [--with-skills] [--okf] [--vault <path>] [--migrate]
```

Knowledge content (`raw/`, `notes/`, `index.md`, `log.md`, `.kg/`, `CLAUDE.md`) is created inside `.brain/`. Agent/tooling config (`.mcp.json`, `.claude/`, `.codex/`, `.cursor/`, `.opencode/`, `.kgx/`, `AGENTS.md`, `config.toml`, `opencode.json`, `.gitignore`) stays at the project root.

### kgx:ship

```
kg ship --out <bundle.okf.tar.gz>
```

### kgx:sync

```
kg sync
```

### Rules

- `raw/` (under `.brain/`) is immutable.
- Supersede or archive; never delete notes.
- Cite note IDs in answers.
- Extraction, Q&A, and consolidation use these verbs — never shell out to `kg ask`, `kg dream`, `kg refine`, `kg extract`, `kg index --communities`, or `kg search --rerank-llm`.

---

## 9. Performance

Benchmark results (10,000 nodes + 30,000 edges, MacBook Apple Silicon):

| Operation | Time |
|-----------|------|
| Index (warm, excl. startup) | **6.2s** (0.62ms/note) |
| Index (cold, first run) | ~38s (includes ~32s startup overhead) |
| Query by entity type | **170-220ms** |
| Recall (entity neighborhood) | **170ms** |
| Status | **200ms** |
| Bulk INSERT 10k notes | 12ms |
| Bulk INSERT 30k edges | 13ms |

The indexer is I/O bound on vec0 vector inserts (~90% of time). Bulk notes+edges INSERT completes in ~25ms at 10k scale. Queries scale O(n) with the graph size.

---

## 10. Architecture

```
┌──────────────┐    ┌──────────────┐    ┌─────────────────┐
│  CLI (kgx)   │    │  MCP Server  │    │   Agent Harness │
│  capture     │    │  tools:      │    │   (Claude/Codex)│
│  index       │◄──►│  query/search│◄──►│   kgx:ask       │
│  query       │    │  recall      │    │   kgx:extract   │
│  search      │    │  ingest      │    │   kgx:dream     │
│  recall      │    │  dream_step  │    └─────────────────┘
│  graph       │    └──────┬───────┘
│  status      │           │
└──────┬───────┘           │
       │                   │
       ▼                   ▼
┌─────────────────────────────────────────┐
│           Markdown Vault (.brain/)      │
│  raw/ → notes/ → .kg/brain.sqlite      │
│                                         │
│  Brain layers:                          │
│  • Vector KNN (sqlite-vec, 384-dim)    │
│  • BM25 / FTS5 full-text               │
│  • Tags-in-LIKE                         │
│  • Tag-frequency expansion              │
│  • Entity NER                           │
│  • Leiden communities                   │
│  • Personalized PageRank (HippoRAG)    │
│  • Reciprocal Rank Fusion              │
└─────────────────────────────────────────┘
```

### Crate map

| Crate | Purpose |
|-------|---------|
| `kgx-core` | Types, traits, errors |
| `kgx-vault` | Vault scanning, note parsing |
| `kgx-graph` | SQLite brain, index, query, vec0 |
| `kgx-retrieval` | Hybrid search, reranking, PageRank |
| `kgx-llm` | LLM + embedder abstraction |
| `kgx-extract` | POLE extraction |
| `kgx-dream` | Consolidation logic |
| `kgx-mcp` | MCP server + tools |
| `kgx-cli` | CLI entry point |
| `kgx-convert` | PDF/Excel/DOCX conversion |
| `kgx-viz` | 3D graph visualization |
| `kgx-cron` | Scheduled job management |
| `kgx-okf` | OKF bundle format |

---

## 11. Troubleshooting

| Issue | Fix |
|-------|-----|
| `kg: command not found` | Add `~/.local/bin` to `PATH` or use full path |
| No vault found | Run `kg init --vault <path>` first, or `kg init --migrate` for legacy vaults |
| Slow index first run | Expected — first run includes SQLite prepare + model download (~30s). Subsequent runs are 10-100x faster. |
| Embedding model download | FastEmbed downloads models on first use. Set `KGX_EMBED=mock` to skip. |
| Pandoc not found for DOCX | Run `install.sh` which downloads pandoc to `~/.local/bin/pandoc-kgx` |
| Gatekeeper on macOS | `xattr -d com.apple.quarantine ~/.local/bin/kg` |
| `brain.sqlite` corruption | Delete `.kg/` dir and re-run `kg index --full` |
