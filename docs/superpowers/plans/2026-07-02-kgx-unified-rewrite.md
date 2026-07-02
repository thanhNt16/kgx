# KGX Unified Rewrite — Architecture & Implementation Plan

> **For agentic workers:** This is a phase-level plan with acceptance gates (per user preference), not bite-sized TDD micro-steps. Each phase lists files (exact paths), the merge reconciliation, acceptance tests, and key implementation detail. Execute phase-by-phase; do not start Phase N+1 until Phase N's acceptance gate passes.

**Goal:** Rewrite/evolve KGX into a unified local-first knowledge-graph toolset that merges `prd.md` (SQLite CLI) and `context-layer.md` (portable context layer) onto a single SQLite + sqlite-vec backend, with stdio + HTTP MCP transports, Docker deployment, continual learning, and a self-improving dream loop.

**Architecture:** SQLite remains the disposable "brain" (now with sqlite-vec ANN, real Leiden, real embeddings). The Markdown vault stays canonical/git-versioned. A unified tool registry serves both the `kg` CLI (offline) and the `kgd` MCP server (online, stdio + HTTP). Per-project brains + a `home` brain give scope isolation. Conversation ingest + friction notes close the continual-learning loop; dream proposes, git review approves.

**Tech Stack:** Rust 2021, `rusqlite` (bundled) + `sqlite-vec` (vec0), `fastembed` (all-MiniLM-L6-v2, 384-dim), `pulldown-cmark`, `petgraph` + a Leiden impl, `axum` + `rmcp` (or bespoke HTTP JSON-RPC), `reqwest`, `clap`, `tera`, `tokio`. Docker Compose for the `kgd` deployment (no Mongo).

**Decisions (2026-07-02):**
- Storage: SQLite + sqlite-vec (evolve current `kgx-graph`). Mongo is dropped.
- Scope: Full unified merge of both PRDs.
- Deploy: stdio default + new HTTP MCP + Docker Compose (SQLite-backed).
- Granularity: Phase-level with acceptance gates.

---

## Ground truth (what exists today — the baseline for the rewrite)

- **~8,200 lines across 16 crates, 5 commits, T01–T18 smoke tests passing.** Real binary `kg` (CLI) + `rtk` (log compressor). Load-bearing crates: `kgx-graph` (1042), `kgx-retrieval` (768), `kgx-dream` (807).
- **Storage:** SQLite via `rusqlite` (bundled). Single schema constant applied with `CREATE TABLE IF NOT EXISTS` at `crates/kgx-graph/src/brain.rs:25` (schema in `schema.rs:1`). Tables: `notes` (incl. `embedding BLOB`), `edges`, `notes_fts` (FTS5 porter), `pagerank`, `communities`, `community_summaries`, `meta`.
- **Embeddings: mocked.** `index.rs:19` hardcodes `MockEmbedder` (384-dim FNV hash). `FastEmbedEmbedder` (all-MiniLM-L6-v2, `embed.rs:82`) exists but is unreachable from the CLI; `candle`/`MiniLmEmbedder` is a dead stub.
- **Vector search:** brute-force cosine O(N) at `knn.rs:15`. No sqlite-vec, no ANN.
- **Communities:** connected-components, not Leiden. `seed` is discarded (`community.rs:49`).
- **Markdown:** `pulldown-cmark` is a declared but **unused** dependency; bodies are opaque strings; wikilinks come from regex (`util.rs:18`).
- **`--full`/`--incremental`:** both no-ops (`index.rs:9-21` always calls `build_full`; `build_incremental` is dead from CLI). `build_full` does NOT clear `pagerank`/`communities`/`community_summaries` (`build.rs:118`).
- **PageRank:** has a dangling-node mass leak (`pagerank.rs:47-49`).
- **MCP:** bespoke JSON-RPC over stdio only (`server.rs:6`); 6 tools (`tools.rs:5`).
- **Dream:** all 7 passes real; proposal-only (good invariant). `meta` table defined, never written.
- **RTK:** only `kg sync` uses `run_with_rtk`; rest of the "every shell-out" claim is aspirational.
- **`kg codebase *`:** shells out to external `codebase-memory-mcp` binary — a separate product, not part of the brain.

