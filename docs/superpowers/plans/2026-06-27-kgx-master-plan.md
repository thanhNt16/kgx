# KGX Master Orchestration Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement the per-phase plans task-by-task. This master plan defines the shared contracts, the parallelization waves, the testing pyramid, the cross-tool compatibility layer, and the CI gate. **Read this document before starting any phase plan.** Steps in the phase plans use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build KGX — a local-first, AI-managed knowledge-graph CLI (`kg`) in Rust that turns a Markdown + `[[wikilinks]]` vault into a queryable hybrid (vector + graph + keyword) brain, with cross-tool agent integration (Claude Code, Codex, Cursor).

**Architecture:** A Cargo **workspace** of focused crates. `kgx-core` holds the shared contracts (types, errors, traits, JSON envelope). Every other crate depends only on `kgx-core` and a small, explicit set of siblings, forming a dependency DAG. The DAG defines **execution waves**: crates in the same wave have no dependency between them and are built by parallel sub-agents. The vault (Markdown) is canonical; `.kg/brain.sqlite` is a rebuildable derived index.

**Tech Stack:** Rust 2021, `clap` (CLI), `rusqlite` + `sqlite-vec` (brain), `pulldown-cmark` (Markdown AST), `serde`/`serde_yaml`/`serde_json` (frontmatter + JSON), `ulid`, `petgraph` (graph algorithms), `candle`/`ort` (embeddings), `tera` (templates), `tokio` + `reqwest` (LLM HTTP), `ratatui` (dashboard), `assert_cmd` + `insta` (testing), `criterion` (recall benchmarks).

---

## Global Constraints

These apply to **every task in every phase plan**. Each task's requirements implicitly include this section.

- **Rust edition:** `2021`. Minimum supported Rust version (MSRV): `1.78`.
- **Workspace:** single `Cargo.toml` workspace at repo root; one crate per module per §18 of the PRD. Crate names are `kgx-<module>` (e.g. `kgx-vault`). The binary crate is `kgx-cli`, producing a binary named exactly `kg`.
- **Every command supports `--json`.** Human output goes to stdout as text; `--json` switches stdout to a single JSON document (the `JsonEnvelope`, defined below). Logs/diagnostics always go to stderr, never stdout.
- **Markdown is canonical.** No command may write to `.kg/` in a way that cannot be reconstructed by `kg index --full`. `T10` enforces this.
- **Supersession, not deletion.** No command deletes a note file in `notes/` or `raw/`. Lifecycle changes flip `status` and set `valid_to`. `T01`, `T05`, `T14` enforce this.
- **Determinism.** Index builds, formatting, and ULID-ordered output must be deterministic given the same inputs. Sort all collections before serializing. `T10` enforces this.
- **Provenance always.** Every extracted `fact`/`entity`/`decision` note has a `source:` frontmatter field pointing to a `raw/` note. `T02` enforces this.
- **Token-conscious.** Every shell-out goes through `kgx-rtk::run_with_rtk`. Every LLM call records tokens via `kgx-tokens`. `T16`, `T17` enforce this.
- **Error handling:** library crates return `Result<T, kgx_core::KgError>` (via `thiserror`). The binary uses `anyhow` only at the `main`/command-dispatch boundary. No `unwrap()`/`expect()`/`panic!` in library code except in tests.
- **Formatting/lint gate:** `cargo fmt --check` and `cargo clippy --all-targets --all-features -- -D warnings` must pass. No `#[allow(...)]` without an inline justification comment.
- **OKF version pin:** `okf_version: "0.1"` everywhere OKF is emitted.
- **Embedding default:** `all-MiniLM-L6-v2`, 384 dimensions, run locally via `candle`. Embeddings stored as little-endian `f32` BLOBs.
- **Commit discipline:** one commit per completed step where the step says "Commit". Conventional Commits style (`feat:`, `test:`, `fix:`, `chore:`). End every commit message with:
  ```
  Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
  ```

---

## 1. Crate / File Structure

