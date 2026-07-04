# KGX Unified — Portable Context Layer PRD

> **Status:** Spec / PRD (not an implementation plan). The companion implementation plan is `docs/superpowers/plans/2026-07-02-kgx-unified-rewrite.md` (phase-level, acceptance-gated).
> **Date:** 2026-07-04
> **Supersedes:** `prd.md` (CLI-only) and `context-layer.md` (Mongo-era). This doc is the authoritative merge onto the SQLite + sqlite-vec backend described in the unified-rewrite plan.
> **Author:** KGX

---

## 0. TL;DR

KGX is a **local-first, harness-agnostic, self-evolving knowledge graph** for humans and AI agents.

- **One `kg` binary** (12.5 MB, statically linked, no server required) that turns a plain Markdown vault into a queryable brain: hybrid vector + BM25 + graph retrieval, PageRank, Leiden communities, an LLM-driven dream/consolidation loop, and a 3D WebGL graph viewer.
- **Plugs into any harness** — Claude Code, Codex, Cursor, OpenCode, ZCode — through one MCP config entry. The harness is disposable; the brain is yours.
- **Evolves while you sleep.** A cron-driven self-research + dream loop compresses, dedupes, surfaces contradictions, and proposes curated diffs onto a git branch for human review. Recall and precision improve over time without manual gardening.
- **Token-lean by design.** RTK output compression + Ponytail operation ladders + retrieval-stage filtering keep prompts small (measured 60–89% reduction vs naive paste-all on the `vault-min` fixture).

**What this PRD changes vs what already ships:** consolidates two competing PRDs into one direction, hardens correctness bugs verified by real tests (PageRank leak, brute-force KNN, fake communities, mock-only embeddings, weak synthesis), and sharpens the F1 / kill-criterion story so the retrieval investment can be defended with numbers, not vibes.

---

## 1. Problem

Three failure modes dominate AI-assisted knowledge work today:

1. **Harness lock-in.** Your accumulated context — decisions, preferences, entity graph — lives inside Claude Code's session, Cursor's index, or Codex's history. Switch harness and you start over. ([Iusztin, "From Harness Lock-In to Portable Context Layer"](https://www.decodingai.com/p/the-context-layer))
2. **Wiki rot.** Humans abandon wikis because maintenance cost grows faster than value. Notes go stale, contradictions accumulate, links die. LLMs don't get bored — they can do this maintenance in their sleep.
3. **Context-window poverty.** Pasting the whole vault into every prompt is O(N) tokens and breaks at ~50 notes. Naive RAG (vector-only) misses multi-hop answers. grep misses semantics.

Existing pieces address fragments:

| System | What it solves | What it lacks |
|---|---|---|
| **Obsidian + graph plugins** | Human-facing vault, backlinks, graph view | No AI-native retrieval, no auto-consolidation, harness-coupled |
| **Karpathy's "LLM Wiki" gist** + descendants ([topic](https://github.com/topics/karpathy-llm-wiki)) | The "compounding knowledge" idea | Mostly prompts + docs; no canonical engine, no graph-layer retrieval |
| **[DeusData/codebase-memory-mcp](https://github.com/DeusData/codebase-memory-mcp)** | Codebase knowledge graph over MCP, tree-sitter + LSP | Code-only; no general notes, no dream loop, no Markdown-canonical vault |
| **[Google OKF](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md)** | Portable, git-diffable knowledge bundle format | A spec, not an engine — explicitly punts storage/serving/query |
| **Mem0 / HippoRAG / LazyGraphRAG (papers)** | Memory consolidation, PPR retrieval, cheap global summarization | Research artifacts; no productized CLI/MCP/skill bundle |

**KGX's wedge:** be the open, local-first engine that takes the OKF bundle format, adds the retrieval + dream + dream loop + harness-plugin surfaces, and runs on a single static binary. Not code-only (like codebase-memory-mcp), not harness-coupled (like Cursor's index), not just-a-format (like OKF).

---

## 2. Goals & non-goals

### Goals (this PRD)

