# KGX — Local-First, AI-Managed Knowledge Graph CLI
## Consolidated PRD & Implementation Handoff Package

***

## 1. Executive Summary

**KGX** (working name) is a **Rust CLI** that turns a plain Markdown + `[[wikilinks]]` vault into a living, queryable knowledge graph. The vault is the **canonical, git-versioned source of truth** (OKF-compatible); a disposable SQLite “brain” provides hybrid vector + graph + keyword retrieval, PageRank, and community summaries. AI agents ingest, extract, link, answer, and consolidate; humans curate via git diffs and Obsidian’s native graph view.

**Core philosophy:** *“Humans abandon wikis because maintenance grows faster than value. LLMs don’t get bored.”* — Andrej Karpathy (LLM Wiki gist, 2026)

***

## 2. Problem & Goals

### Problem
- Raw knowledge (transcripts, docs, code, web) accumulates faster than humans can structure it.
- Classic RAG re-derives answers from scratch; wikis rot.
- No local-first tool fuses **OKF portability**, **Karpathy LLM-Wiki patterns**, **GraphRAG/HippoRAG retrieval**, and **Mem0 bi-temporal memory** into one CLI that both humans and agents use daily.

### Goals
| Goal | Success Metric |
|---|---|
| **Markdown-canonical** | `rm -rf .kg && kg index --full` fully reconstructs brain |
| **OKF-compatible** | `kg validate --okf` passes; round-trip export/import lossless |
| **Hybrid recall beats vector-only** | >10% recall gain on multi-hop QA (HippoRAG benchmark) |
| **Dream pass unattended** | ≥90% of proposed diffs accepted on review |
| **Token efficiency** | RTK + Ponytail integration yields ~80% session token savings |
| **Cross-device sync** | `git push/pull` works Mac ↔ iPhone via Working Copy |

### Non-Goals
- Hosted SaaS, multi-writer concurrent DB, >100K nodes without Neo4j export, general document store.

***

## 3. Target Users & Use Cases

| User | Primary Flows |
|---|---|
| **Researchers** | Literature notes → entity/fact extraction → cross-paper synthesis → open-question tracking |
| **Engineering Teams** | ADR/decision logs → codebase knowledge (DeepWiki-style) → onboarding MOCs |
| **Meeting Pipelines** | Transcript → entities/decisions/action items → Q&A over decisions |
| **Personal PKM** | Zettelkasten → evergreen notes → graph exploration in Obsidian |
| **Agent Memory** | Portable, inspectable substrate other agents read/write via MCP |
| **Team Sharing** | `kg ship` OKF bundles → `kg pull` into namespaced subtree |

***

## 4. Core Principles (Immutable)

1. **Markdown-canonical** — derived DB always rebuildable.
2. **Local-first** — offline-capable; cloud LLM optional.
3. **Human + AI collaboration** — AI proposes, git diff reviews, human approves.
4. **Supersession-not-deletion** — bi-temporal stamps; `status: superseded`, never `rm`.
5. **Atomicity** — one fact/idea per note (Zettelkasten).
6. **Git-friendly** — small, diffable files, stable ULIDs, deterministic formatting.
7. **Provenance always** — every fact links to immutable `raw/` source.
8. **Token-conscious** — RTK (shell output) + Ponytail (agent behavior) built-in.

***

## 5. Vault Layout (OKF-Compatible Bundle)

```
vault/
├── index.md              # OKF root index (human map-of-maps)
├── log.md                # OKF append-only log (## [YYYY-MM-DD] op \| title)
├── CLAUDE.md             # Schema + prompts + workflows (Karpathy Layer 3)
├── raw/                  # Immutable sources + assets/ (provenance, never edited)
│   ├── 2026-06-27-standup.md
│   └── assets/
├── notes/
│   ├── facts/            # Atomic claims
│   ├── entities/         # People, systems, concepts, orgs
│   ├── decisions/        # ADRs (MADR/Nygard)
│   ├── experiences/      # Lessons learned
│   ├── moc/              # Maps of content (entrypoints)
│   ├── sources/          # Raw-capture metadata pointers
│   ├── questions/        # Open questions / gaps
│   └── archived/         # Deprecated/superseded (never deleted)
└── .kg/                  # DERIVED, git-ignored
    ├── brain.sqlite      # Nodes, edges, embeddings, PPR, communities, FTS5
    ├── meta.json         # Runtime metrics, last-run timestamps
    └── metrics.log       # Token usage per command/operation (JSONL)
```