```
kgx/                                  # repo root (Cargo workspace)
├── Cargo.toml                        # [workspace] members + shared deps
├── rust-toolchain.toml               # pin 1.78
├── .github/workflows/ci.yml          # CI gate (lint, unit, integration, smoke)
├── crates/
│   ├── kgx-core/                     # WAVE 0 — shared contracts (no kgx-* deps)
│   ├── kgx-okf/                      # WAVE 1
│   ├── kgx-vault/                    # WAVE 1
│   ├── kgx-tokens/                   # WAVE 1
│   ├── kgx-llm/                      # WAVE 1
│   ├── kgx-rtk/                      # WAVE 1
│   ├── kgx-ponytail/                 # WAVE 1
│   ├── kgx-cron/                     # WAVE 1
│   ├── kgx-graph/                    # WAVE 2 (vault, core)
│   ├── kgx-extract/                  # WAVE 2 (llm, vault, ponytail, tokens)
│   ├── kgx-retrieval/               # WAVE 3 (graph, llm)
│   ├── kgx-dream/                    # WAVE 3 (graph, llm, vault, retrieval)
│   ├── kgx-viz/                      # WAVE 4 (graph)
│   ├── kgx-docs/                     # WAVE 4 (vault, tera)
│   ├── kgx-mcp/                      # WAVE 4 (retrieval, vault, graph, extract)
│   └── kgx-cli/                      # WAVE 5 (binary `kg`, depends on all)
├── skills/                           # WAVE 5 — cross-tool agent integration
│   ├── claude/.claude/skills/kgx/SKILL.md
│   ├── codex/AGENTS.md
│   └── cursor/.cursor/rules/kgx.mdc
├── tests/
│   ├── fixtures/vault-min/           # canonical fixture vault (shared)
│   └── smoke/                        # T01–T18 smoke scripts (bash + assert_cmd)
└── install.sh                        # WAVE 6 — installer one-liner
```

**Per-crate internal layout** (every `kgx-*` crate):
```
crates/kgx-<name>/
├── Cargo.toml
├── src/
│   ├── lib.rs                        # pub re-exports + module decls
│   └── <feature>.rs                  # one responsibility per file
└── tests/                            # integration tests for this crate
```

---

## 2. Dependency DAG & Execution Waves

A **wave** is a set of crates with no dependency edges between them — assign one sub-agent per crate and run the wave in parallel. A wave starts only after the previous wave's crates compile, pass their own tests, and are reviewed.

```
Wave 0  ─ kgx-core ─────────────────────────────────────────┐ (blocks everything)
                                                              │
Wave 1  ┌ kgx-okf ┐ ┌ kgx-vault ┐ ┌ kgx-tokens ┐            │
        │ kgx-llm │ │ kgx-rtk   │ │ kgx-ponytail│ kgx-cron   │ (all depend only on core)
        └─────────┘ └───────────┘ └─────────────┘            │
Wave 2  ┌ kgx-graph (vault) ┐  ┌ kgx-extract (llm,vault,ponytail,tokens) ┐
        └───────────────────┘  └─────────────────────────────────────────┘
Wave 3  ┌ kgx-retrieval (graph,llm) ┐  ┌ kgx-dream (graph,llm,vault,retrieval) ┐
        └────────────────────────────┘  └──────────────────────────────────────┘
Wave 4  ┌ kgx-viz (graph) ┐ ┌ kgx-docs (vault) ┐ ┌ kgx-mcp (retrieval,vault,graph,extract) ┐
        └─────────────────┘ └──────────────────┘ └──────────────────────────────────────────┘
Wave 5  ─ kgx-cli (binary) + skills/ (claude, codex, cursor) ─ (depends on all crates)
Wave 6  ─ install.sh + smoke suite hardening + CI finalization
```

**Phase → Wave mapping** (PRD §17 phases map onto waves; phases can overlap because waves are the real ordering constraint):