| # | Goal | Success metric | Verified? |
|---|---|---|---|
| G1 | Markdown-canonical: brain is fully disposable | `rm -rf .kg && kg index --full` → byte-identical brain across 2 runs | ✅ T10 (today: counts match; this PRD upgrades to byte-hash) |
| G2 | OKF-conformant bundle | `kg validate --okf` passes; ship→pull round-trip is lossless | ✅ T11 |
| G3 | Harness-portable | One MCP config entry wires Claude/Codex/Cursor/OpenCode/ZCode | ✅ today (4 agents; +ZCode target) |
| G4 | Retrieval F1 beats grep + vector-only | kg hybrid F1 > ripgrep F1 AND kg hybrid F1 > kg semantic-only F1 on a 50-query gold set | ⚠️ partial — today's gold set is 3 queries (stale); PRD adds a real 50-Q set |
| G5 | Dream is unattended-safe | ≥90% of staged diffs are non-destructive (soft/archive/merge); hard blocks always surface for review | ✅ T05–T08 |
| G6 | Token efficiency | ≥60% input-token reduction vs naive paste-all on `vault-min` (measured, mock LLM) | ✅ today: 89% on fixture |
| G7 | Self-evolving | A scheduled cron job runs `kg dream` + `kg codebase-index` + a friction digest weekly without human poking | ⚠️ cron helpers exist; auto-research + digest not wired |
| G8 | Real embeddings, real ANN, real Leiden | fastembed MiniLM replaces mock; sqlite-vec replaces O(N) cosine; real Leiden replaces connected-components | ❌ today — all three are stubs (verified by `index.rs:19`, `knn.rs:15`, `community.rs:49`) |

### Non-goals

- **Hosted SaaS.** Local-first forever. Docker Compose is a deployment convenience, not a cloud product.
- **Multi-writer concurrent DB.** All brain writes serialize through one process (CLI, or `kg serve` for HTTP). SQLite is correct because we control the write path.
- **>10K notes per project.** Design envelope is **personal scale**. SQLite + sqlite-vec `vec0` ANN handles this comfortably on a laptop. Above 10K, shard by project (`kg project add`) before considering anything heavier. Neo4j/external-graph export stays a non-goal.
- **Permissions / RBAC / secret redaction.** Explicitly OKF non-goals too. Use git for access control.
- **A new note-format spec.** KGX conforms to OKF; it does not compete with it.

### Confirmed decisions (2026-07-04)

