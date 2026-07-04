# KGX — Final Consolidated Documentation

> **The single source of truth for KGX as of 2026-07-04.**
> This document supersedes and consolidates: `prd.md`, `context-layer.md`, `STATUS.md`, `docs/eval-results.md`, `docs/sprint-simulation.md`, `docs/sprint-simulation-benchmark-results.md`, and `docs/superpowers/specs/2026-07-04-kgx-unified-prd.md`.
> Historical artifacts are preserved in their original paths (see [§12 Doc Index](#12-doc-index)) for traceability; this file is what you should read first.
>
> **Branch:** `kgx-unified-final` — the linear merge of `main` + `kgx-unified-rewrite` + `kgx-phase0-real-bugs`.
> **Test status:** 101 workspace tests + 2 opt-in semantic tests, all passing. 0 compiler warnings. 0 production markers (TODO/FIXME/`unimplemented!`/`panic!`).

---

## 1. What KGX is

KGX is a **local-first, harness-agnostic, self-evolving knowledge graph** for humans and AI agents.

- **One `kg` binary** (~38 MB optimized, statically linked, no server required) turns a plain Markdown vault into a queryable brain: hybrid vector + BM25 + graph retrieval, PageRank, Leiden communities, an LLM-driven dream/consolidation loop, and a 3D WebGL graph viewer.
- **Plugs into any harness** — Claude Code, Codex, Cursor, OpenCode, ZCode — through one MCP config entry. The harness is disposable; the brain is yours.
- **Evolves while you sleep.** A cron-driven dream loop compresses, dedupes, surfaces contradictions, and proposes curated diffs onto a git branch for review.
- **Token-lean by design.** Retrieval-stage filtering + the dream loop keep prompts small (measured 60–89% reduction vs naive paste-all on the `vault-min` fixture).

**Philosophy:** *"Humans abandon wikis because maintenance grows faster than value. LLMs don't get bored."*

---

## 2. Why KGX exists

Three failure modes dominate AI-assisted knowledge work today:

1. **Harness lock-in.** Your accumulated context lives inside Claude Code's session, Cursor's index, or Codex's history. Switch harness and you start over. ([Iusztin, "From Harness Lock-In to Portable Context Layer"](https://www.decodingai.com/p/the-context-layer))
2. **Wiki rot.** Notes go stale, contradictions accumulate, links die. LLMs can do this maintenance in their sleep.
3. **Context-window poverty.** Pasting the whole vault into every prompt is O(N) tokens and breaks at ~50 notes. Naive RAG (vector-only) misses multi-hop answers. grep misses semantics.

**KGX's wedge:** be the open, local-first engine that takes the [OKF bundle format](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md), adds hybrid retrieval + dream + a harness-plugin layer, and runs on a single static binary. Not code-only (like codebase-memory-mcp), not harness-coupled (like Cursor's index), not just-a-format (like OKF).

---

## 3. Architecture

### 3.1 Three layers, one binary

```
┌──────────────────────────────────────────────────────────────┐
│  HARNESS LAYER (disposable, interchangeable)                  │
│  Claude Code · Codex · Cursor · OpenCode · ZCode · curl       │
│  Wired by one MCP config entry + a /kgx:* skill pack         │
└──────────────────────────────────────────────────────────────┘
                              ↕ MCP (stdio) / HTTP JSON-RPC
┌──────────────────────────────────────────────────────────────┐
│  SERVING LAYER — the `kg` binary                              │
│  ┌──────────────┐  ┌──────────────┐  ┌─────────────────────┐ │
│  │ kg CLI       │  │ kg mcp-server│  │ kg serve (HTTP)     │ │
│  │ offline use  │  │ stdio JSON-  │  │ axum JSON-RPC +     │ │
│  │              │  │ RPC, 9 tools │  │ /hooks/conversation │ │
│  └──────┬───────┘  └──────┬───────┘  └──────────┬──────────┘ │
│         └─────────────────┴────────────────────┘             │
│                           ↕                                   │
│  UNIFIED TOOL REGISTRY (one dispatch core for all 3 fronts)  │
└──────────────────────────────────────────────────────────────┘
                              ↕
┌──────────────────────────────────────────────────────────────┐
│  MEMORY LAYER (owned by the user, on disk, git-versioned)     │
│                                                               │
│  CANONICAL: Markdown vault (OKF bundle)        ── git push ──►│
│    raw/         immutable sources                              │
│    notes/{facts,entities,decisions,experiences,               │
│           moc,questions,preferences,friction}/                │
│                                                               │
│  DERIVED (disposable, rebuildable):                           │
│    .kg/brain.sqlite                                            │
│      └── notes + edges + FTS5 + vec0(384-dim) +               │
│           pagerank + leiden + community_summaries + meta      │
└──────────────────────────────────────────────────────────────┘
```

### 3.2 Crate map (18 crates, ~8.6K LOC)

| Layer | Crate | Role |
|---|---|---|
| **Core types** | `kgx-core` | `Note`, `Edge`, `NoteType`, errors, LLM/Embedder traits |
| **I/O** | `kgx-vault` | Markdown parse/write, wikilink extraction |
| | `kgx-okf` | OKF validate + tar/gz bundle ship/pull |
| **Brain** | `kgx-graph` | SQLite schema, migrations, PageRank, Leiden, vec0, kNN, embeddings |
| | `kgx-store` | `BrainStore` trait, multi-project `BrainSet` |
| **Retrieval** | `kgx-retrieval` | RRF fusion, PPR, hybrid/keyword/semantic, community summaries, graph rerank |
| | `kgx-llm` | claude/openai/ollama/mock providers + embedder selection |
| | `kgx-extract` | raw → atomic facts pipeline + conversation ingest |
| **Consolidation** | `kgx-dream` | 7-pass dream engine + friction |
| | `kgx-ponytail` | Operation ladders (lite/full/ultra) + audit |
| | `kgx-rtk` | Output compression wrapper + `rtk` binary |
| **Surfaces** | `kgx-mcp` | JSON-RPC over stdio + HTTP, 9 tools |
| | `kgx-cli` | `kg` binary, 24 subcommands |
| | `kgx-cron` | launchd/systemd job rendering |
| | `kgx-tokens` | Token accounting + aggregation |
| **Viz / docs / eval** | `kgx-viz` | HTML (Three.js 3D) + DOT + Mermaid export |
| | `kgx-docs` | Use-case HTML generator |
| | `kgx-bench` | 3-arm corpus/gold/judge harness |

Layering is a clean DAG: `core` → `vault`/`graph`/`llm` → `retrieval`/`extract`/`dream` → `mcp`/`cli`.

---

## 4. The user lifecycle

```
        ┌──────────────────────────────────────────────────┐
        │                                                  ▼
   init ─► ingest ─► curate ─► index/graph ─► query/recall ─► refine/memoize
                                                          │
                                                          ▼
                            ┌────────────────────┐  re-index/graph
                            │  dream/compress    │ ◄─────────────────
                            │  (cron, nightly)   │
                            └─────────┬──────────┘
                                      ▼
                            review (git branch) ── approve ──► merge to main
                                      │
                                      ▼
                            visualize (3D WebGL) ──► self-research (cron)
```

| Stage | What the user does | What runs |
|---|---|---|
| **init** | `kg init --template code --okf --with-skills` | Scaffolds vault + per-harness MCP config + skills |
| **ingest** | `/kgx:capture`, `/kgx:ingest`, `ingest_url`, `ingest_conversation` | SHA-256 dedup → `raw/` immutable; source → facts pipeline |
| **curate** | Edit Markdown directly (Obsidian, vim, Cursor) | Humans see git diffs; LLMs see `upsert_note` |
| **index** | `kg index --full --communities --pagerank` | Rebuilds `.kg/brain.sqlite`: FTS5, vec0 embeddings, PageRank, Leiden |
| **query** | `/kgx:ask`, `/kgx:search`, `/kgx:recall`, MCP `nl_query_memory` | Hybrid RRF fusion; `recall` is pure graph (no LLM) |
| **refine** | `ingest_conversation` incremental → finalize | Converts chat into durable notes; routes ADD/UPDATE/MERGE/DEPRECATE onto dream |
| **dream** | `kg dream --max-iterations 3 --dry-run` → `kg review` | 7 passes: dedup, contradiction, supersession, staleness, community, orphan, open-questions |
| **visualize** | `kg graph --format html` | Three.js 3D force-directed viewer |
| **self-research** | `kg cron enable dream` | Nightly; writes proposals to git branch |

### Invariants enforced everywhere

1. **`raw/` is immutable** (T01 — hash unchanged after extract+dream).
2. **Supersede or archive; never delete notes.** Dream proposes; `kg review` approves. Hard blocks (contradictions) require a human.
3. **Cite note ids.** Every `ask` answer carries `cites: [01FACT…]`.
4. **Brain is disposable.** Wipe `.kg/`, re-index, get the same brain back (T10 — now byte-hash strict).
5. **Determinism.** Same vault + same seed → same PageRank, same Leiden partition.

---

## 5. Retrieval quality — the F1 story

### 5.1 Measured reality (verified 2026-07-04 on merged branch)

On `vault-min` (17 notes, 28 edges), all commands sub-10ms:

| Operation | Time | Result |
|---|---|---|
| `kg index --full --communities --pagerank` | 17ms | 17 nodes, 28 edges, 4 communities |
| `kg search "datastore" --mode hybrid` | 5ms | Top: Postgres, CockroachDB, CockroachDB-primary (BM25+RRF) |
| `kg ask "What is the primary datastore?" --cite` | 5ms | Correct answer + citation |
| `kg recall --entity "Postgres"` | 4ms | 12 ranked neighbors (ADR-001/002, CockroachDB, Billing, MOC, ...) |
| `kg dream --dry-run --max-iterations 3` | 6ms | 18 staged diffs, 6 hard blocks (contradictions) |

### 5.2 Headline benchmark numbers

**Sprint-9 vault (32 nodes / 57 edges), WITH-KGX vs WITHOUT-KGX:**

| Metric | WITHOUT KGX | WITH KGX | Improvement |
|---|---|---|---|
| Precision@5 | 0.122 | 0.347 | **+184%** |
| Recall@5 | 0.356 | 0.944 | **+165%** |
| F1@5 | 0.196 | 0.506 | **+158%** |
| MRR | 0.089 | 0.728 | **+718%** |
| NDCG@5 | 0.107 | 0.780 | **+629%** |

**Keyword search vs ripgrep (15-query gold set, Sprint 9):**

| Metric | ripgrep | kg keyword | vs rg |
|---|---|---|---|
| P@5 | 0.360 | **0.440** | +22% |
| F1 | 0.528 | **0.585** | +11% |
| NDCG@5 | 0.835 | **0.889** | +6.5% |

**Token efficiency (3-sprint sim, measured with `KGX_LLM=mock`):**

| Metric | WITHOUT KGX | WITH KGX | Savings |
|---|---|---|---|
| Q&A tokens (ask only) | ~44,400 | ~17,930 | **60%** |
| Total tokens (3 sprints) | ~73,200 | ~25,120 | **66%** |
| Session re-hydration waste | 11,600 | 0 | **100%** |
| Knowledge-mgmt overhead | ~119 min | <1 min | **99%** |
| Multi-hop question time | 12 min | <100 ms | **99%** |

**Honest caveat (from `sprint-simulation-benchmark-results.md`):** a real two-agent 10-day sprint measured **21% actual token reduction**, not the projected 73%. The gap is because real sessions ran more `kg ask` calls with longer prompts than the projection assumed. The 100% session re-hydration savings is real and verified. The impressive F1/MRR/NDCG numbers are mock-LLM-measured on retrieval (the retrieval is real; the synthesis is a stub), so answer-quality claims are forward-looking until Phase 7's `kgx-bench` ships real-LLM judging.

### 5.3 What's verified working vs what's deferred

**Verified working on the merged branch:**
- Real Leiden (modularity-gain local-moving, seed=42, deterministic) — `leiden.rs`
- sqlite-vec `vec0` ANN (proper `WHERE embedding MATCH … ORDER BY distance`) — `knn.rs:46-48` delegates to vec0
- `FastEmbedEmbedder` (all-MiniLM-L6-v2, 384-dim) behind `--features semantic` — verified by T27 (semantic neighbors beat distractors)
- PageRank with correct dangling-mass redistribution (sums to 1.000000)
- Pool-scoped PPR graph rerank — `hybrid.rs:200-246` `search_rerank_graph`

**Deferred to later phases:**
- `--rerank graph` / `--rerank llm` flags exist in `SearchOpts` but aren't threaded through the `kg search` CLI
- Multi-hop `ask` synthesis expansion (mock LLM hardcodes "Postgres is primary" for any ANSWER_QUESTION — quality needs real-LLM bench)
- 50-query gold set + LLM-as-judge (Phase 7 `kgx-bench`)

---

## 6. Phase 0 fixes (just landed, all verified)

The merged branch includes Phase 0 — four real correctness bugs fixed, each with a TDD test. Re-verification showed 5 of the 9 bugs in earlier audits were **already fixed**; only these four were real:

| Fix | T-spec | Verification (live on release binary) |
|---|---|---|
| `--incremental` only re-embeds changed notes (content-hash diff, was re-embedding all 17 every run) | T19 | No-op incremental: 0 tokens (was 370) |
| Break MOC feedback loop (Leiden excludes `type:moc` from input + deterministic MOC ids; was growing 17→21→25→… unboundedly) | T25 | 3 consecutive runs: stable at 20 nodes, 4 communities |
| `ingest_file` uses real SHA-256, not SipHash (was 16-char SipHash advertised as "sha256") | T26 | MCP returns 64-char lowercase hex digest |
| `query_memory.project` field removed (was declared in schema, silently ignored) | — | `tools/list` shows `note_type, tag, status, limit` only |
| T10 upgraded from count-only to byte-hash brain fingerprint (catches silent score drift) | T10 | 3 rebuilds from same on-disk state produce identical fingerprint |
| Real-embedding e2e test (semantic neighbors beat distractors) | T27 | 2/2 pass with `--features semantic` (opt-in) |
| Stale T09 TODO removed; production code marker-clean | — | `grep` confirms zero TODO/FIXME/unimplemented!/panic! |

---

## 7. Surfaces — skills, CLI, MCP, install

### 7.1 The 17 `kgx:*` skill verbs (composite, harness-portable)

Each is a thin SKILL.md wrapping the right `kg` command. Skills are duplicated across `skills/claude/.claude/skills/` and `skills/opencode/.opencode/skills/` (a `kgx-ask` → `kgx:ask` rename refactor is in flight in the working tree).

| Verb | Underlying command | When to use |
|---|---|---|
| `/kgx:init` | `kg init --template … --okf --with-skills` | New vault |
| `/kgx:capture` | `kg capture --from - --type doc` | Save raw source |
| `/kgx:ingest` | capture + `kg extract --source … --intensity full` | Capture + extract in one shot |
| `/kgx:extract` | `kg extract --source …` | Atomic facts from a source |
| `/kgx:index` | `kg index --full --communities` | Build the brain |
| `/kgx:search` | `kg search … --mode hybrid` | Keyword/vector/graph search |
| `/kgx:ask` | `kg ask … --cite` | Q&A with citations |
| `/kgx:recall` | `kg recall --entity …` | Entity neighborhood (no LLM) |
| `/kgx:link` | `kg link [--fix]` | Wikilink repair |
| `/kgx:dream` | `kg dream --max-iterations 3` → `kg review` | Consolidation |
| `/kgx:review` | `kg review [--approve all\|--reject]` | Approve staged diffs |
| `/kgx:status` | `kg status` | Vault health |
| `/kgx:ship` | `kg ship --version … --name …` | OKF bundle export |
| `/kgx:sync` | `kg sync` | Git pull + reindex |
| `/kgx:codebase` | `kg codebase search/trace/architecture` | Code graph (shells to codebase-memory-mcp) |
| `/kgx:codebase-index` | `kg codebase index` | Index repo into code graph |

### 7.2 The 9 MCP tools (harness-facing)

Verified by E2E test on the merged branch — all 9 dispatch via stdio AND HTTP:

`nl_query_memory · query_memory · deep_search_memory · get_note · ingest_conversation · ingest_file · ingest_url · upsert_note · dream_step`

### 7.3 The `kg` CLI — 24 subcommands

`init · capture · extract · index · search · ask · recall · link · dream · review · graph · status · validate · tokens · project · serve · mcp-server · ship · pull · sync · codebase · cron · dashboard · docs`. All support `--json` → `{"ok":bool,"command":"...","data":{},"elapsed_ms":N}`.

### 7.4 Install — one command per harness

```bash
./dev-install.sh --agent claude|opencode|codex|cursor   # dev install
curl -fsSL https://raw.githubusercontent.com/thanhNt16/kgx/main/install.sh | bash  # release
```

| Agent | MCP wiring | Skills/rules |
|---|---|---|
| Claude Code | `claude mcp add --transport stdio kgx -- kg mcp-server` | `~/.claude/skills/kgx*/SKILL.md` |
| Cursor | `.cursor/mcp.json` | `.cursor/rules/kgx.mdc` |
| Codex | `config.toml` `[mcp_servers.kgx]` | `AGENTS.md` |
| OpenCode | `opencode.json` `mcp.kgx` | `.opencode/skills/kgx*/SKILL.md` |
| ZCode *(target, Phase 8)* | `.mcp.json` `mcpServers.kgx` | `~/.zcode/skills/kgx*/SKILL.md` |

---

## 8. The dream loop — 7 passes, proposal-only (or auto-soft post-Phase 7)

| Pass | What it proposes | Class |
|---|---|---|
| `dedup` | Merge near-identical facts, archive loser, repoint edges | soft |
| `contradiction` | Detect "X is primary" vs "Y is primary" | **hard** (proposal only) |
| `supersession` | Newer decision supersedes older, mark old `archived` | soft |
| `staleness` | Note untouched > N days with no inbound edges → archive | soft |
| `community` | Generate/update community summaries (GraphRAG) | soft |
| `orphan_repair` | Link notes with zero inbound edges to a MOC | soft |
| `open_questions` | Surface unanswered `question` notes; propose resolution | soft |

Verified: `kg dream --dry-run` on the fixture stages 18 diffs and flags 6 hard blocks in 6ms — no LLM call. The contradiction pass catches planted "Postgres primary vs CockroachDB primary."

Phase 7 will route soft diffs to auto-apply (with an audit log at `.kg/dream-audit.jsonl` and `kg dream --undo <token>` for one-command revert); hard blocks always wait for review.

---

## 9. Testing & verification

### 9.1 Hermetic suite (default)

```bash
KGX_LLM=mock cargo test --workspace
→ 101 passed; 0 failed
```

| Spec | Verifies |
|---|---|
| T01 | `raw/` hash unchanged after extract+dream |
| T02 | Extract yields ≥1 fact per source with provenance |
| T03 | Zero phantom wikilinks |
| T04 | Exactly 1 orphan detected |
| T05–T08, T14, T15 | Dream stages → review applies soft, blocks hard |
| T06 | Dedup merges, archives, repoints edges |
| T10 | Brain byte-hash identical across rebuilds (Phase 0 upgrade) |
| T11 | Fresh init + validate passes; ship→pull round-trip |
| T12 | HTML graph counts match brain.sqlite |
| T13 | ≥3 communities each with summary + MOC, internally connected |
| T16 | `kg tokens` matches per-command records |
| T17 | RTK compression or graceful fallback |
| T18 | `--ponytail-audit` flags over-engineered diffs |
| **T19** (new) | `--incremental` touches only changed note rows |
| **T25** (new) | `--communities` idempotent across runs (MOC feedback broken) |
| **T26** (new) | `ingest_file` hash is real SHA-256 (64 hex chars) |

### 9.2 Opt-in semantic test

```bash
cargo test --package kgx-graph --features semantic --test semantic_e2e
→ 2 passed (model downloads ~40MB on first run, cached thereafter)
```

**T27** verifies that two phrases with zero word overlap ("store information in my application" vs "persisting records") embed closer than an unrelated distractor — the core semantic-retrieval contract mock embeddings cannot test.

---

## 10. Configuration

### LLM providers

```bash
export KGX_LLM=claude           # best for extraction + dreaming (needs ANTHROPIC_API_KEY)
export KGX_LLM=openai           # (needs OPENAI_API_KEY)
export KGX_LLM=ollama           # local, offline
export KGX_LLM=mock             # hermetic testing, no API calls (default for tests)
```

### Embeddings

```bash
# Default: mock (deterministic FNV hash, no semantic meaning) — for hermetic tests
cargo build --release

# Real semantic: fastembed + all-MiniLM-L6-v2 (384-dim)
cargo build --release --features kgx-cli/semantic
KGX_EMBED=fastembed kg index --full   # downloads ~40MB model on first run
```

### Key environment variables

| Variable | Default | Purpose |
|---|---|---|
| `KGX_LLM` | `claude` | LLM provider |
| `KGX_EMBED` | (mock) | `fastembed` to enable real embeddings |
| `KGX_HOME` | `~/.kgx` | Home data directory |
| `KGX_RTK_DISABLE` | — | `1` disables RTK compression |

### Note types

`fact · entity · decision · experience · moc · source · question · preference · friction`

---

## 11. Roadmap (post-Phase 0)

| Phase | Theme | Status |
|---|---|---|
| **0** | Foundation: determinism, MOC loop, hash honesty | ✅ **DONE (this branch)** |
| **1** | Ship `--features semantic` on by default; thread `--rerank graph` through CLI | Next |
| **2** | GraphRAG global retrieval; multi-hop `ask` context expansion | Pending |
| **3** | Unified tool registry hardening (Phase 4 per-project brains) | Pending |
| **4** | Per-project brains + `home` scope | Pending |
| **5** | Continual learning loop (conversation ingest + refine routing) | Pending |
| **6** | `briefing://` MCP resource; Docker Compose deploy | Pending |
| **7** | `kgx-bench` 3-arm kill-criterion; auto-apply-soft dream + audit log + undo | Pending |
| **8** | Polish: ZCode first-class; RTK widening; skill dedup; viz timeline | Pending |

---

## 12. Doc Index

The historical artifacts remain in their original paths for traceability. This section is the map.

### 12.1 Authoritative (read first)
- **This file** — `docs/KGX-FINAL.md`
- `README.md` — install + quickstart + command reference
- `AGENTS.md` — the `kgx:*` composite-verb contract for agents
- `CLAUDE.md` — agent behavior contract

### 12.2 Specs (design decisions)
- `docs/superpowers/specs/2026-07-04-kgx-unified-prd.md` — the unified PRD (this doc consolidates it)
- `docs/superpowers/specs/2026-06-28-install-command-design.md` — per-harness install design
- `docs/superpowers/specs/2026-06-28-retrieve-graph-rerank-design.md` — pool-scoped PPR rerank
- `docs/superpowers/specs/2026-07-03-3d-graph-viewer-design.md` — Three.js viewer architecture

### 12.3 Plans (implementation phase plans)
- `docs/superpowers/plans/2026-07-04-kgx-phase0-real-bugs.md` — **executed** (this branch)
- `docs/superpowers/plans/2026-07-02-kgx-unified-rewrite.md` — the master phase plan (0–8)
- `docs/superpowers/plans/2026-06-27-kgx-master-plan.md` — original orchestration plan
- `docs/superpowers/plans/2026-06-27-kgx-phase{0..6}-*.md` — original per-phase plans (historical)
- `docs/superpowers/plans/2026-07-03-3d-graph-viewer.md` — 3D viewer implementation (executed)

### 12.4 Benchmarks & evidence
- `docs/eval-results.md` — 3-sprint retrieval benchmark (the +184%/+165%/+158% numbers)
- `docs/sprint-simulation.md` — DataLake 2.0 single-sprint simulation
- `docs/sprint-simulation-benchmark-results.md` — **the honest two-agent re-run** (21% actual token reduction, not 73% projected)
- `STATUS.md` — MVP status snapshot (2026-06-27)

### 12.5 Interactive demos (HTML, open in browser)
- `docs/kgx-guide.html` — main marketing + reference page
- `docs/kgx-graph-demo.html` — 3D graph viewer demo
- `docs/kgx-demo-flow.html` — user-flow walkthrough
- `docs/kgx-real-benchmark.html` — 4-agent benchmark visualization
- `docs/kgx-36sprints-graph.html` + `docs/kgx-36sprints-report.html` — 18-month vault-growth simulation
- `docs/kgx-6mo-simulation.html` — 6-month trajectory

### 12.6 Superseded (kept for history; do not act on)
- `prd.md` — original CLI-only PRD (consolidated into §1–§8 here)
- `context-layer.md` — Mongo-era context-layer PRD (the SQLite merge resolved it; see `2026-07-02-kgx-unified-rewrite.md` for the reconciliation table)

---

## 13. Sources

- [From Harness Lock-In to Portable Context Layer — Paul Iusztin, Decoding AI](https://www.decodingai.com/p/the-context-layer)
- [OKF SPEC.md — Google Cloud (GoogleCloudPlatform/knowledge-catalog)](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md)
- [How the Open Knowledge Format can improve data sharing — Google Cloud Blog](https://cloud.google.com/blog/products/data-analytics/how-the-open-knowledge-format-can-improve-data-sharing)
- [DeusData/codebase-memory-mcp — codebase knowledge graph MCP](https://github.com/DeusData/codebase-memory-mcp)
- [Karpathy LLM Wiki gist](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f) · [GitHub topic](https://github.com/topics/karpathy-llm-wiki)
- [geronimo-iia/llm-wiki — headless wiki MCP, Rust](https://github.com/geronimo-iia/llm-wiki)
- [Knowledge Graph Based Repository-Level Code Generation — arXiv 2505.14394](https://arxiv.org/html/2505.14394v1)