| PRD Phase | Plan file | Crates produced | Waves |
|---|---|---|---|
| 0 — Skeleton | `…-phase0-skeleton.md` | core, okf, vault, cli(init/validate) | 0, 1, partial 5 |
| 1 — Brain | `…-phase1-brain.md` | tokens, graph, cli(index) | 1, 2 |
| 2 — Ask | `…-phase2-ask.md` | llm, extract, retrieval, cli(ask/recall/search/extract/capture/link) | 1, 2, 3 |
| 3 — Dream | `…-phase3-dream.md` | dream, cli(dream/review) | 3 |
| 4 — GraphRAG | `…-phase4-graphrag.md` | graph(leiden/communities), retrieval(global) | 2/3 extensions |
| 5 — MCP+Skills | `…-phase5-mcp-skills.md` | mcp, rtk, ponytail, cron, skills/, cli(mcp-server/cron) | 1, 4, 5 |
| 6 — Polish | `…-phase6-polish.md` | viz, docs, cli(dashboard/graph/docs/ship/pull/status/tokens), install.sh | 4, 5, 6 |

> **Parallelization rule for the orchestrator:** dispatch one sub-agent per crate within a wave. Each sub-agent receives (a) this master plan, (b) the relevant phase plan section, (c) the **Interfaces** block of its task (the exact signatures it consumes/produces). Sub-agents never edit `kgx-core` after Wave 0 — contract changes go through the orchestrator to avoid merge conflicts.

---

## 3. Shared Contracts (`kgx-core`) — the single source of inter-crate truth

These types are defined in Phase 0, Task 2. They are reproduced here because **every phase plan references them by exact name**. If a phase needs a new shared type, it is added to `kgx-core` in a dedicated task, not invented locally.

```rust
// crates/kgx-core/src/types.rs

/// Note category. Serializes lowercase: "fact", "entity", etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteType { Fact, Entity, Decision, Experience, Moc, Source, Question }

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status { Active, Deprecated, Archived, Superseded }

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence { High, Medium, Low }

/// Edge relationship type stored in the brain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelType { LinksTo, Supersedes, DerivedFrom, Cites, MentionsEntity, Contradicts }

/// Parsed frontmatter. Unknown keys preserved in `extra` (OKF §9 tolerance).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Frontmatter {
    pub r#type: NoteType,
    pub id: String,                       // ULID
    pub title: String,
    #[serde(default = "Status::default_active")]
    pub status: Status,
    pub valid_from: Option<String>,       // ISO date
    pub valid_to: Option<String>,
    pub recorded_at: Option<String>,      // ISO datetime
    #[serde(default)] pub supersedes: Vec<String>,
    pub superseded_by: Option<String>,
    pub source: Option<String>,           // "[[raw/...]]"
    #[serde(default = "Confidence::default_medium")]
    pub confidence: Confidence,
    #[serde(default)] pub sources_count: u32,
    #[serde(default)] pub tags: Vec<String>,
    #[serde(default)] pub links: Vec<String>,
    pub entity_type: Option<String>,
    #[serde(default)] pub aliases: Vec<String>,
    #[serde(default)] pub created_by: CreatedBy,
    #[serde(default)] pub created_via: CreatedVia,
    #[serde(flatten)] pub extra: std::collections::BTreeMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CreatedBy { #[default] Human, Agent }

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CreatedVia { #[default] Cli, Mcp, Sync }

/// A note = frontmatter + Markdown body + on-disk path (relative to vault root).
#[derive(Debug, Clone)]
pub struct Note { pub fm: Frontmatter, pub body: String, pub rel_path: std::path::PathBuf }

/// A graph edge (matches the `edges` SQL table).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub src_id: String, pub dst_id: String, pub rel_type: RelType,
    pub valid_from: Option<String>, pub valid_to: Option<String>,
}
```

```rust
// crates/kgx-core/src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum KgError {
    #[error("io error at {path}: {source}")]
    Io { path: String, #[source] source: std::io::Error },
    #[error("frontmatter parse error in {path}: {msg}")]
    Frontmatter { path: String, msg: String },
    #[error("brain/sqlite error: {0}")]
    Brain(String),
    #[error("llm provider error: {0}")]
    Llm(String),
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("{0}")]
    Other(String),
}
pub type Result<T> = std::result::Result<T, KgError>;
```