| Decision | Choice | Implication |
|---|---|---|
| Build order | **Correctness first (P0–P2)** | Fix PageRank, ANN, Leiden, embeddings, multi-hop synthesis before any new surface. Every later benchmark is then defensible. |
| Scale target | **Personal (≤10K notes)** | SQLite forever; no sharding; static 12.5MB binary; sqlite-vec is the only ANN we need. |
| Dream autonomy | **Auto-apply soft, propose hard** | Soft diffs (dedup, staleness, archive, orphan-repair, community, open-questions) auto-merge on the nightly cron. Hard blocks (contradictions) always wait for `/kgx:review`. Requires an audit log + one-command undo (see §7). |
| ZCode tier | **First-class harness** | `dev-install.sh --agent zcode`, `.mcp.json` template, mirrored skills in `~/.zcode/skills/kgx*/`, CI parity with claude/cursor/codex/opencode. |

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
│  nl_query · query · deep_search · get · upsert ·             │
│  ingest_file · ingest_url · ingest_conversation · dream_step │
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
│    ~/brains/<project>/.kg/brain.sqlite                        │
│      └── notes + edges + FTS5 + vec0(384-dim) +               │
│           pagerank + leiden + community_summaries + meta      │
│    ~/brains/home/.kg/brain.sqlite   (cross-cutting)           │
└──────────────────────────────────────────────────────────────┘
```

### 3.2 Crate map (18 crates today, ~8.6K LOC)

| Layer | Crate | Role | LOC |
|---|---|---|---|
| **Core types** | `kgx-core` | `Note`, `Edge`, `NoteType`, errors, LLM trait | 350 |
| **I/O** | `kgx-vault` | Markdown parse/write, wikilink extraction | 242 |
| | `kgx-okf` | OKF validate + tar/gz bundle ship/pull | 193 |
| **Brain** | `kgx-graph` | SQLite schema, migrations, PageRank, Leiden, vec0, kNN | 1528 |
| | `kgx-store` | `BrainStore` trait, multi-project `BrainSet` | 116 |
| **Retrieval** | `kgx-retrieval` | RRF fusion, PPR, hybrid/keyword/semantic, community summaries | 768 |
| | `kgx-llm` | claude/openai/ollama/mock providers + embedder | 355 |
| | `kgx-extract` | raw → atomic facts pipeline + conversation ingest | 365 |
| **Consolidation** | `kgx-dream` | 7-pass dream engine (dedup, contradiction, supersession, staleness, community, orphan, open_questions) + friction | 1010 |
| | `kgx-ponytail` | Operation ladders (lite/full/ultra) + audit | 109 |
| | `kgx-rtk` | Output compression wrapper + `rtk` binary | 216 |
| **Surfaces** | `kgx-mcp` | JSON-RPC over stdio + HTTP, 9 tools | 781 |
| | `kgx-cli` | `kg` binary, 24 subcommands | 2057 |
| | `kgx-cron` | launchd/systemd job rendering | 251 |
| | `kgx-tokens` | Token accounting + aggregation | 92 |
| **Viz / docs** | `kgx-viz` | HTML (Three.js 3D) + DOT + Mermaid export | 156 |
| | `kgx-docs` | Use-case HTML generator | 81 |
| **Eval** | `kgx-bench` | 3-arm corpus/gold/judge harness | 176 |

Layering is a clean DAG: `core` → `vault`/`graph`/`llm` → `retrieval`/`extract`/`dream` → `mcp`/`cli`. **Verified: zero TODO/FIXME/`unimplemented!`/`panic!` markers in production code.**

---

## 4. The user lifecycle (the canonical flow)

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

### 4.1 Each stage, mapped to tools

| Stage | What the user does | What runs (CLI · MCP · cron) |
|---|---|---|
| **init** | `kg init --template code --okf --with-skills` (or `/kgx:init`) | Scaffolds vault + writes per-harness MCP config + skills |
| **ingest** | `/kgx:capture` (paste), `/kgx:ingest` (file+extract), `ingest_url`, `ingest_conversation` (auto, ~10 turns) | SHA-256 dedup → `raw/` immutable; source note → facts pipeline |
| **curate** | Edit Markdown directly (Obsidian, vim, Cursor) | Humans see git diffs; LLMs see `upsert_note` |
| **index / graph** | `kg index --full --communities --pagerank` (or `/kgx:index`) | Rebuilds `.kg/brain.sqlite`: FTS5, vec0 embeddings, PageRank, Leiden communities, summaries |
| **query / recall** | `/kgx:ask`, `/kgx:search`, `/kgx:recall`, MCP `nl_query_memory` | Hybrid RRF fusion (BM25 + vector + PPR + entity-NER expansion); `recall` is pure graph (1-hop, no LLM) |
| **refine / memoize** | `ingest_conversation` incremental → finalize | Converts ephemeral chat into durable `preference`/`fact`/`decision` notes; routes ADD/UPDATE/MERGE/DEPRECATE onto dream passes |
| **re-index** | Re-run `kg index` (deterministic, idempotent) | `--incremental` touches only changed notes (PRD fixes today's no-op) |
| **dream / compress** | `kg dream --max-iterations 3 --dry-run` then `kg review --approve all --ponytail-audit` | 7 passes: dedup, contradiction, supersession, staleness, community, orphan-repair, open-questions. Hard blocks never auto-apply. |
| **visualize** | `kg graph --format html --out g.html` | Three.js 3D force-directed viewer (dark theme, bloom, sprite labels, filter sidebar, auto-rotate, fit/auto-rotate buttons) |
| **self-research (cron)** | `kg cron enable dream` + `kg cron enable codebase-index` + weekly friction digest | launchd/systemd unit runs nightly; writes proposals to `brain/dream/<date>/` git branch for review |

### 4.2 Invariants enforced everywhere

1. **`raw/` is immutable.** ✅ T01 — hash unchanged after extract+dream.
2. **Supersede or archive; never delete notes.** Dream proposes; `kg review` approves. Hard blocks (contradictions) require a human.
3. **Cite note ids.** Every `ask` answer carries `cites: [01FACT…]`.
4. **Brain is disposable.** Wipe `.kg/`, re-index, get the same brain back.
5. **Determinism.** Same vault + same seed → same PageRank, same Leiden partition.

---

## 5. Retrieval quality — the F1 story

This is where the PRD sharpens today's hand-wavy "beats grep" claim into a defensible number.

### 5.1 Today's measured reality (verified by me, 2026-07-04, mock LLM)

On `vault-min` (17 nodes, 28 edges):

| Operation | Time | Result |
|---|---|---|
| `kg index --full --communities --pagerank` | 17ms | 17 nodes, 28 edges ✅ |
| `kg ask "What is the primary datastore?" --cite` | 5ms | Correct: "Postgres is the primary datastore" + cite ✅ |
| `kg ask "What depends on Postgres?" --cite` | 5ms | **Wrong** — returns the same "primary datastore" answer (synthesis doesn't follow the entity edge to Billing Service) ⚠️ |
| `kg recall --entity "Postgres"` | 4ms | 12 ranked neighbors incl. ADR-001/002, CockroachDB, Billing Service, MOC ✅ |
| `kg dream --dry-run --max-iterations 3` | 6ms | 18 staged diffs, 6 hard blocks (contradictions) ✅ |

On the published 15-query benchmark (Sprint 9 vault, 32 nodes / 57 edges):

| Metric | ripgrep | kg keyword | kg hybrid (target) |
|---|---|---|---|
| P@5 | 0.360 | **0.440 (+22%)** | TBD |
| F1 | 0.528 | **0.585 (+11%)** | TBD |
| NDCG@5 | 0.835 | **0.889 (+6.5%)** | TBD |

### 5.2 The honest gaps

**Re-verified 2026-07-04 against the actual code (not the rewrite plan's stale audit).** Several bugs that earlier audits listed turn out to be already fixed. The table below separates fact from folklore.

**Already fixed (do not re-fix — verified empirically):**

| Alleged bug | Reality | Evidence |
|---|---|---|
| PageRank dangling-node mass leak | **Fixed** — `pagerank.rs:43-60` redistributes `dangling_mass / n` | `SELECT printf('%.6f', sum(score)) FROM pagerank` = `1.000000` on `vault-min` |
| `build_full` doesn't clear derived tables | **Fixed** — `build.rs:156-161` DELETEs notes, edges, fts, pagerank, communities, summaries, notes_vec | Inspected the execute_batch string |
| `meta` table never written | **Fixed** — `build.rs:112-145` writes `last_index`, `node_count`, `edge_count`, `build_mode` | `SELECT key,value FROM meta` returns 4 rows |
| Communities = connected-components | **Fixed** — `leiden.rs` is a real modularity-gain local-moving algorithm, seeded (default 42), deterministic | `SELECT count(DISTINCT community_id) FROM communities` = `4` on fixture |
| KNN brute-force O(N) | **Fixed** — `knn.rs:46-48` delegates to vec0 `knn_search` when `notes_vec` exists | `notes_vec` table populated; `vec::knn_search` issues `WHERE embedding MATCH … ORDER BY distance` |

**Still real (fix in Phase 0 — see `docs/superpowers/plans/2026-07-04-kgx-phase0-real-bugs.md`):**

| Bug | Where | Impact |
|---|---|---|
| `--incremental` re-embeds every note | `index.rs:86-114` `find_changed_ids` flags all existing notes as changed (set arithmetic bug) | Incremental is a full re-embed in disguise; O(N) per run |
| MOC feedback loop | `index.rs:51` generates a fresh ULID per community MOC on every `--communities` run | Node count grows unboundedly (17 → 21 → 25 …) |
| `ingest_file` hash is SipHash, not SHA-256 | `kgx-mcp/src/tools/ingest_file.rs:49-55` | The "idempotent by sha256" description and frontmatter `hash:` field are misleading; collision space is 64-bit not 256-bit |
| `query_memory.project` filter declared, silently ignored | `kgx-mcp/src/tools/query.rs` (no `project` handling) | Per-project MCP queries can't scope; honest fix is to remove the field until Phase 4 adds real per-project brains |
| Real embeddings ship disabled + untested e2e | `embed.rs` `FastEmbedEmbedder` exists behind `--features semantic`, but the release binary uses mock and no test asserts semantic retrieval works | The "kg hybrid beats semantic-only" claim is undefended |
| `ask` synthesis cannot be quality-tested under mock | `mock.rs:28-33` hardcodes "Postgres is primary" for any `ANSWER_QUESTION` prompt | Multi-hop answer quality is a real-LLM concern, not a mock-testable one |

**Deferred to later phases (not Phase 0):**

| Item | Phase |
|---|---|
| Gold set is 3 queries, one stale (Redis) | Phase 7 (`kgx-bench` 50-query corpus) |
| Multi-hop `ask` retrieval polish | Phase 2 (GraphRAG global + pool-scoped PPR consumed by `ask`) |

### 5.3 The F1 plan (this PRD's central quality investment)

**Target: kg hybrid F1@5 ≥ 0.70 on a 50-query gold set, beating both ripgrep and vector-only.**

Most of the foundation is already built and verified. The remaining work:

1. **Ship real embeddings on by default** (Phase 0 → Phase 8). `FastEmbedEmbedder` exists and works behind `--features semantic` + `KGX_EMBED=fastembed`. Phase 0 adds the e2e test (T27) proving retrieval works; Phase 8 makes `semantic` a default feature so the release binary uses real MiniLM.
2. **sqlite-vec `vec0` ANN — already wired** (verified). `knn.rs:46-48` delegates to vec0; `vec::knn_search` issues a proper `WHERE embedding MATCH … ORDER BY distance` query. The Phase 0 task here is just to add the `EXPLAIN` assertion (T20) so a future regression to brute-force would fail loudly.
3. **Real Leiden — already wired** (verified). `leiden.rs` is a real modularity-gain local-moving algorithm. T13 was passing vacuously before but now passes for the right reason; T21 adds a planted-3-community fixture to make the determinism claim defensible.
4. **Pool-scoped PPR rerank** (already specced in `2026-06-28-retrieve-graph-rerank-design.md`, code at `hybrid.rs:200-246` `search_rerank_graph`). Phase 2 work is to expose it via `kg search --rerank graph` (the flag exists in `SearchOpts` but isn't threaded through the CLI today).
5. **Synthesis that uses the graph** (Phase 2). When `ask` retrieves facts, expand 1-hop via `kg recall` before synthesis so multi-hop questions get multi-hop evidence. **Note:** the apparent multi-hop failure I saw is a MockProvider artifact (it hardcodes "Postgres is primary" for any `ANSWER_QUESTION`); real-LLM quality testing happens in Phase 7's `kgx-bench`.
6. **A real gold set** (Phase 7, `kgx-bench`). Replace the 3-query `qa.json` with 50 queries across 7 suites (S1 needle, S2 decision, S3 multi-hop, S4 cross-session, S5 preference, S6 cold-start, S7 negative). LLM-as-judge + 20% human spot-check.

### 5.4 The kill criterion (stolen from `context-layer.md` §11, made real)

`kgx-bench` runs 3 arms on identical tasks:

- **Arm A — no memory.** The harness answers from its parametric weights only.
- **Arm B — raw folder + grep.** The harness gets the `.brain/` Markdown tree and ripgrep.
- **Arm C — kg full.** The harness gets the MCP serving layer.

**Kill criterion: uplift = Arm C − Arm B.** If Arm C does not clearly beat Arm B on S1–S3 (needle / decision / multi-hop), the serving layer is not earning its complexity and we simplify. This is the only honest way to defend the wedge.

---

## 6. Surfaces — skills, CLI, MCP, install

### 6.1 The 17 `kgx:*` skill verbs (composite, harness-portable)

Each is a thin SKILL.md that wraps the right `kg` command. The skill layer is the human-facing surface; the CLI is the agent-facing surface; the MCP tools are the harness-facing surface. All three dispatch to the same core.

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
| `/kgx:dream` | `kg dream --max-iterations 3` then `kg review --approve all` | Consolidation |
| `/kgx:review` | `kg review [--approve all|--reject]` | Approve staged diffs |
| `/kgx:status` | `kg status` | Vault health |
| `/kgx:ship` | `kg ship --version … --name …` | OKF bundle export |
| `/kgx:sync` | `kg sync` | Git pull + reindex |
| `/kgx:codebase` | `kg codebase search/trace/architecture` | Code graph (shells to codebase-memory-mcp) |
| `/kgx:codebase-index` | `kg codebase index` | Index repo into code graph |
| `/kgx:graph` *(new — this PRD)* | `kg graph --format html` | 3D WebGL visualization |

### 6.2 The 9 MCP tools (harness-facing)

All registered in `kgx-mcp/src/tools/mod.rs:15-27`. Verified by my tests — none are stubbed.

| Tool | Args | Returns |
|---|---|---|
| `nl_query_memory` | `query`, `limit`, `mode`, `scope` | `scope=="global"` → LLM answer + citations; else search hits |
| `query_memory` | `note_type`, `tag`, `project` *(unused today)*, `status`, `limit` | Structured note list |
| `deep_search_memory` | `query`, `limit` | Clustered drill-down → `.kg/wiki-cache/<slug>/` |
| `get_note` | `id` | Full note body + path |
| `ingest_file` | `content`, `kind` *(unused)*, `hash` *(unused)* | `{status, raw, hash}` — idempotent by content hash |
| `ingest_url` | `url`, `kind` *(unused)* | `{status, raw, url}` |
| `ingest_conversation` | `turns[]`, `action: incremental\|finalize` | `{notes_created, notes_updated, decisions}` |
| `upsert_note` | `type`, `title`, `body`, `id?` | `{id, path}` |
| `dream_step` | `only`, `max_iterations` | `{iterations, diffs}` |

**Cleanup items this PRD commits to:** fix `project` filtering; replace SipHash with SHA-256 in `ingest_file`; honor `kind` or remove it; add `briefing://` MCP resource (Phase 6).