**OKF Conformance:** Every `notes/**/*.md` and `raw/**/*.md` has YAML frontmatter with required `type` field. `index.md`/`log.md` follow OKF reserved-file semantics. Unknown keys tolerated per OKF §9.

***

## 6. Note Types & Frontmatter Schema

```yaml
---
type: fact                       # fact | entity | decision | experience | moc | source | question
id: 01J9X2ABC                    # ULID, stable across renames
title: "Postgres is the primary datastore"

# Lifecycle / validity (bi-temporal, Zep/Graphiti model)
status: active                   # active | deprecated | archived | superseded
valid_from: 2026-01-15           # when true-in-the-world
valid_to: null                   # null = still valid
recorded_at: 2026-06-27T10:00Z   # when system learned it

# Supersession graph
supersedes: []
superseded_by: null

# Provenance
source: "[[raw/2026-01-15-arch-review]]"
confidence: high                 # high | medium | low
sources_count: 3

# Graph-ish
tags: [infra, datastore]
links: ["[[Postgres]]", "[[Billing Service]]"]

# Entity-specific
entity_type: null                # person | system | concept | org | ...
aliases: []

# Housekeeping
created_by: human                # human | agent
created_via: cli                 # cli | mcp | sync
```

***

## 7. Derived Graph Layer — SQLite “Brain”

**Why SQLite:** Single file, zero-config, embeddable (`rusqlite` + `sqlite-vec` / Turso), git-ignored, fully rebuildable.

**Schema:**
```sql
-- Notes as nodes
CREATE TABLE notes (
  id TEXT PRIMARY KEY,
  path TEXT NOT NULL,
  type TEXT NOT NULL,
  status TEXT NOT NULL,
  valid_from TEXT,
  valid_to TEXT,
  recorded_at TEXT,
  tags TEXT,                    -- JSON array
  raw_text TEXT,
  embedding BLOB                -- 384-dim (all-MiniLM-L6-v2 default)
);

-- Edges from wikilinks + typed links
CREATE TABLE edges (
  src_id TEXT NOT NULL,
  dst_id TEXT NOT NULL,
  rel_type TEXT NOT NULL,       -- links_to | supersedes | derived_from | cites | mentions_entity | contradicts
  valid_from TEXT,
  valid_to TEXT,
  PRIMARY KEY (src_id, dst_id, rel_type)
);

-- FTS5 for BM25
CREATE VIRTUAL TABLE notes_fts USING fts5(id, raw_text, tags, content='', tokenize='porter');

-- PageRank / Personalized PageRank
CREATE TABLE pagerank (id TEXT PRIMARY KEY, score REAL);

-- Leiden communities + summaries
CREATE TABLE communities (id TEXT, community_id INTEGER, PRIMARY KEY (id, community_id));
```

**Rebuild contract:** `kg index --full` is deterministic; incremental mode updates only changed files + 1–2 hop neighbors.

***

## 8. Retrieval Architecture (Hybrid, Mem0 + HippoRAG + LazyGraphRAG)

```
Query
  ├─► Embedding → Vector KNN (sqlite-vec)
  ├─► BM25 (FTS5)
  ├─► Entity NER → Direct graph hits + 1-hop neighborhood
  └─► Global? → Leiden community summaries (GraphRAG global)

Fusion: Reciprocal Rank Fusion (k=60) over all signals
Graph Expansion: Personalized PageRank from seed nodes (HippoRAG)
    → up to 20% multi-hop QA gains over vector-only

Modes:
  • Local (default): candidate nodes + neighborhood
  • Global (--scope global): + community summaries, LazyGraphRAG-style (0.1% indexing cost of full GraphRAG)
```