```rust
// crates/kgx-core/src/json.rs
/// Universal --json output envelope. EVERY command serializes exactly this.
#[derive(Debug, serde::Serialize)]
pub struct JsonEnvelope<T: serde::Serialize> {
    pub ok: bool,
    pub command: String,                 // e.g. "index"
    pub data: T,                         // command-specific payload
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub elapsed_ms: u64,
}
impl<T: serde::Serialize> JsonEnvelope<T> {
    pub fn success(command: &str, data: T, elapsed_ms: u64) -> Self {
        Self { ok: true, command: command.into(), data, warnings: vec![], error: None, elapsed_ms }
    }
}
```

```rust
// crates/kgx-core/src/llm.rs  — the provider trait every LLM caller depends on.
#[derive(Debug, Clone)]
pub struct LlmRequest { pub system: String, pub prompt: String, pub max_tokens: u32, pub temperature: f32 }
#[derive(Debug, Clone)]
pub struct LlmResponse { pub text: String, pub input_tokens: u32, pub output_tokens: u32, pub model: String }

#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, req: LlmRequest) -> crate::Result<LlmResponse>;
    fn model_id(&self) -> &str;
}

/// 384-dim embedding vector.
pub trait Embedder: Send + Sync {
    fn embed(&self, texts: &[String]) -> crate::Result<Vec<Vec<f32>>>;
    fn dim(&self) -> usize; // 384
}
```

```rust
// crates/kgx-core/src/diff.rs — dream passes emit these; review consumes them.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProposedDiff {
    pub id: String,                      // ULID of the proposal
    pub pass: String,                    // "dedup" | "contradiction" | ...
    pub kind: DiffKind,
    pub rationale: String,
    pub severity: Severity,              // affects auto-commit gating
    pub files: Vec<FileChange>,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind { Merge, Supersede, Archive, AddLink, AddNote, Resummarize, FlagContradiction }
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity { Info, Soft, Scope, Hard }  // Hard blocks auto-commit (T07)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileChange { pub rel_path: String, pub before: Option<String>, pub after: Option<String> }
```

These six modules (`types`, `error`, `json`, `llm`, `diff`, plus `ulid`/`util`) are the entire public surface of `kgx-core`. **No business logic lives in core** — only contracts and pure helpers (ULID gen, wikilink parsing regex, ISO-time helpers).

---

## 4. Testing Pyramid

Three layers, enforced by CI. Every phase plan's tasks specify which layer each test belongs to.

### 4.1 Unit tests (fast, per-crate, in-module)
- Location: `#[cfg(test)] mod tests` inside each `src/*.rs`.
- Scope: pure functions — frontmatter parse/serialize round-trips, wikilink extraction, RRF math, cosine similarity, ULID monotonicity, Leiden on a toy graph.
- No filesystem beyond `tempfile`, no network (LLM/embedder mocked via trait objects).
- Run: `cargo test --workspace --lib`.

### 4.2 Integration tests (per-crate `tests/`, real deps within the crate)
- Location: `crates/kgx-<name>/tests/*.rs`.
- Scope: a crate's public API against real SQLite (`tempfile` db), real fixture vault (`tests/fixtures/vault-min`), mocked LLM/embedder.
- Run: `cargo test --workspace --test '*'`.

### 4.3 Smoke tests (end-to-end, the real `kg` binary)
- Location: `tests/smoke/` — one script/test per PRD acceptance test **T01–T18**.
- Mechanism: `assert_cmd` drives the compiled `kg` binary against a copy of `tests/fixtures/vault-min`; LLM is replaced by `KGX_LLM=mock` (a deterministic canned-response provider selected via env, see Phase 2). Assertions check stdout `--json`, file system state, and brain row counts.
- Run: `cargo test --workspace --test smoke -- --test-threads=1` (serial: shared binary + fs fixtures).