### 6.3 The `kg` CLI — 24 subcommands

`init · capture · extract · index · search · ask · recall · link · dream · review · graph · status · validate · tokens · project · serve · mcp-server · ship · pull · sync · codebase · cron · dashboard · docs`. Every command supports `--json` → `{"ok":bool,"command":"...","data":{},"elapsed_ms":N}`.

### 6.4 Install — one command per harness

```bash
# From source (dev)
./dev-install.sh --agent claude|opencode|codex|cursor

# Release
curl -fsSL https://raw.githubusercontent.com/thanhNt16/kgx/main/install.sh | bash
```

| Agent | MCP wiring | Skills/rules | Hook |
|---|---|---|---|
| Claude Code | `claude mcp add --transport stdio kgx -- kg mcp-server` | `~/.claude/skills/kgx*/SKILL.md` | — |
| Cursor | `.cursor/mcp.json` | `.cursor/rules/kgx.mdc` | — |
| Codex | `config.toml` `[mcp_servers.kgx]` | `AGENTS.md` | `hooks.json` Stop hook |
| OpenCode | `opencode.json` `mcp.kgx` | `.opencode/skills/kgx*/SKILL.md` | JS plugin |
| **ZCode** *(new — this PRD)* | `.mcp.json` `mcpServers.kgx` | `~/.zcode/skills/kgx*/SKILL.md` | SessionStart hook (optional) |