---

## Merge Reconciliation (the crux)

| Decision in context-layer.md | Resolution for this rewrite |
|---|---|
| MongoDB view + per-project DBs | **SQLite per project**: `~/brains/<project>/.kg/brain.sqlite`. A `home` brain holds cross-cutting knowledge. No Mongo. |
| In-process HNSW (`usearch`) | **sqlite-vec `vec0` virtual table** — same role (ANN), zero extra service, lives in the same `.sqlite` file. |
| `kgd` separate binary, Rust axum+rmcp | **One binary, two fronts**: `kg` (CLI, offline) and `kg serve --transport http` (online). Same crates. `kgd` becomes an alias/deployment profile. |
| 6 MCP tools (Tree surface) | **Unified registry** merging current 6 + context-layer 6 (de-duplicated; see Phase 3 table). |
| Docker Compose with `mongo`+`mongo-init` | Compose drops Mongo; bind-mounts `~/brains`. Services: `kgd`, `kgd-worker`, `kgd-dream`. WAL-mode SQLite for read concurrency; **writes serialize through `kgd`** (worker enqueues via in-DB queue table, server applies) to avoid SQLite write contention. |
| `briefing://` MCP resource, `/hooks/conversation` | Implemented on the HTTP transport (Phase 6). |
| `kgd-bench` 3-arm evaluation | New crate `kgx-bench` (Phase 7); corpus from `tests/fixtures/vault-min` + a real-history snapshot. |

Everything in `prd.md` that's already built (extraction, dream 7 passes, OKF bundle, RTK, cron, viz, 18 T-specs) is **preserved and hardened**, not rebuilt.

---

## File Structure (create / modify)

