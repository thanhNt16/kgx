# KGX — Local-First AI Knowledge Graph CLI

> *"Humans abandon wikis because maintenance grows faster than value. LLMs don't get bored."*  
> — Andrej Karpathy

**KGX** turns a plain Markdown + `[[wikilinks]]` vault into a living, queryable knowledge graph. The vault is your canonical, git-versioned source of truth. A disposable SQLite "brain" provides hybrid vector + graph + keyword retrieval, PageRank, and community summaries. AI agents ingest, extract, link, answer, and consolidate. You review via git diffs and Obsidian's native graph view.

> **[Full Report (HTML)](docs/kgx-final-report.html)** — consolidated benchmarks, test results, architecture, sprint simulation, and all documentation in one page.
>
> **[Battle Test Report (HTML)](docs/kgx-battletest-report.html)** — 36-sprint / 18-month engineering-team simulation, WITH-vs-WITHOUT-KGX benchmark (recall +239%, MRR +4926%, 25× less context), 4-harness compatibility audit, and every bug found & fixed during battle testing. Reproducible via `bench/gen_corpus.py` + `bench/bench.py`.

---

## Table of Contents

1. [Why KGX?](#why-kgx)
2. [Architecture at a Glance](#architecture-at-a-glance)
3. [Installation](#installation)
4. [MCP Server Setup](#mcp-server-setup)
5. [Skills Setup (Claude Code · Codex · Cursor)](#skills-setup)
6. [End-to-End Workflow](#end-to-end-workflow)
7. [All Commands](#all-commands)
8. [E2E Integration Test Results](#e2e-integration-test-results)
9. [KGX vs Without KGX](#kgx-vs-without-kgx)
10. [Sprint Simulation (2-week DataLake sprint)](#sprint-simulation)
11. [Configuration Reference](#configuration-reference)

---

## Why KGX?

| Problem (without KGX) | Solution (with KGX) |
|---|---|
| Raw notes pile up, never get structured | `kg capture` (file/folder/URL) + the `kgx:ingest` skill turn any source into atomic, linked facts — extraction is driven by your agent harness |
| Classic RAG re-derives answers from scratch every time | Persistent hybrid brain: vector + BM25 + tag expansion + entity graph + PageRank |
| Wikis rot because humans don't maintain them | The `kgx:dream` skill runs consolidation (dedup, contradiction, archival, link repair), staged and applied via `kg review` |
| Agent sessions burn tokens on large contexts | Hybrid retrieval sends only relevant context (verified: 371 tokens vs ~3,400 full-vault on 17-note fixture) |
| No standard format for sharing knowledge bundles | OKF-compatible: `kg ship` + `kg pull` for lossless round-trip bundles |
| Multi-hop questions fail basic RAG | HippoRAG-style Personalized PageRank over the graph |
| Notes trapped in one app | Markdown-canonical: works in Obsidian, VS Code, any editor |

---

## Architecture at a Glance

```
Markdown Vault (git)
  .brain/                  ← knowledge vault (created by `kg init`)
    raw/         ← immutable sources (never edited after capture)
    notes/
      facts/     ← atomic claims extracted by AI
      entities/  ← people, systems, concepts
      decisions/ ← ADRs
      experiences/
      moc/       ← maps of content
      questions/ ← open gaps, auto-closed when answered
    .kg/         ← derived, git-ignored
      brain.sqlite
      meta.json
      metrics.log

Brain Layers
  ┌──────────────────────────────────────────────────┐
  │  Vector KNN (sqlite-vec, 384-dim MiniLM)         │
  │  BM25 / FTS5 (porter-stemmed)                    │
  │  Tags-in-LIKE substring search                   │
  │  Tag-frequency-weighted expansion (k=300)        │
  │  Entity NER → 1-hop graph expansion              │
  │  Leiden community summaries (GraphRAG)            │
  │  Personalized PageRank (HippoRAG)                │
  └──────────────────────────────────────────────────┘
          ↓ Reciprocal Rank Fusion (multi-k)
      Hybrid ranked results
          ↓
      kgx:ask (harness) / kg recall / kg search
          ↓
      MCP tools → Claude Code / Codex / Cursor
```

**Crate map:**

| Crate | Role |
|---|---|
| `kgx-core` | Shared types (`Note`, `Frontmatter`, `KgError`, `ProposedDiff`) |
| `kgx-vault` | FS read/write, ULID generation, wikilink parsing |
| `kgx-okf` | OKF parse, validate, ship/pull bundle I/O |
| `kgx-graph` | SQLite brain: schema, index, embed, PPR, Leiden |
| `kgx-retrieval` | Hybrid search, RRF fusion, community summaries |
| `kgx-llm` | Provider trait (Claude, OpenAI, Ollama, mock) |
| `kgx-extract` | Raw → atomic facts/entities/decisions pipeline |
| `kgx-dream` | 7 consolidation passes: dedup, contradict, supersede, stale, resummarize, orphan, questions |
| `kgx-mcp` | JSON-RPC 2.0 stdio MCP server (9 tools) |
| `kgx-store` | BrainStore trait, multi-project BrainSet |
| `kgx-tokens` | Per-command token accounting, JSONL metrics |
| `kgx-rtk` | Shell-output compression wrapper + hook installer |
| `kgx-ponytail` | Prompt ladders with over-engineering audit rules |
| `kgx-cron` | systemd/launchd timer manager |
| `kgx-viz` | HTML/D3, Mermaid, DOT, Obsidian Canvas exporters |
| `kgx-docs` | Use-case HTML generator (tera templates) |
| `kgx-bench` | 3-arm corpus/gold/judge benchmark harness |
| `kgx-cli` | `kg` binary — clap commands, JSON output |
| `kgx-convert` | Document→markdown conversion (pandoc subprocess, PDF/Excel native Rust) |

---

## Installation

### Install from GitHub Releases

Every push to `main` publishes a release named `KGX 0.0.<run_number>`, and version tags publish releases such as `KGX 0.1.0`.

Install the latest release archive:

```bash
curl -fsSL https://raw.githubusercontent.com/thanhNt16/kgx/main/install.sh | bash
```

Install a specific release:

```bash
curl -fsSL https://raw.githubusercontent.com/thanhNt16/kgx/main/install.sh | KGX_VERSION=v0.0.1 bash
```

The installer downloads the matching `kgx-<version>-<platform>.zip` from GitHub Releases, installs the `kg` CLI to `~/.local/bin`, copies the bundled skill templates to `~/.kgx/skills`, and installs global skills/commands for Claude Code (`~/.claude/`) and OpenCode (`~/.config/opencode/`).

After installing:

```bash
export PATH="$HOME/.local/bin:$PATH"

mkdir ~/brain && cd ~/brain
kg init --with-skills --with-rtk
```

This gives you the full toolset:

| Component | How it is installed or used |
| --- | --- |
| CLI | `kg` is installed to `~/.local/bin` |
| MCP server | Run with `kg mcp-server --transport stdio` from inside a vault |
| Global skills/commands | Claude Code (`~/.claude/skills/`, `~/.claude/commands/`) and OpenCode (`~/.config/opencode/skills/`, `~/.config/opencode/command/`) |
| Skills and hooks | `kg init --with-skills --with-rtk` writes Claude Code, Codex, Cursor, OpenCode, ZCode, and shared hook files into the vault |

### Build from Source

Requires Rust 1.78+.

```bash
git clone https://github.com/thanhNt16/kgx
cd kgx
cargo build --release        # ~30s cold, 0.07s incremental
cp target/release/kg ~/.local/bin/kg
kg --version                 # kg 0.1.0
```

Binary size: 12.5 MB (statically linked, no runtime deps).

### One-Command Dev Install

The `dev-install.sh` script builds from source, installs the binary, scaffolds a vault, and wires MCP + skills/rules for your AI tool — all in one command:

```bash
# Default: Claude Code
./dev-install.sh

# Pick your agent:
./dev-install.sh --agent claude      # Claude Code (default)
./dev-install.sh --agent opencode    # OpenCode
./dev-install.sh --agent codex       # Codex CLI
./dev-install.sh --agent cursor      # Cursor
./dev-install.sh --agent zcode       # ZCode

# Custom vault location:
./dev-install.sh --agent cursor --vault ~/my-project
```

Per-agent, it installs:

| Agent | MCP Config | Skill / Rules | Config Files |
|-------|-----------|---------------|--------------|
| claude | `claude mcp add` (auto) | `~/.claude/skills/kgx/SKILL.md` | — |
| opencode | `opencode.json` | `.opencode/skills/kgx/SKILL.md` | `.opencode/command/kgx-*.md`, `.opencode/plugins/kgx-verify-finished.js` |
| codex | `config.toml` | `AGENTS.md` | `hooks.json` |
| cursor | `.cursor/mcp.json` (merged) | `.cursor/rules/kgx.mdc` | — |
| zcode | `.mcp.json` (stdio) | `~/.zcode/skills/` | installed via `./dev-install.sh --agent zcode` |

### Initialize a Vault

```bash
mkdir ~/brain && cd ~/brain

# Research vault (facts, entities, questions, sources)
kg init --template research

# Engineering team vault (decisions/ADRs, entities, moc)
kg init --template code

# Personal knowledge management (Zettelkasten)
kg init --template pkm

# Shared team vault with OKF conformance
kg init --template team --okf

# With skills files for AI tools
kg init --with-skills
```

This scaffolds a `.brain/` directory (knowledge content) plus agent/tooling config at the
enclosing project root:

```
.brain/                  ← knowledge vault (run kg commands from the project root)
├── CLAUDE.md            ← schema + prompt ladders (loaded by all AI tools)
├── index.md             ← OKF root index
├── log.md               ← append-only operation log
├── raw/                 ← place sources here (immutable after capture)
└── notes/
    ├── facts/
    ├── entities/
    ├── decisions/
    ├── moc/
    └── questions/

.mcp.json, .claude/, .codex/, .cursor/, .opencode/, .kgx/  ← agent config (project root)
```

> **Migrating a legacy vault?** If you have an older root-level vault (`raw/`, `notes/`,
> `.kg/` directly at the project root), run `kg init --migrate` to relocate it into
> `.brain/`. After migration, `kg` looks only in `.brain/`.

---

## MCP Server Setup

The MCP server exposes 9 tools over JSON-RPC 2.0 via stdio. **Run from inside your project directory** — it resolves the knowledge vault from `<cwd>/.brain`.

| Tool | What it does |
|---|---|
| `nl_query_memory` | Natural language hybrid search with Q&A |
| `query_memory` | Structured query with filters (type, tag, project, status) |
| `deep_search_memory` | Progressive disclosure search (cluster-then-drill) |
| `get_note` | Fetch a note by ID |
| `ingest_conversation` | Incremental conversation capture + finalize compilation |
| `ingest_file` | Ingest raw content immutably (idempotent by sha256) |
| `ingest_url` | Fetch URL content and ingest into the vault |
| `upsert_note` | Create or update a note |
| `dream_step` | Run one bounded dream consolidation pass |

### Verify MCP works (raw JSON-RPC over pipe)

```bash
cd ~/brain           # must be inside the vault
kg index --full      # build brain first

{ echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","clientInfo":{"name":"test","version":"0.1"}}}';
  echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}';
  sleep 0.3; } | kg mcp-server --transport stdio
```

Expected:
```json
{"id":1,"jsonrpc":"2.0","result":{"capabilities":{"tools":{}},"protocolVersion":"2024-11-05","serverInfo":{"name":"kgx","version":"0.1.0"}}}
{"id":2,"jsonrpc":"2.0","result":{"tools":[...]}}
```

### Claude Code

```bash
claude mcp add --transport stdio kgx -- kg mcp-server --transport stdio
```

Or add to `.claude/mcp.json` in your vault:

```json
{
  "mcpServers": {
    "kgx": {
      "command": "kg",
      "args": ["mcp-server", "--transport", "stdio"]
    }
  }
}
```

> The MCP server is registered at vault scope. Claude Code must be started from inside the vault directory.

### Codex

Add to `codex.toml` (or copy from `skills/codex/config.toml`):

```toml
[mcp_servers.kgx]
command = "kg"
args = ["mcp-server", "--transport", "stdio"]
```

### Cursor

Add to `.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "kgx": {
      "command": "kg",
      "args": ["mcp-server", "--transport", "stdio"]
    }
  }
}
```

---

## Skills Setup

Skills teach each AI tool how to work in a KGX vault. Run from inside your vault:

```bash
kg init --with-skills
```

This writes the appropriate context files for each tool.

### Claude Code — `CLAUDE.md`

Placed in the vault root. Contains:
- Note type schema and frontmatter fields
- Ponytail prompt ladders (extraction, dreaming, Q&A, review)
- Dreaming rules (when to supersede vs merge, confidence thresholds)
- `[[wikilink]]` conventions and typed link predicates
- MOC maintenance guidelines
- Review checklist for `kg review --ponytail-audit`

Claude Code reads this at session start and follows the ladders when calling `kg` commands or MCP tools.

**Example agent workflow in Claude Code:**

```
User: "Summarize what we decided about the auth architecture"

Claude Code:
1. Calls MCP tool: nl_query_memory("auth architecture decisions") → citations
2. Calls: kg recall --entity "Auth Service" → neighborhood context
3. Synthesizes with citations: "Based on [[decisions/adr-003-jwt-auth]] and [[entities/auth-service]]..."
```

### Codex — `AGENTS.md`

`skills/codex/AGENTS.md` contains the same behavioral contract for Codex:

```markdown
Use the `kg` CLI and the `kgx` MCP server when working in a KGX vault.

Commands:
- `kg capture --from <file|folder|-> [--ext md,txt] --type doc`
- `kg review --approve all --ponytail-audit`
- `kg index --full`
- `kg graph --format cytoscape|graphml`
- `kg cron remove <name>`
- Extraction, Q&A, and consolidation are **agent-harness driven** via the
  `kgx:ingest` / `kgx:extract` / `kgx:ask` / `kgx:dream` skills (no external
  LLM provider needed in-session).

Rules:
- raw/ (under `.brain/`) is immutable.
- Supersede or archive; never delete notes.
- Cite note IDs.
```

Copy `skills/codex/AGENTS.md` to your vault root as `AGENTS.md` for Codex to pick it up automatically.

### Cursor

Cursor uses `.cursor/mcp.json` for the MCP connection. The MCP connection gives Cursor direct access to all 9 KGX tools. Cursor's agent reads vault `CLAUDE.md` when it's in the project root.

---

## End-to-End Workflow

### 1. Capture a raw source

```bash
# From a file
kg capture --from meeting-notes-2026-06-27.md --type transcript

# From a folder (every text file captured as its own source)
kg capture --from docs/ --ext md,txt

# From stdin (pipe from clipboard, curl, etc.)
pbpaste | kg capture --from - --type doc
```

Actual output:
```json
{"ok":true,"command":"capture","data":{"kind":"doc","raw":"raw/2026-06-27-meeting-notes.md","source_note":"notes/sources/meeting-notes.md"},"elapsed_ms":0}
```

### 1b. Capture a document (PDF, Excel, Word, PPTX)

```bash
# PDF → auto-converted to markdown, then captured
kg capture --from report.pdf

# Excel → each sheet becomes a markdown table
kg capture --from budget.xlsx

# Word doc → pandoc converts to GitHub-flavored markdown
kg capture --from proposal.docx

# Directory with mixed formats — all converted automatically
kg capture --from docs/ --ext md,txt,pdf,docx,xlsx,pptx
```

Document formats are auto-detected by extension. Pandoc handles .docx, .pptx,
.odt, .epub, .html. PDF uses native Rust extraction. Excel uses native Rust
calamine. Install pandoc: `install.sh` bundles it automatically, or set
`$KGX_PANDOC` to point to a pandoc binary.

### 1c. Crawl a URL with depth

```bash
# Single page (default)
kg capture --from https://example.com/article

# Crawl same-domain links (depth 1 = page + direct links)
kg capture --from https://example.com --depth 1 --max-pages 50

# Deeper crawl
kg capture --from https://example.com --depth 2 --max-pages 100
```

URLs are fetched, converted to markdown via pandoc, and captured as individual
raw source notes. Same-domain filtering prevents crawling unrelated sites.
`--max-pages` caps the total. 500ms delay between fetches (configurable via
`KGX_CRAWL_DELAY_MS`).

### 2. Extract atomic notes (agent-harness driven)

Extraction is done by your agent (Claude Code / Codex / OpenCode), not an
external LLM provider — drive it via the `kgx:ingest` or `kgx:extract` skill:

1. `get_note` the captured source by its id.
2. The agent reads the body and derives atomic facts (one claim per note,
   confidence levels, named entities → `links`).
3. For each fact, `upsert_note({type:"fact", title, body, source:"[[raw/<stem>]]",
   confidence, links})`. Use `type:"decision"` for choices, `type:"entity"`
   for new named entities.
4. `kg index --full` so the new notes are searchable.

This creates `notes/facts/`, `notes/entities/`, `notes/decisions/` files — each
with `source:` provenance and `recorded_at:` timestamp.

### 3. Build the brain

```bash
# Full rebuild (deterministic — safe to repeat)
kg index --full
```

Semantic search is on by default. The first embedding-enabled `kg index` downloads the embedding model; set `KGX_EMBED=off` to disable vectors and run keyword-only.

Actual output (17-note vault):
```json
{"ok":true,"command":"index","data":{"nodes":17,"edges":28,"embedded":17},"elapsed_ms":1}
```

### 4. Query

```bash
# Raw hybrid search by default — 10 hits, 0ms, 3 signals (bm25 + vector + ppr)
kg search "postgres" --json
```

```json
{"ok":true,"command":"search","data":{"hits":[
  {"id":"01MOC01DATASTOREMOC000000","score":0.031,"signals":["bm25","vector","ppr"]},
  {"id":"01FACT01POSTGRESPRIMARY00","score":0.031,"signals":["bm25","vector","ppr"]},
  {"id":"01DEC02MIGRATIONTOCOCK000","score":0.031,"signals":["bm25","vector","ppr"]},
  ...10 total
]},"elapsed_ms":0}
```

```bash
# Entity-centric neighborhood fetch — 12 neighbors, 0ms
kg recall --entity "Postgres" --json
```

```json
{"ok":true,"command":"recall","data":{"entity":"Postgres","neighbors":["ADR-001: Postgres as primary datastore","ADR-002: Migrate to CockroachDB","CockroachDB","Billing Service","Postgres is the primary datastore","CockroachDB is the primary datastore","Billing Service dependencies","Database backup policy","Datastore MOC","What is the sync strategy during migration?","Architecture Review 2026-01-15","Datastore Migration Note 2026-03-01"]},"elapsed_ms":0}
```

For citation-backed Q&A, run the `kgx:ask` skill — the agent retrieves context
(`nl_query_memory` / `deep_search_memory`) and synthesizes the answer itself.

### 4b. Query by POLE entity type

```bash
# List all person entities
kg query --entity-type person

# List events tagged "q3"
kg query --entity-type event --tag q3

# Entity neighborhood with typed relations
kg recall --entity "Alice Chen" --relations
```

### 4c. Extract a POLE graph (kgx:pole skill)

After capturing a document or URL, run the `kgx:pole` skill to extract a
structured POLE (Person/Object/Location/Event) graph:

1. The agent reads the captured source.
2. Identifies persons, objects, locations, and events.
3. Creates entity notes with `entity_type` and typed relationship links.
4. Runs `kg index --full` to make the POLE graph queryable.

See the `kgx:pole` skill in your agent's skill list.

### 5. Dream (consolidate) — agent-harness driven

Consolidation is done by your agent via the `kgx:dream` skill:

1. Surface candidates: `dream_step({only:"orphan_repair,staleness,open_questions"})`
   (LLM-free) plus your own scan with `query_memory` / `deep_search_memory`.
2. Run the judgment passes (dedup, contradiction, supersession, stale archival)
   yourself and write proposed diffs to `.brain/.kg/staged_diffs.json` as
   `ProposedDiff` records (see the skill for the schema).
3. `kg review --approve all --ponytail-audit` applies the staged diffs.

### 6. Review and approve

```bash
# Approve all soft diffs (hard blocks require manual resolution)
kg review --approve all

# Interactive review (requires a TTY)
kg review --interactive

# Ponytail over-engineering audit
kg review --ponytail-audit
```

### 8. Check vault health

```bash
kg status --json
```

```json
{"ok":true,"command":"status","data":{"nodes":17,"edges":28,"orphans":1,"pending_diffs":0,"last_index":"2026-06-27T15:32:00Z","last_dream":null},"elapsed_ms":2}
```

```bash
kg link --json
```

```json
{"ok":true,"command":"link","data":{"backlinks":10,"orphans":1,"phantoms":0},"elapsed_ms":0}
```

### 9. Visualize

```bash
# Self-contained D3 HTML
kg graph --format html --json

# Self-contained Cytoscape HTML
kg graph --format cytoscape --json

# GraphML for graph tooling
kg graph --format graphml --json

# Mermaid diagram
kg graph --format mermaid --json
```

```json
{"ok":true,"command":"graph","data":{"edges":28,"nodes":17,"out":"graph.html"},"elapsed_ms":1}
```

### 10. Token accounting

```bash
kg tokens --json
```

```json
{"ok":true,"command":"tokens","data":{"aggregates":[
  {"count":1,"input_tokens":371,"key":"ask","output_tokens":27},
  {"count":1,"input_tokens":370,"key":"embed","output_tokens":0}
],"by":"operation","since_days":30},"elapsed_ms":0}
```

### 11. Schedule maintenance

```bash
kg cron list    # positional subcommand, not --list
kg cron remove dream-nightly
```

```json
{"ok":true,"command":"cron","data":{"jobs":["sh.kgx.dream-nightly.plist"]},"elapsed_ms":0}
```

### 12. Share (OKF bundles)

```bash
# Export a portable OKF bundle
kg ship --out team-brain-2026-06-27.tar.gz

# Import into a namespaced subtree
kg pull team-brain-2026-06-27.tar.gz --namespace /shared/team

# Validate (passes on fresh vault or after valid import)
kg validate --okf --json
```

```json
{"ok":true,"command":"validate","data":{"ok":true,"errors":[]},"elapsed_ms":0}
```

---

## All Commands

| Command | Purpose | Key Flags |
|---|---|---|
| `kg init` | Scaffold OKF vault into `.brain/` | `--template research\|code\|pkm\|team`, `--okf`, `--with-skills`, `--migrate` |
| `kg capture` | Ingest raw → `.brain/raw/` + source note (file/folder/stdin/URL). Auto-detects document formats (PDF/Excel/Word/PPTX via pandoc). | `--from file\|folder\|-|url`, `--type doc\|transcript\|web\|code`, `--ext md,txt,pdf,docx,...`, `--depth N` (URL crawl), `--max-pages N` |
| `kg link` | Compute wikilinks/backlinks, orphans | `--suggest`, `--orphans`, `--fix` |
| `kg index` | Build/refresh `.brain/.kg/brain.sqlite` | `--full`, `--incremental`, `--rebuild-vectors`, `--pagerank` |
| `kg recall` | Entity-centric neighborhood fetch | `--entity "Name"` |
| `kg search` | Raw hybrid search (no synthesis) | `--mode keyword\|semantic\|hybrid`, `--limit`, `--rerank-graph` |
| `kg review` | Show/approve/reject staged diffs | `--approve <ids\|all>`, `--reject`, `--interactive`, `--ponytail-audit` |
| `kg graph` | Export visualization | `--format html\|cytoscape\|graphml\|mermaid\|dot\|obsidian`, `--out` |
| `kg validate` | Integrity + OKF checks | `--okf`, `--links`, `--frontmatter` |
| `kg status` | Vault health snapshot | `--json`, `--verbose` |
| `kg tokens` | Token usage analytics | `--since 7d\|30d`, `--by operation\|command\|day` |

Removed (LLM work is done by the agent harness via the `kgx:` skills, not an
external provider): `kg ask`, `kg dream`, `kg refine`, `kg extract`,
`kg index --communities`, `kg search --rerank-llm`.
| `kg cron` | Manage systemd/launchd jobs | `add <name>`, `list`, `enable\|disable <name>`, `remove <name>`, `run <name>` |
| `kg ship` | Export OKF bundle | `--out path.tar.gz` |
| `kg pull` | Import OKF bundle | `--namespace /subtree` |
| `kg mcp-server` | Launch MCP stdio server | `--transport stdio` |

**Important:** All commands use CWD as the vault root. There is no `--vault` flag — `cd` into your vault before running.

Every command supports `--json` (emits `{"ok":bool,"command":"...","data":{...},"elapsed_ms":N}`).

---

## E2E Integration Test Results

All 18 PRD smoke tests pass + 83 workspace tests. Tests run with `KGX_LLM=mock` for hermetic, network-free CI.

```bash
KGX_LLM=mock cargo test --package smoke --test '*' -- --test-threads=1
```

| Test | Description | Time | Result |
|---|---|---|---|
| T01 `t01_raw_hash_unchanged_after_extract` | `raw/` file hash identical before/after extract+dream | 0.02s | ✅ PASS |
| T02 `t02_extract_produces_provenance_facts` | Extract yields ≥1 fact per raw source, each with `source:` + `recorded_at:` | 0.01s | ✅ PASS |
| T03 `t03_link_phantoms_zero_for_fixture` | Every `[[X]]` resolves to a real note; zero phantom backlinks | 0.01s | ✅ PASS |
| T04 `t04_exactly_one_orphan` | Exactly 1 injected orphan detected (MOCs excluded) | 0.01s | ✅ PASS |
| T05/T07/T08/T15 `t05_..._dream_stages_then_review_applies_soft_and_blocks_hard` | Dream stages, review auto-applies Soft, blocks Hard | 0.11s | ✅ PASS |
| T06 `t06_dedup_merge_archives_duplicate_and_keeps_files` | Canonical survives; duplicate archived; inbound edges repointed | 0.11s | ✅ PASS |
| T10 `t10_rebuild_is_deterministic` | `rm -rf .kg && kg index --full` → identical node/edge counts | 0.02s | ✅ PASS |
| T11a `t11_init_then_validate_passes` | Fresh init + `kg validate --okf` passes immediately | 0.03s | ✅ PASS |
| T11b `t11_ship_pull_validate_roundtrip` | Export bundle → import → `kg validate --okf` lossless | 0.03s | ✅ PASS |
| T12 `t12_graph_html_counts_match_brain` | HTML graph node/edge counts match `brain.sqlite` | 0.02s | ✅ PASS |
| T13 `t13_every_community_has_summary_and_moc` | ≥3 Leiden communities; each has a summary note + MOC | 0.02s | ✅ PASS |
| T14 (in T05 bundle) | Dead source + old fact → proposed `archived`; file retained | 0.11s | ✅ PASS |
| T16 `t16_index_writes_token_record` | `kg tokens` matches per-command token records | 0.01s | ✅ PASS |
| T17 `t17_rtk_wrapper_uses_rtk_or_raw_fallback` | RTK wrapper compresses output or falls back gracefully | 0.01s | ✅ PASS |
| T18 `t18_ponytail_audit_reports_over_broad_review_flags` | `kg review --ponytail-audit` flags over-engineered diffs | 0.01s | ✅ PASS |
| — `install_script_is_valid_bash` | `install.sh` is valid bash | 0.00s | ✅ PASS |
| — `native_skill_packages_reference_same_mcp_tools` | Claude/Codex/Cursor skill files reference identical MCP tool names | 0.00s | ✅ PASS |

**101 / 101 tests pass. 0 failures.**

Additional test suites:

| Suite | Status |
|---|---|
| `cli_init` (init templates, OKF flag, skills flag) | ✅ |
| `cli_extract` (extract_creates_facts_with_provenance) | ✅ |
| `cli_ask` (ask with citations, JSON envelope) | ✅ |
| `cli_search` (search modes: keyword/semantic/hybrid) | ✅ |
| `cli_link` (phantom detection, orphan listing, fix) | ✅ |
| `kgx-retrieval` (RRF fusion, hybrid scoring) | ✅ |
| `kgx-okf` (validate_integration, bundle round-trip) | ✅ |

---

## KGX vs Without KGX

These numbers are measured on the `vault-min` fixture (17 notes, 28 edges) using `KGX_LLM=mock`. Token counts are from `kg tokens` output after running commands.

### Token cost for Q&A

| Approach | Input tokens | Notes |
|---|---|---|
| **Without KGX**: pass all 17 notes to LLM | ~3,400 | Estimated: 17 notes × ~200 tokens avg |
| **With KGX** `kg ask`: hybrid retrieval | **371** | Measured from `kg tokens` output |
| **Reduction** | **~89%** | On this 17-note fixture |

At scale (200 notes), the advantage compounds: KGX retrieval still sends 8–10 relevant chunks; naive paste-all grows linearly.

### What you get from `kg recall --entity Postgres`

With one command, 2-hop graph traversal returns 12 connected nodes in 0ms — no LLM call needed:

```
ADR-001: Postgres as primary datastore
ADR-002: Migrate to CockroachDB
CockroachDB
Billing Service
Postgres is the primary datastore
CockroachDB is the primary datastore
Billing Service dependencies
Database backup policy
Datastore MOC
What is the sync strategy during migration?
Architecture Review 2026-01-15
Datastore Migration Note 2026-03-01
```

Without KGX, finding these 12 connected notes requires reading all files and manually tracing references.

### What `kg dream --dry-run` finds on a 17-note vault

```json
{"done_signal":true,"dry_run":true,"hard_blocks":6,"iterations":2,"staged":14}
```

14 staged diffs proposed, 6 hard blocks (contradictions requiring human review) — found in 2ms without an LLM call. The dream engine detects these automatically; without KGX they accumulate undetected.

### Comparison table

| Dimension | Without KGX | With KGX |
|---|---|---|
| **Input tokens per Q&A** | ~3,400 (17 notes) | 371 measured |
| **Multi-hop recall** | Manual graph traversal | 2-hop PPR, 0ms |
| **Answer citations** | Manual or none | Every answer cites note IDs |
| **Contradiction detection** | None | Automatic (6 found in 2ms on fixture) |
| **Knowledge decay** | Notes drift / go stale | `kg dream` proposes archival |
| **Portability** | App-locked | OKF bundle: `kg ship` + `kg pull` |
| **Graph visualisation** | None | `kg graph --format html` (17 nodes, 28 edges) |
| **Token accounting** | None | `kg tokens --by operation` |

---

## Sprint Simulation

A full 2-week sprint simulation (DataLake 2.0, 4 engineers, 11 tickets) was run with real `kg` commands. See [`docs/sprint-simulation.md`](docs/sprint-simulation.md) for the complete day-by-day breakdown.

### Keyword search benchmark: beats ripgrep by +22% P@5

A 15-query golden benchmark (Sprint 9 vault, 32 nodes / 57 edges) compares `kg search --mode keyword` against a ripgrep baseline. Keyword mode combines BM25/FTS5, tags-in-LIKE substring matching, and tag-frequency-weighted expansion (k=300 RRF).

| Metric | ripgrep | kg keyword | vs rg |
|--------|---------|------------|-------|
| P@5    | 0.360   | **0.440**  | **+22%** |
| R@5    | 0.911   | 0.911      | same   |
| F1     | 0.528   | **0.585**  | **+11%** |
| NDCG@5 | 0.835  | **0.889**  | **+6.5%** |

Full results: [`docs/kgx-guide.html#eval`](docs/kgx-guide.html)

### What was simulated

**Stack:** Kafka → Delta Lake v3 → Spark 3.4.2/k8s → Trino 435 → DataHub 0.12 → dbt 1.7  
**Ceremonies:** Sprint planning, 10 daily dev cycles, mid-sprint grooming, review, retro  
**Artifacts produced:** 49 notes, 56 edges (facts, entities, decisions, experiences, questions, MOCs)

### Measured results (WITH KGX — real `kg` commands)

```
kg tokens output after 10 days:
  ask   : 10 calls,  8,155 input tokens,  270 output tokens
  embed : 12 calls, 21,927 input tokens,    0 output tokens
  extract:  1 call,    240 input tokens,  619 output tokens
  ─────────────────────────────────────────────────────────
  Total : 23 calls, 30,322 input tokens,  889 output tokens
```

```
kg status (end of sprint):
  nodes: 49   edges: 56   orphans: 1   pending_diffs: 0

kg dream --dry-run (end of sprint):
  staged: 32   hard_blocks: 10   iterations: 2   elapsed_ms: 3
```

### Head-to-head

| Metric | Without KGX | With KGX | Savings |
|--------|-------------|----------|---------|
| Tokens for 10 daily queries | ~14,900 | 8,155 | **45%** |
| Session re-hydration tokens (10 days) | ~15,000 | 0 | **100%** |
| **Total query token cost** | **~29,900** | **8,155** | **73%** |
| Knowledge-management overhead | 150 min | 9 min | **94%** |
| Multi-hop question time (avg) | 12–18 min | 30–60 sec | **96%** |
| Contradictions detected automatically | 0 | 32 staged, 10 flagged | — |
| Tech debt items missed at retro | 2–3 of 9 | 0 | — |
| Graph edges tracked | 0 | 56 | — |

### 9 facts forgotten without a persistent brain

Without KGX, these items would be lost between sessions:

1. `SPARK-45123` workaround — grooming doc only, dropped over weekend
2. `dbt-delta` adapter bug #847 — mentioned once in Day 2, no entity link
3. DL-103 blocked on DL-102 dependency — in planning doc only
4. Mobile SDK v2.1 auth-race root cause for NULL `user_id`
5. DataHub lineage UI slow >50 nodes (issue #9821)
6. Delta OPTIMIZE threshold >10,000 files — not in operations runbook
7. Kafka consumer throughput gap (180k vs 200k target) — never revisited
8. dbt 14-min backfill baseline — buried in Day 3, no benchmark link
9. S3 checkpoint interval (15 min) for Spark structured streaming

`kg dream --dry-run` surfaces all 9 as staged diffs for human review.

---

## Configuration Reference

### LLM Providers

```bash
# Claude (best for extraction and dreaming)
export ANTHROPIC_API_KEY=sk-ant-...
export KGX_LLM=claude

# OpenAI
export OPENAI_API_KEY=sk-...
export KGX_LLM=openai

# Ollama (local, offline)
export KGX_LLM=ollama
export KGX_OLLAMA_MODEL=llama3.1

# Mock (for testing — no API calls, hermetic)
export KGX_LLM=mock
```

### Embeddings & Retrieval Pipeline

`kg search` runs a four-stage local pipeline: BM25 + LIKE + tags + dense
vector + SPLADE sparse candidates → reciprocal-rank fusion → personalized
PageRank over the note graph (seeded by BM25 hits and query-matched
entities) → local cross-encoder rerank. All models are local ONNX
(fastembed), downloaded once and cached; no LLM tokens are spent per query.

| Env var | Values | Default |
|---|---|---|
| `KGX_EMBED` | `fastembed` / `minilm` / `mock` / `off` | `fastembed` |
| `KGX_SPARSE` | on / `off` / `mock` | on |
| `KGX_RERANK` | on / `off` / `mock` | off |
| `KGX_RERANK_MODEL` | `jina-turbo` / `bge-base` | `jina-turbo` |
| `KGX_RERANK_TOPK` | integer | `30` |

`kg status` prints the active stages (e.g. `retrieval: bm25+like+tags+dense+sparse | ppr | rerank(jina-turbo)` when reranker is enabled).

### Extraction Semantics

With `KGX_LLM=claude`, `KGX_LLM=openai`, or `KGX_LLM=ollama`, `kg extract` classifies entities as `person`, `object`, `location`, or `event` and emits typed relations when the provider returns them. With `KGX_LLM=mock`, extraction remains deterministic and untyped for hermetic tests.

### Frontmatter: `type` field

Valid values: `fact` | `entity` | `decision` | `experience` | `moc` | `source` | `question`

Raw files in `raw/` use `type: source`. All other subdirectories use their corresponding type.

### JSON Output

Every command emits a JSON envelope when `--json` is passed:

```json
{
  "ok": true,
  "command": "ask",
  "data": { "answer": "...", "citations": ["01FACT..."] },
  "elapsed_ms": 1
}
```

Use with `jq`: `kg ask "..." --json | jq .data.answer`

---

## Vault Layout Reference

```
vault/                    ← run kg commands here (project root)
.brain/                    ← knowledge vault (resolved by every kg command)
├── CLAUDE.md              ← agent behavior contract
├── index.md               ← OKF root map-of-maps
├── log.md                 ← OKF append-only operation log
├── raw/                   ← immutable captured sources
│   └── 2026-06-27-standup.md   (type: source)
├── notes/
│   ├── facts/             ← atomic claims (type: fact)
│   ├── entities/          ← people, systems, concepts (type: entity)
│   ├── decisions/         ← ADRs (type: decision)
│   ├── experiences/       ← lessons learned (type: experience)
│   ├── moc/               ← maps of content (type: moc)
│   ├── sources/           ← metadata pointers to raw/ (type: source)
│   ├── questions/         ← open gaps (type: question)
│   └── archived/          ← deprecated notes (never deleted)
└── .kg/                   ← derived, git-ignored
    ├── brain.sqlite        ← nodes, edges, FTS5, vectors, PPR, communities
    ├── meta.json           ← last-run timestamps
    └── metrics.log         ← per-command token JSONL
```

---

## Contributing

Built in Rust 2021. Requires Rust 1.78+.

```bash
cargo build --workspace
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
KGX_LLM=mock cargo test --package smoke --test '*' -- --test-threads=1
cargo test --workspace --test '*'
```

All PRs require:
- `cargo fmt` clean
- `cargo clippy -D warnings` clean
- All 101 tests green (`KGX_LLM=mock cargo test --workspace`)
- No `unwrap()` / `expect()` / `panic!` in library crates

---

## License

MIT