**Gap to close:** ZCode isn't in `dev-install.sh` yet. Adding it is one `--agent zcode` branch + a `.mcp.json` template (ZCode uses the same MCP-stdio contract as Cursor).

### 6.5 Skill dedup (this PRD)

Today the 17 `kgx:*` SKILL.md files are **duplicated verbatim** across `skills/claude/.claude/skills/` and `skills/opencode/.opencode/skills/`. Cursor and Codex carry a single rules file each. This PRD consolidates: one canonical `skills/kgx/<verb>/SKILL.md` source-of-truth, agent-specific installers copy or symlink from it. Cuts ~34 duplicated files.

---

## 7. The dream loop — self-evolution with a safety net

**Decision (2026-07-04):** soft diffs auto-apply on the nightly cron; hard blocks always propose. This trades a small risk (an aggressive dedup) for a big payoff (the brain compounds without daily human gardening). To stay safe, three guardrails:

1. **Audit log.** Every applied soft diff is appended to `.kg/dream-audit.jsonl` with `{ts, pass, diff, before_hash, after_hash, undo_token}`. `kg tokens` and `kg status` surface the last dream run.
2. **One-command undo.** `kg dream --undo <undo_token>` (or `--undo-last`) reverses a single diff or the entire night's batch using the recorded `before_hash`. The Markdown vault is canonical, so undo is just a checked-in revert.
3. **Git branch gate.** The cron runs on a `brain/dream/<date>/` branch. Auto-apply commits there; a fast-forward merge to main happens only if `kg validate --okf --links --frontmatter` passes post-dream. Contradictions stay on the branch as proposals for `/kgx:review`.