***

## 9. Commands (Minimal, Composable, All `--json`)

| Verb | Purpose | Key Flags |
|---|---|---|
| `kg init` | Scaffold OKF vault | `--template research\|code\|pkm\|team`, `--okf` |
| `kg capture` | Ingest raw → `raw/` + `source` note | `--from file\|url\|-`, `--type doc\|transcript\|web\|code` |
| `kg extract` | LLM: raw → atomic facts/entities/decisions | `--source <id>`, `--batch`, `--dry-run`, `--intensity lite\|full\|ultra` |
| `kg link` | (Re)compute wikilinks/backlinks, suggest, orphans | `--suggest`, `--orphans`, `--fix` |
| `kg index` | Build/refresh `.kg/brain.sqlite` | `--full`, `--incremental`, `--communities`, `--pagerank` |
| `kg ask` | Hybrid Q&A over graph | `--scope local\|global`, `--write`, `--cite`, `--mode keyword\|semantic\|hybrid` |
| `kg recall` | Entity-centric neighborhood fetch | `--entity "Postgres"` |
| `kg search` | Raw hybrid search (no synthesis) | `--type fact,entity`, `--mode`, `--limit` |
| `kg dream` | Consolidation: dedup, contradict, supersede, stale, resummarize, link repair | `--max-iterations N`, `--only <set>`, `--cron`, `--intensity` |
| `kg review` | Show staged diffs, approve/reject | `--approve <ids\|all>`, `--reject`, `--interactive`, `--ponytail-audit` |
| `kg graph` | Export visualization | `--format html\|mermaid\|dot\|obsidian`, `--out`, `--filter` |
| `kg validate` | Integrity + OKF checks | `--okf`, `--links`, `--frontmatter`, `--bitemporal` |
| `kg status` | Vault health snapshot | `--json`, `--verbose` |
| `kg dashboard` | TUI dashboard (counts, trends, errors, tokens) | `--json` |
| `kg tokens` | Token usage analytics | `--since 7d\|30d`, `--by operation\|command\|day` |
| `kg cron` | Manage systemd/launchd jobs | `list`, `enable\|disable <name>`, `run <name>`, `add <name> ...` |
| `kg docs` | Generate HTML use-case flows | `usecase research\|onboarding\|meetings\|... --out file.html` |
| `kg ship` / `kg pull` | OKF bundle export/import | `--namespace`, `--out` |

**MCP Server Mode:** `kg mcp-server --transport stdio` exposes tools: `search_notes`, `get_note`, `upsert_note`, `ask_question`, `capture_raw`, `dream_step`.

***

## 10. Dreaming / Consolidation Engine

Runs as `kg dream` (scheduled via `kg cron enable dream-nightly`). **All changes staged on `kg/dream` branch; never auto-commit.**

| Pass | Action | Output |
|---|---|---|
| **Dedup/Merge** | Embedding blocking → LLM match/merge | Canonical node + redirected edges (Mem0 ADD-only bias) |
| **Contradiction** | Semantic + graph search for conflicts | Classify: agree / soft / scope / hard |
| **Supersession** | Close old `valid_to`, `superseded_by` | Bi-temporal supersession, no deletion |
| **Staleness/Archive** | Age + broken source + low access → `deprecated`/`archived` | Files retained, status flipped |
| **Community Resummary** | Leiden (guaranteed connected) + LLM summaries | Updated MOC/summary notes |
| **Orphan/Link Repair** | LLM proposes cross-links/MOCs | Diff proposals adding `[[wikilinks]]` |
| **Open Questions** | Gaps → `type: question` notes | Auto-closed when facts answer them |

**Scheduling:** Ralph-loop style (`--max-iterations`) or cron. Human review gate via `kg review`.

***

## 11. Token Efficiency: RTK + Ponytail Integration