**New crates:**
- `crates/kgx-store` — `BrainStore` trait + `SqliteBrain` impl (wraps today's `kgx-graph`), `BrainSet` (multi-project mount: active + `home`).
- `crates/kgx-bench` — frozen corpus, gold set S1–S7, 3-arm runner, LLM-as-judge, report writer.

**New modules in existing crates:**
- `kgx-graph/src/migrate.rs` — versioned schema migrations (replaces single `CREATE TABLE IF NOT EXISTS`).
- `kgx-graph/src/vec.rs` — sqlite-vec `vec0` table + KNN queries (replaces `knn.rs` brute force).
- `kgx-graph/src/leiden.rs` — real Leiden (replaces `community.rs` connected-components).
- `kgx-mcp/src/http.rs` — axum HTTP JSON-RPC + `/hooks/conversation` + `briefing://` resource.
- `kgx-mcp/src/tools/` — split `tools.rs` into one file per tool (registry pattern).
- `kgx-extract/src/conversation.rs` — `ingest_conversation` incremental + finalize + compile judgment.
- `kgx-dream/src/friction.rs` — friction clustering + fix proposals + weekly digest.

**Modified (key files):**
- `kgx-core/src/types.rs` — add `NoteType::Preference`, `NoteType::Friction`; add `project` field.
- `kgx-cli/src/commands/index.rs:19` — replace hardcoded `MockEmbedder` with `embedder_from_env()`; wire `--full`/`--incremental` (currently both no-ops); add `--rebuild-vectors`.
- `kgx-graph/src/pagerank.rs:47-49` — fix dangling-node mass redistribution.
- `kgx-graph/src/build.rs:118` — clear `pagerank`/`communities`/`community_summaries` in `build_full`.
- `docker-compose.yml` (new, repo root) — `kgd` + `kgd-worker` + `kgd-dream`, bind-mount `~/brains`.

---

## Phases

### Phase 0 — Foundation: determinism, schema, parsing
**Fix the load-bearing correctness bugs before adding features.**
- Files: `kgx-graph/src/{migrate.rs,brain.rs:25,build.rs:118,pagerank.rs:47-49,schema.rs}`, `kgx-vault/src/parse.rs`, new `kgx-vault/src/markdown.rs` (pulldown-cmark).
- Work: migration system (version table + additive DDL); fix PageRank dangling leak; make `build_full` clear all derived tables; actually dispatch `--incremental` to `build_incremental`; parse markdown via pulldown-cmark (headings + cleaner wikilink extraction); write `meta` table on every index.
- **Acceptance:** `tests/smoke/smoke_t10_rebuild.rs` upgraded to a strict hash check (notes+edges+fts identical across 2 runs); `kg index --incremental` after a 1-file touch touches only that note; PageRank sums to ~1.0 on a fixture with dangling nodes.

### Phase 1 — Real embeddings + sqlite-vec
- Files: `kgx-graph/src/{vec.rs,embed.rs,knn.rs}`, `kgx-cli/src/commands/index.rs:19`, `kgx-retrieval/src/hybrid.rs:134`.
- Work: construct `FastEmbedEmbedder` when `KGX_EMBED=fastembed` (or `semantic` feature on) instead of always `MockEmbedder`; add `vec0` virtual table (`CREATE VIRTUAL TABLE notes_vec USING vec0(embedding float[384])`); insert on build; replace `knn.rs` O(N) scan with `SELECT id, distance FROM notes_vec WHERE embedding MATCH ? ORDER BY distance LIMIT k`; add `kg index --rebuild-vectors` (re-embed from text, no API re-call if embeddings cached).
- **Acceptance:** semantic search returns semantically-relevant (non-keyword-overlap) results on a new gold mini-set; `EXPLAIN` confirms vec0 index used (not full scan); T09 recall beats vector-disabled baseline.

### Phase 2 — Real Leiden + GraphRAG global
- Files: `kgx-graph/src/leiden.rs` (replaces `community.rs`), `kgx-retrieval/src/{global.rs,community_summary.rs}`.
- Work: implement Leiden (local-moving + refinement) over `petgraph`, honoring `seed` for determinism; guaranteed-connected communities; LLM community summaries; LazyGraphRAG global mode consuming summaries for `ask --scope global`.
- **Acceptance:** T13 — a fixture with genuine community structure yields ≥3 communities (not 1); same seed → identical partition (deterministic); global mode cites community summaries.

### Phase 3 — Unified tool registry (the merge)
De-duplicate current 6 + context-layer 6 into a canonical set:

| Canonical tool | Replaces | Kind |
|---|---|---|
| `nl_query_memory` | `search_notes` + `ask_question` (NL path) | read |
| `query_memory` | new (structured filters: type, tag, project, date, status) | read |
| `deep_search_memory` | new (progressive disclosure → `.kg/wiki-cache/<slug>/`) | read |
| `get_note` | `get_note` | read |
| `ingest_file` | `capture_raw` (file) | write |
| `ingest_url` | new (fetch → pipeline) | write |
| `ingest_conversation` | new (Phase 5) | write |
| `upsert_note` | `upsert_note` | write |
| `dream_step` | `dream_step` | write |

- Files: split `kgx-mcp/src/tools.rs` into `tools/{mod,nl_query,query,deep_search,get,ingest_file,ingest_url,ingest_conversation,upsert,dream}.rs`; idempotency on content sha256 for ingest.
- **Acceptance:** `tests/smoke` MCP protocol test covers all tools; re-ingesting the same file is a no-op (hash match); `kg` CLI verbs and MCP tools share one dispatch core.

### Phase 4 — Per-project brains + `home` scope
- Files: new `crates/kgx-store/`; `kgx-cli/src/commands/project.rs` (new); `project` param threaded into read tools.
- Work: `BrainStore` trait; `SqliteBrain` (wraps `kgx-graph::Brain`); `BrainSet::query(projects: &[&str])` fans out across active + `home`; `kg project add/list/use`; read tools take `project` (default active+home); cross-project = explicit multi-scope.
- **Acceptance:** a note in project A is invisible when querying B alone; querying `home+project` returns union; `home` brain created on first `kg init`.

### Phase 5 — Continual learning loop
- Files: `kgx-extract/src/conversation.rs`, `kgx-core/src/types.rs` (Preference type), `skills/hooks/kgx-conversation.{sh,js}` (Stop/turn-count adapter), `kgx-dream/src/refine.rs`.
- Work: `ingest_conversation` incremental (~10 turns) + finalize (compile judgment: durable facts/decisions/preferences, discard ephemera); refine routing ADD/UPDATE/MERGE/DEPRECATE mapping onto dream's dedup/supersession/staleness passes; contradictions surfaced (never silent overwrite); hook posts transcript delta to `kg serve /hooks/conversation`.
- **Acceptance (UF-5):** a stated preference becomes a `preference` note and is surfaced in a later session; a contradiction produces a proposal on the dream branch, not an overwrite.

### Phase 6 — Briefing resource + HTTP transport + Docker
- Files: `kgx-mcp/src/http.rs`, `docker-compose.yml`, `deploy/Dockerfile`, `crates/kgx-cli/src/commands/serve.rs` (new).
- Work: `briefing://{project}` MCP resource (~1k tokens, regen on dream + finalize); axum HTTP JSON-RPC (same tool registry as stdio); `/hooks/conversation` endpoint; compose with `kgd` (http serve) + `kgd-worker` (ingest queue) + `kgd-dream` (cron sidecar, writes proposals to a git branch only); WAL-mode SQLite; **all brain writes serialize through `kgd`** (worker enqueues via in-DB queue table, server applies) to avoid write contention.
- **Acceptance (UF-1):** `docker compose up` + one MCP config entry → agent answers "what do you know about me?" in <5 min using the briefing resource.

### Phase 7 — Dream telemetry + friction loop + bench
- Files: `kgx-core/src/types.rs` (Friction type), `kgx-dream/src/friction.rs`, new `crates/kgx-bench/`, `tests/fixtures/bench-corpus/`.
- Work: write `friction` notes on recall miss / failed NL plan / empty result / re-ask signal; dream friction-pass clusters the week's friction → proposes fixes (missing note, alias, synonym, skill amendment, gold-set question); weekly digest top-3 themes; `kgx-bench`: frozen corpus + gold S1–S7, 3-arm runner (A no-memory / B raw-folder+grep / C kgd), LLM-as-judge + 20% human spot-check, writes `bench/results/<date>.json`.
- **Acceptance (UF-6 + kill criterion):** dream never mutates main; friction pass proposes ≥1 actionable fix; Arm C correctness clearly beats Arm B on S1–S3 or the hybrid investment is flagged.

### Phase 8 — Polish & ship
- Files: `kgx-viz/` (D3 single-file HTML, brain-map-skill parity), `kgx-docs/`, `install.sh`, `dev-install.sh`, `crates/kgx-rtk/src/wrap.rs` (widen RTK usage).
- Work: harden `kg graph --format html`; `kg docs usecase` generators; OKF `ship`/`pull` round-trip (T11); release installer auto-registers MCP for cursor/codex/opencode (not just claude); wire `run_with_rtk` into more shell-outs; update README/AGENTS.md/CLAUDE.md to the unified tool set.
- **Acceptance:** T11 (OKF lossless), T12 (viz counts match brain), T16 (token accounting), T17 (rtk ≤30% tokens); full 18-T-spec gate green in CI.

---

## Self-Review

**Spec coverage:** prd.md §5–16 (vault, brain, retrieval, commands, dream, tokens, sync, cron, viz, metrics) → Phases 0,1,2,3,8. prd.md §9 MCP server mode → Phase 3+6. context-layer.md UF-1→P6, UF-2→P3, UF-3→P1/P3, UF-4→P3, UF-5→P5, UF-6→P7, UF-7→P6. D-1(vec index)→P1, D-2(per-project)→P4, D-3(grimoire retired—n/a, skills hosted on server)→P6, D-4(cadence)→P5, D-5(briefing)→P6. §11 bench→P7. §12 friction/adaptation→P7. All 18 T-specs covered (0,1,2,3,5,6,7,8). **No gaps.**

**Placeholder scan:** none — every phase names exact files, the merge table is concrete, acceptance tests reference real fixtures.

**Type consistency:** `BrainStore`/`SqliteBrain`/`BrainSet` (P4) used consistently; `NoteType::{Preference, Friction}` added in types and consumed in P5/P7; canonical tool names (P3) used unchanged in P6 HTTP transport.