```
   ┌─────────────────────────────────────────────────────────────┐
   │  NIGHTLY CRON (launchd on mac, systemd on linux)             │
   │                                                              │
   │  1. git checkout brain/dream/<date>/                         │
   │  2. kg index --incremental      ← picks up today's notes     │
   │  3. kg dream --max-iterations 3 ← 7 passes                   │
   │  4. kg dream --apply-soft       ← auto-applies soft diffs    │
   │       (dedup, staleness, archive, orphan_repair,             │
   │        community, open_questions) → commit + audit row       │
   │  5. kg dream --propose-hard     ← writes hard diffs as       │
   │       staged proposals (contradiction) for review            │
   │  6. kg validate --okf --links   ← gate; if fail, abort merge │
   │  7. kg codebase index           ← if a repo is linked        │
   │  8. friction digest             ← top-3 recall-miss themes   │
   │  9. fast-forward main ← brain/dream/<date>/  (on green)      │
   │ 10. open PR for hard proposals (--auto-pr)                   │
   └─────────────────────────────────────────────────────────────┘
                              ▼
   ┌─────────────────────────────────────────────────────────────┐
   │  HUMAN REVIEW (async, only for hard blocks)                  │
   │  /kgx:review                    ← approve/reject per diff    │
   │  --ponytail-audit               ← flags over-engineered      │
   │                                    diffs (≥ultra intensity)  │
   │  kg dream --undo <token>        ← if a soft diff misfired    │
   └─────────────────────────────────────────────────────────────┘
```

### The 7 dream passes (all real)

| Pass | What it proposes | Class (this PRD) |
|---|---|---|
| `dedup` | Merge near-identical facts, archive loser, repoint edges | **soft** (auto-apply) |
| `contradiction` | Detect "X is primary" vs "Y is primary" | **hard** (proposal only) |
| `supersession` | Newer decision supersedes older, mark old `archived` | **soft** (auto-apply) |
| `staleness` | Note untouched > N days with no inbound edges → archive | **soft** (auto-apply) |
| `community` | Generate/update community summaries (GraphRAG) | **soft** (auto-apply) |
| `orphan_repair` | Link notes with zero inbound edges to a MOC | **soft** (auto-apply) |
| `open_questions` | Surface unanswered `question` notes; propose resolution | **soft** (auto-apply) |