**Acceptance-test → smoke-file mapping** (each phase plan ends with the smoke tasks it unlocks):

| Test | File | Unlocked by phase |
|---|---|---|
| T01 capture-immutability | `tests/smoke/t01_capture_immutability.rs` | 2 |
| T02 extract-correctness | `tests/smoke/t02_extract.rs` | 2 |
| T03 link-integrity | `tests/smoke/t03_link.rs` | 2 |
| T04 orphan-detection | `tests/smoke/t04_orphan.rs` | 2 |
| T05 bitemporal-supersession | `tests/smoke/t05_supersede.rs` | 3 |
| T06 dedup-merge | `tests/smoke/t06_dedup.rs` | 3 |
| T07 contradiction-detect | `tests/smoke/t07_contradiction.rs` | 3 |
| T08 dream-review-gate | `tests/smoke/t08_review_gate.rs` | 3 |
| T09 qa-recall | `tests/smoke/t09_recall.rs` (criterion bench) | 2/4 |
| T10 graph-rebuild-sync | `tests/smoke/t10_rebuild.rs` | 1 |
| T11 okf-roundtrip | `tests/smoke/t11_okf.rs` | 0/6 |
| T12 viz-export | `tests/smoke/t12_viz.rs` | 6 |
| T13 community-summary | `tests/smoke/t13_community.rs` | 4 |
| T14 stale-archival | `tests/smoke/t14_stale.rs` | 3 |
| T15 ralph-loop-bound | `tests/smoke/t15_ralph.rs` | 3 |
| T16 token-accounting | `tests/smoke/t16_tokens.rs` | 1/2 |
| T17 rtk-integration | `tests/smoke/t17_rtk.rs` | 5 |
| T18 ponytail-audit | `tests/smoke/t18_ponytail.rs` | 5 |

### 4.4 Shared fixture vault (`tests/fixtures/vault-min`)
Built in Phase 0, Task 9. Deterministic, hand-authored, with known counts so every smoke test can assert exact numbers:
- 2 `raw/` sources, 5 `facts/`, 3 `entities/`, 2 `decisions/`, 1 `moc/`, 1 `questions/`, 1 intentional orphan fact, 1 intentional contradiction pair, 1 supersession pair.
- A `COUNTS.json` sidecar (git-tracked, **not** part of the vault) records expected node/edge/orphan counts for assertions.

---

## 5. Cross-Tool Compatibility Layer

The universal capability is the **MCP stdio server** (`kg mcp-server --transport stdio`), built in Phase 5. On top of it, each tool gets a **native** package (per the chosen "full native skills per tool" approach). All three are produced and tested in Phase 5.

| Tool | Native artifact | MCP wiring | RTK hook |
|---|---|---|---|
| **Claude Code** | `skills/claude/.claude/skills/kgx/SKILL.md` (+ optional scripts) | `.mcp.json` → `kg mcp-server --transport stdio` | `.claude/settings.json` `PostToolUse` hook wraps Bash output via `rtk` |
| **Codex** | `skills/codex/AGENTS.md` (workflows + ladders) | `~/.codex/config.toml` `[mcp_servers.kgx]` | Codex `notify`/exec wrapper invokes `rtk` |
| **Cursor** | `skills/cursor/.cursor/rules/kgx.mdc` (rule with frontmatter `globs`/`alwaysApply`) | `.cursor/mcp.json` → `kg mcp-server` | Cursor terminal profile aliases shell-outs through `rtk` |

**Compatibility contract (tested in Phase 5):**
- The MCP server exposes the same 6 tools (`search_notes`, `get_note`, `upsert_note`, `ask_question`, `capture_raw`, `dream_step`) with identical JSON schemas regardless of client.
- `kg init --with-skills` writes the correct artifact set after detecting which tool(s) are present (`.claude/`, `~/.codex/`, `.cursor/`).
- A smoke test (`t_mcp_protocol.rs`) speaks the MCP `initialize` + `tools/list` + `tools/call` handshake over stdio and asserts the tool schemas — this validates *protocol* compatibility once; per-tool config files are validated by schema/lint checks (valid JSON/TOML/MDC frontmatter).