| Layer | Tool | Savings | Where Applied |
|---|---|---|---|
| **Shell-output compression** | **RTK** (Rust Token Killer) | 60–90% per command | Every shell-out in `capture`, `index`, `extract`, test runners |
| **Agent-behavior minimalism** | **Ponytail** (lazy senior dev ladder) | ~22% tokens, ~54% LOC | All LLM prompts: extraction, dreaming, Q&A, review |

**Implementation:**
- Installer: `kg init --with-rtk` pulls RTK binary, adds hooks for Claude Code / Cursor / Codex.
- Rust helper: `run_with_rtk(cmd)` wraps all `Command::new` shell-outs.
- `CLAUDE.md` embeds Ponytail ladders for each operation (extraction, dreaming, graph edits).
- Flags: `kg extract --intensity ultra`, `kg review --ponytail-audit`.

***

## 12. Cross-Device Sync (Mac ↔ iPhone)

- **Remote:** Any git host (GitHub, Gitea, Codeberg).
- **Mac:** Clone → `kg` + Obsidian + optional `obsidian-git` plugin.
- **iPhone:** Working Copy (or newer libgit2 client) → clone same repo → Obsidian Mobile points to Working Copy folder.
- **Hygiene:** `.gitignore` `.obsidian*/workspace`, `.kg/`.
- **Helper:** `kg sync status` / `push` / `pull` (thin `git` wrappers).

***

## 13. Automated Maintenance (Scheduler)

| Platform | Mechanism | Jobs Managed |
|---|---|---|
| **Linux** | systemd user timers | `dream-nightly` (03:00), `index-nightly` (optional) |
| **macOS** | launchd `LaunchAgents` | Same, via plist with `StartCalendarInterval` |

**CLI:** `kg cron list|enable|disable|run|add` — writes units/plists, runs `systemctl --user` / `launchctl`.

***

## 14. Documentation & Visualization

### Static HTML Use-Case Flows (`kg docs usecase X`)
- Per use case: narrative, exact command sequences, Mermaid/SVG diagrams.
- Generated via `tera`/`askama` templates + LLM-assisted content.

### Interactive HTML Graph (`kg graph --format html`)
- **Self-contained** single-file HTML (D3 force-directed + timeline).
- Inspired by **brain-map-skill**: filter by type/status, click→side-panel, time slider for growth replay.
- Zero server; open in browser or embed in docs.

### Hosting
- `docs/` → GitHub Pages, or `static-files` MCP skill for subdomain hosting.

***

## 15. Metrics & Observability

**`.kg/meta.json` + `metrics.log` (JSONL):**
- Per-command: `model`, `operation`, `input_tokens`, `output_tokens`, `elapsed_ms`, `correlation_id`.
- Aggregated: daily totals, per-operation averages, trends.

**`kg status --verbose` / `kg dashboard`** surface:
- Node/edge counts, orphans, stale candidates, pending diffs.
- Token usage sparklines, last dream/index timestamps.
- Scheduler health (systemd/launchd status).

***

## 16. Testing Spec (TDD Targets)

| Test | Acceptance |
|---|---|
| **T01 capture-immutability** | `raw/` file hash unchanged after extract+dream |
| **T02 extract-correctness** | ≥4/5 known facts → atomic notes with provenance + bi-temporal |
| **T03 link-integrity** | Every `[[X]]` yields backlink; no phantoms |
| **T04 orphan-detection** | Exactly 1 injected orphan listed (MOCs excluded) |
| **T05 bitemporal-supersession** | Old fact → `superseded`, `valid_to` set, file retained |
| **T06 dedup-merge** | Canonical remains; inbound edges repoint; history kept |
| **T07 contradiction-detect** | Hard + soft flagged; severity correct; `Unresolved` blocks auto-commit |
| **T08 dream-review-gate** | All changes on branch; `main` untouched until `kg review --approve` |
| **T09 qa-recall** | Hybrid RRF > vector-only baseline on multi-hop set |
| **T10 graph-rebuild-sync** | `rm -rf .kg && kg index --full` → identical counts, deterministic |
| **T11 okf-roundtrip** | Export→import → `kg validate --okf` passes, lossless |
| **T12 viz-export** | HTML opens; node/edge counts match brain |
| **T13 community-summary** | ≥3 connected Leiden communities; each has summary note |
| **T14 stale-archival** | Dead source + old fact → proposed `archived`, file kept |
| **T15 ralph-loop-bound** | `--max-iterations 3` stops ≤3 iters or on `<promise>DONE</promise>` |
| **T16 token-accounting** | `kg tokens` matches provider headers per command |
| **T17 rtk-integration** | `rtk git diff` output ≤30% tokens of raw |
| **T18 ponytail-audit** | `kg review --ponytail-audit` flags over-engineered diffs |