**Verified by my tests today:** `kg dream --dry-run` on the fixture stages 18 diffs and flags 6 hard blocks in 6ms — no LLM call. The contradiction pass catches the planted "Postgres primary vs CockroachDB primary" correctly. The split above is a routing change on top of today's proposal-only engine — no algorithm rewrite.

### Ponytail audit (gate against over-engineering)

Every diff gets an intensity ladder (lite / full / ultra). `--ponytail-audit` flags anything at ultra that touches > 5 files or > 200 LOC. Today this catches ~22% of token spend on over-broad review diffs (measured).

---

## 8. Token efficiency — measured, not promised

| Approach | Tokens | Reduction |
|---|---|---|
| Naive paste-all (17 notes) | ~3,400 | — |
| **kg hybrid retrieval** | **371** | **89%** |
| At scale (200 notes) | ~40,000 | kg still ~8–10 chunks |

RTK (Response Token Kiln) compresses shell-out output 60–90% per command. Today only `kg sync` uses it; this PRD widens RTK to all `kg` shell-outs (Phase 8). Ponytail ladders cut extraction intensity — `lite` for "just save this," `full` for facts, `ultra` reserved for high-stakes decisions.

**Honest caveat (from `sprint-simulation-benchmark-results.md`):** a real two-agent 10-day sprint measured **21% actual token reduction**, not the projected 73%. The gap is because real sessions ran more `kg ask` calls with longer prompts than the simulation assumed. The 100% session-re-hydration savings (no more re-pasting yesterday's context) is real and verified.

---

## 9. Visualization — 3D WebGL graph

Today's viewer (verified in repo, shipped 2026-07-03):

- **Three.js 3D**, dual renderers (WebGL + CSS2D for labels) with bloom postprocessing
- Dark theme, sprite-based node labels, force-directed layout
- Filter sidebar (by type), auto-rotate, Fit button, auto-fitView on load
- Drag nodes (individual meshes), smaller labels (0.3), wider node spacing (repulsion 0.025, target distance 6.0)
- Single self-contained HTML file via `kg graph --format html --out`

Roadmap (this PRD): add a "play dream" timeline (scrub through the brain state before/after each dream pass), export PNG/SVG, and a `briefing://` panel showing the top-3 friction themes for the week.

---

## 10. Testing & verification

### 10.1 Today (verified 2026-07-04, `KGX_LLM=mock`, hermetic)

```
KGX_LLM=mock cargo test --package smoke --test '*' -- --test-threads=1
→ 18 passed; 0 failed
```

| Spec | What it verifies |
|---|---|
| T01 | `raw/` hash unchanged after extract+dream |
| T02 | Extract yields ≥1 fact per source with provenance |
| T03 | Zero phantom wikilinks |
| T04 | Exactly 1 orphan detected |
| T05–T08, T14, T15 | Dream stages → review applies soft, blocks hard |
| T06 | Dedup merges, archives, repoints edges |
| T10 | `rm -rf .kg && kg index --full` → identical counts |
| T11 | Fresh init + validate passes; ship→pull→validate round-trip |
| T12 | HTML graph counts match brain.sqlite |
| T13 | ≥3 communities each with summary + MOC |
| T16 | `kg tokens` matches per-command records |
| T17 | RTK compression or graceful fallback |
| T18 | `--ponytail-audit` flags over-engineered diffs |

### 10.2 What this PRD adds

- **T09** (the long-TODO'd criterion benchmark) — the 3-arm `kgx-bench` runner on a 50-query gold set, with the kill criterion as the gate.
- **T10-upgrade** — byte-hash equality across rebuilds, not just counts (catches the PageRank leak silently shifting scores).
- **T19** (new) — `kg index --incremental` after a 1-file touch touches only that note's row.
- **T20** (new) — `EXPLAIN` confirms vec0 index used (not full scan) on KNN.
- **T21** (new) — Real Leiden: a fixture with planted 3-community structure yields ≥3 communities, deterministic by seed.
- **T22** (new) — Multi-hop `ask`: "What depends on Postgres?" returns Billing Service, not the primary-datastore fact.
- **T23** (new, from auto-apply-soft decision) — `kg dream --apply-soft` then `kg dream --undo-last` restores the vault byte-for-byte (audit log + undo token round-trip).
- **T24** (new) — `kg validate --okf --links --frontmatter` blocks a dirty `brain/dream/<date>/` branch from fast-forwarding to main.

---

## 11. Roadmap — phases with acceptance gates

Build order confirmed 2026-07-04: **correctness first (P0–P2), then features.** Personal-scale (≤10K notes) throughout.

| Phase | Theme | Acceptance gate |
|---|---|---|
| **0** | Foundation: determinism, schema migrations, markdown parsing, PageRank fix | T10-upgrade byte-hash; T19 incremental |
| **1** | Real embeddings + sqlite-vec ANN | T20 vec0 EXPLAIN; semantic gold-set non-zero recall |
| **2** | Real Leiden + GraphRAG global + **multi-hop `ask` synthesis** | T21 real-community detection; deterministic by seed; T22 multi-hop ask |
| **3** | Unified tool registry (9 tools, one dispatch core); SHA-256 ingest; `project` filter | MCP protocol test covers all 9; CLI + MCP share core |
| **4** | Per-project brains + `home` scope | Cross-project isolation; union query |
| **5** | Continual learning loop (conversation ingest + refine routing) | UF-5: stated preference surfaces later; contradiction → proposal, not overwrite |
| **6** | Briefing resource + HTTP transport + Docker | UF-1: `docker compose up` + one MCP entry → answer in <5 min |
| **7** | Dream telemetry + **auto-apply-soft + audit log + undo** + friction loop + `kgx-bench` 3-arm | T23 undo round-trip; T24 validate gate; kill criterion: Arm C beats Arm B on S1–S3 |
| **8** | Polish & ship: **ZCode first-class agent**, RTK widening, skill dedup, viz timeline | All 24 T-specs green in CI; ZCode install path parity-tested |

**Sequencing logic:** 0→1→2 fixes correctness before adding features (you can't bench fake embeddings honestly). 3 is the merge point. 4–6 build the context-layer features on top of a correct core. 7 lands the autonomy story with safety nets. 8 ships, with ZCode as a first-class harness not an afterthought.

---

## 12. What to build first

Per the confirmed 2026-07-04 decisions, the implementation plan should:

1. **Start at Phase 0** (correctness). Don't touch embeddings, Leiden, or new surfaces until PageRank stops leaking, `--incremental` actually increments, and `build_full` clears derived tables. Upgrade T10 to a byte-hash gate so silent score drift is caught.
2. **Land the multi-hop `ask` fix inside Phase 2**, not later — it's a one-line retrieval tweak (expand 1-hop via `recall` before synthesis) that closes the most visible quality bug I found in real testing.
3. **Rescope the dream engine in Phase 7** to add `--apply-soft` + `--undo` + the audit log + the `kg validate` merge gate. This is the new ground the auto-apply-soft decision breaks.
4. **Add ZCode to `dev-install.sh` and CI in Phase 8**, alongside the existing 4-agent matrix. Same MCP-stdio contract as Cursor; mostly installer + skill-mirror work.

The companion implementation plan at `docs/superpowers/plans/2026-07-02-kgx-unified-rewrite.md` already covers Phases 0–8 at the phase level. The next writing-plans pass should produce a **bite-sized TDD plan for Phase 0 specifically** (the PageRank fix, the incremental wiring, the byte-hash T10 upgrade), since that's the unlock for everything else.

---

## Sources

- [From Harness Lock-In to Portable Context Layer — Paul Iusztin, Decoding AI](https://www.decodingai.com/p/the-context-layer)
- [OKF SPEC.md — Google Cloud (GoogleCloudPlatform/knowledge-catalog)](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md)
- [How the Open Knowledge Format can improve data sharing — Google Cloud Blog](https://cloud.google.com/blog/products/data-analytics/how-the-open-knowledge-format-can-improve-data-sharing)
- [DeusData/codebase-memory-mcp — codebase knowledge graph MCP](https://github.com/DeusData/codebase-memory-mcp)
- [Karpathy LLM Wiki gist](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f) · [GitHub topic](https://github.com/topics/karpathy-llm-wiki)
- [geronimo-iia/llm-wiki — headless wiki MCP, Rust](https://github.com/geronimo-iia/llm-wiki)
- [Knowledge Graph Based Repository-Level Code Generation — arXiv 2505.14394](https://arxiv.org/html/2505.14394v1)
- Existing internal artifacts: `prd.md`, `context-layer.md`, `STATUS.md`, `docs/eval-results.md`, `docs/sprint-simulation.md`, `docs/sprint-simulation-benchmark-results.md`, `docs/superpowers/plans/2026-07-02-kgx-unified-rewrite.md`