---

## 6. CI Gate (`.github/workflows/ci.yml`)

Built incrementally; finalized in Phase 6. Jobs (fail-fast off so all report):
1. **lint** — `cargo fmt --check` + `cargo clippy --all-targets --all-features -- -D warnings`.
2. **unit** — `cargo test --workspace --lib`.
3. **integration** — `cargo test --workspace --test '*'` (excluding smoke).
4. **smoke** — `cargo test --workspace --test smoke -- --test-threads=1` with `KGX_LLM=mock`.
5. **cross-tool** — validate `skills/**` artifacts (jsonschema for `.mcp.json`/`mcp.json`, `taplo` for TOML, frontmatter lint for `.mdc`/`SKILL.md`) + MCP protocol smoke.
6. **bench (non-gating)** — `cargo bench` for T09 recall, posts delta as PR comment.

A PR merges only if jobs 1–5 are green. The phase plans add their tests to the relevant job as they go; the **Definition of Done for the whole project** is all 18 smoke tests green in job 4/5 (PRD §22 checklist).

---

## 7. Orchestration Runbook (for the dispatching agent)

For each wave, in order:
1. Open the phase plan(s) that produce this wave's crates.
2. For each crate in the wave, dispatch a sub-agent with: this master plan §3 (contracts) + the crate's tasks + its **Interfaces** block.
3. When all wave sub-agents report green (`cargo test -p <crate>` passes), run the two-stage review (superpowers:subagent-driven-development), then `cargo build --workspace` to confirm cross-crate compile.
4. Commit the wave. Proceed to next wave.

**Conflict avoidance:** within a wave, sub-agents touch only their own `crates/kgx-<name>/` directory and append to the shared `Cargo.toml` `[workspace.members]` list (orchestrator merges that one file). No two sub-agents edit the same file.

---

## 8. Self-Review (master)

- **Spec coverage:** all 16 PRD command verbs, MCP server, dream's 7 passes, 18 tests, cross-tool integration, installer — each maps to a phase plan in §2. ✔
- **Type consistency:** §3 contract names (`Note`, `Frontmatter`, `Edge`, `RelType`, `ProposedDiff`, `JsonEnvelope`, `LlmProvider`, `Embedder`) are the canonical names every phase plan uses. ✔
- **Parallelism:** waves in §2 have no intra-wave dependency edges. ✔

---

## 9. Phase Plan Index

Execute in this order. Each is a standalone plan with TDD tasks.

1. `2026-06-27-kgx-phase0-skeleton.md` — workspace, `kgx-core` contracts, `kgx-okf`, `kgx-vault`, `kg init`, `kg validate`.
2. `2026-06-27-kgx-phase1-brain.md` — `kgx-tokens`, `kgx-graph` (schema, FTS, vec, KNN), `kg index --full/--incremental`.
3. `2026-06-27-kgx-phase2-ask.md` — `kgx-llm`, `kgx-extract`, `kgx-retrieval` (RRF, PPR), `kg capture/extract/link/search/recall/ask`.
4. `2026-06-27-kgx-phase3-dream.md` — `kgx-dream` (7 passes), `kg dream`, `kg review`.
5. `2026-06-27-kgx-phase4-graphrag.md` — Leiden communities + summaries in `kgx-graph`/`kgx-retrieval`, `--scope global`.
6. `2026-06-27-kgx-phase5-mcp-skills.md` — `kgx-mcp`, `kgx-rtk`, `kgx-ponytail`, `kgx-cron`, cross-tool skills, `kg mcp-server/cron`.
7. `2026-06-27-kgx-phase6-polish.md` — `kgx-viz`, `kgx-docs`, `kg dashboard/graph/docs/ship/pull/status/tokens`, `install.sh`, CI finalization.