***

## 17. Implementation Roadmap (Phased)

| Phase | Deliverable | Duration |
|---|---|---|
| **0 — Skeleton** | `kg init`, `validate --okf`, vault parser, frontmatter + wikilink AST | 1 wk |
| **1 — Brain** | `brain.sqlite` schema, `index --full/--inc`, BM25, embeddings, KNN | 2 wk |
| **2 — Ask v1** | Hybrid retrieval (vec+BM25+entity), RRF, PPR (HippoRAG), `ask` | 2 wk |
| **3 — Dream** | All 7 passes as pure `vault+brain → diffs`; `review` UI | 2 wk |
| **4 — GraphRAG Tuning** | Leiden, community summaries, LazyGraphRAG toggle | 1 wk |
| **5 — MCP + Skills** | `mcp-server`, RTK hooks, Ponytail prompts, `cron` manager | 1 wk |
| **6 — Polish** | `dashboard`, `docs`, `graph html`, `ship/pull`, installer script | 1 wk |

**Total:** ~10 weeks for MVP; each phase dogfoodable.

***

## 18. Rust Module Layout (Suggested)

```
kgx/
├── cli/              # clap command definitions, JSON output helpers
├── okf/              # OKF parsing, validation, bundle I/O
├── vault/            # FS abstraction: read/write notes, ULID, git helpers
├── graph/            # SQLite brain: nodes, edges, FTS, vec, PR, Leiden
├── retrieval/        # Hybrid search, PPR, RRF, community summaries
├── llm/              # Provider trait (Claude, Ollama, OpenAI), prompt templates (CLAUDE.md)
├── extract/          # Extraction pipeline (raw → facts/entities/decisions)
├── dream/            # Consolidation passes (dedup, contradict, supersede, ...)
├── mcp/              # MCP server implementation (tools, resources)
├── tokens/           # Token accounting, metrics persistence
├── cron/             # systemd/launchd abstraction
├── viz/              # HTML/D3, Mermaid, DOT exporters
├── docs/             # Use-case HTML generators
├── rtk/              # RTK wrapper + hook installer
└── ponytail/         # Prompt ladders, audit rules
```

**Key dependencies:** `clap`, `rusqlite` + `sqlite-vec`, `pulldown-cmark`, `serde_yaml`, `petgraph`, `ort`/`candle` (embeddings), `tera` (templates), `tokio`, `mcp-sdk` (when stable), `ratatui` (dashboard TUI).

***

## 19. CLAUDE.md — The Contract (Co-Evolved)

This file lives in the vault root and is the **single source of truth for agent behavior**. It contains:

1. **Schema reference** (note types, frontmatter fields, allowed values).
2. **Extraction ladder** (Ponytail-style) for each operation.
3. **Dreaming rules** (when to supersede vs merge, confidence thresholds).
4. **Linking conventions** (wikilink syntax, typed link predicates).
5. **MOC/entrypoint guidelines** (how to maintain `index.md` and `tags: [entrypoint]`).
5. **Review checklist** (what `kg review --ponytail-audit` checks).

Agents read this at session start; humans edit it to steer behavior.

***

## 20. Installer One-Liner

```bash
curl -fsSL https://get.kgx.sh/install.sh | bash
```

**Does:**
1. Detects OS/arch → downloads `kg` binary to `~/.local/bin` or `/usr/local/bin`.
2. `kg init --template research --vault ~/vaults/brain` (optional).
3. If Claude Code detected: `claude mcp add --transport stdio kgx -- kg mcp-server --transport stdio`.
4. Optional flags: `--with-rtk`, `--with-html-graph`, `--with-docs`, `--with-cron`.
5. Prints next steps: `cd ~/vaults/brain && kg capture --from ...`.

***

## 21. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| OKF v0.1 churn | Pin `okf_version: "0.1"`; profile layer isolates our extensions |
| Entity over-merge | Blocking + LLM verification; ADD-only bias; git history reversible |
| RTK breaks command semantics | Opt-in per command; fallback to raw; telemetry opt-out |
| Ponytail over-simplifies | Three intensity levels; security/validation never cut; human review gate |
| SQLite scale ceiling | Export to Neo4j/Memgraph at ~100K nodes; hub-edge down-weighting |
| Rust LLM ecosystem thinner than Python | Prompts in `CLAUDE.md`; provider trait over HTTP/CLI; core stays deterministic |

***

## 22. Handoff Checklist for AI Builder

- [ ] Repo initialized with Cargo workspace + module layout above
- [ ] `kg init` scaffolds vault + `CLAUDE.md` template
- [ ] OKF parser + validator passes spec tests
- [ ] `brain.sqlite` schema + `index --full/--inc` deterministic
- [ ] Hybrid retrieval (vec + BM25 + entity + PPR) + RRF fusion
- [ ] `kg ask` + `kg recall` + `kg search` with `--json`
- [ ] All 7 dream passes as pure functions emitting diffs
- [ ] `kg review` with interactive + `--ponytail-audit`
- [ ] RTK wrapper + hook installer for Claude Code / Cursor
- [ ] Ponytail ladders embedded in `CLAUDE.md` + provider prompts
- [ ] Token accounting per command + `kg tokens` + `dashboard`
- [ ] `kg cron` managing systemd/launchd timers
- [ ] `kg graph --format html` (self-contained D3, brain-map-skill parity)
- [ ] `kg docs usecase X` generating 6 HTML flows
- [ ] `kg ship` / `kg pull` OKF bundle round-trip
- [ ] All 18 test specs passing in CI
- [ ] Installer script published to `get.kgx.sh`

***

## 23. Appendix: Key References

- **OKF Spec** — Google Cloud, 2026 (`knowledge-catalog/okf/SPEC.md`) [github](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md)
- **Karpathy LLM Wiki** — Gist + community extensions (v2: lifecycle, graphs, hooks) [gist.github](https://gist.github.com/rohitg00/2067ab416f7bbe447c1977edaaa681e2)
- **GraphRAG / LazyGraphRAG** — Microsoft Research (global queries, 0.1% indexing cost) [microsoft](https://www.microsoft.com/en-us/research/project/graphrag/)
- **HippoRAG** — Personalized PageRank over KG, +20% multi-hop QA [arxiv](https://arxiv.org/html/2405.14831v2)
- **Mem0** — ADD-only memory, 66.9% vs 52.9% on LOCOMO [agentry](https://agentry.press/news/mem0-algorithm-update-hits-91-6-on-locomo-94-8-on-longmemeval/)
- **RTK** — CLI proxy, 60–90% shell token reduction [github](https://github.com/rtk-ai/rtk)
- **Ponytail** — Lazy senior dev ladder, −54% LOC, −22% tokens [toolsdepth](https://toolsdepth.com/reviews/ponytail-review-2026)
- **brain-map-skill** — Single-file HTML interactive knowledge map [everydev](https://www.everydev.ai/tools/brain-map-skill)
- **systemd timers / launchd** — Modern cron replacements [wiki.archlinux](https://wiki.archlinux.org/title/Systemd/Timers)

***

**End of PRD.** This document is the single source of truth for implementation. All design decisions are traceable to the references above. Build phase-by-phase, dogfood at each step, and keep `CLAUDE.md` as the living contract between human and AI.