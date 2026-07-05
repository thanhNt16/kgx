# Retrieval Pipeline Upgrade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade default hybrid search to candidate-gen (BM25/LIKE/tags/dense/SPLADE) → RRF → entity-seeded PPR → local cross-encoder rerank, gated by an expanded 45-question bench.

**Architecture:** All work happens inside the existing crates: `kgx-core` gains two traits (`Reranker`, `SparseEmbedder`), `kgx-graph` gains the ONNX implementations + sparse postings storage (fastembed is already its dependency), `kgx-retrieval` gains the pipeline stages, `kgx-llm::select` gains env-var selection. `search()` changes signature once to take a `Retrievers` bundle so later stages never touch callers again. Sparse indexing runs as a post-build step in the `kg index` command so `build_full`'s ~12 test callers are untouched.

**Tech Stack:** Rust workspace, fastembed 4.9.1 (`TextRerank` + `SparseTextEmbedding`, local ONNX), rusqlite/FTS5/sqlite-vec, Python bench harness.

**Spec:** `docs/superpowers/specs/2026-07-05-retrieval-pipeline-design.md`

## Global Constraints

- Default search path: local ONNX only, offline after first download, **no LLM tokens per query**. LLM rerank stays behind `--rerank-llm`.
- Latency gate: **p95 end-to-end `kg search` < 200 ms** on the 220-note bench corpus (recorded in `bench/results.json`).
- Env contract: `KGX_RERANK` = unset→off / `jina-turbo` / `bge-base` / `on` / `off` / `mock`; `KGX_RERANK_MODEL` = `jina-turbo` (default) / `bge-base`; `KGX_RERANK_TOPK` default `30`; `KGX_SPARSE` = unset→on (when `semantic` built) / `off` / `mock`.
- Entity seeding constants: cosine threshold **0.60**, cap **5**, seed weight **0.5/(i+1)**.
- Sparse RRF ranking uses **k = 60** (same as BM25/dense).
- `SCHEMA_VERSION` becomes **3** (spec allocation; battletest-gaps WS2 owns 2). Phase 3 (Tasks 11+) requires battletest-gaps WS2 (POLE) merged; Tasks 1–10 only require WS1 (already on this branch).
- Degradation is visible, never silent: missing model → one `eprintln!` warning + stage skipped; result shape never changes.
- Bench floors (Task 13): original 15 questions (`cohort: v1`) Recall@5 ≥ 0.85; `vocab-mismatch` ≥ 0.70; `multi-hop` ≥ 0.60; `temporal` ≥ 0.60; `entity-relation` ≥ 0.70.
- Repo finish gate: every commit must pass `cargo fmt --all --check`, `git diff --check`, and the workspace tests (the Stop hook runs `.kgx/hooks/verify-finished.sh`).

## File Map

| File | Responsibility |
|---|---|
| `crates/kgx-core/src/llm.rs` | + `Reranker`, `SparseEmbedder` traits, `SparseVec` alias |
| `crates/kgx-graph/src/rerank.rs` (new) | `MockReranker`, `FastEmbedReranker` (cfg semantic) |
| `crates/kgx-graph/src/sparse_embed.rs` (new) | `MockSparseEmbedder`, `FastEmbedSparse` (cfg semantic) |
| `crates/kgx-graph/src/sparse.rs` (new) | postings storage + `sparse_search` + `index_sparse` |
| `crates/kgx-graph/src/schema.rs`, `migrate.rs` | `sparse_postings` table, v3 forward-fill |
| `crates/kgx-graph/src/knn.rs`, `query.rs` | `pub cosine`, `entity_scores`, `has_edges` |
| `crates/kgx-llm/src/select.rs` | `rerank_choice`/`reranker_from_env`, `sparse_choice`/`sparse_from_env`, `retrieval_label` |
| `crates/kgx-retrieval/src/hybrid.rs` | `Retrievers` bundle, sparse ranking, PPR un-gate, entity seeds, rerank stage |
| `crates/kgx-retrieval/src/rerank.rs` (new) | `apply_rerank` (fetch texts, score, reorder) |
| `crates/kgx-retrieval/src/ppr.rs` | `select_entity_seeds` pure fn |
| `crates/kgx-cli/src/commands/{search,ask,index,status}.rs` | wire env selections; status `retrieval:` line |
| `crates/kgx-mcp/src/tools/{nl_query,deep_search}.rs` | pass `Retrievers` |
| `crates/kgx-dream/src/passes/{orphan_repair,open_questions}.rs` | pass `Retrievers` (no reranker — dream stays cheap) |
| `bench/gen_corpus.py`, `bench/bench.py` | +30 gold questions (cohort v2), per-category aggregates + gates |

---

## Phase 1 — Cross-encoder rerank + PPR un-gate

### Task 1: `Reranker` trait + `MockReranker` + `FastEmbedReranker`

**Files:**
- Modify: `crates/kgx-core/src/llm.rs`
- Create: `crates/kgx-graph/src/rerank.rs`
- Modify: `crates/kgx-graph/src/lib.rs` (add `pub mod rerank;`)

**Interfaces:**
- Produces: `kgx_core::llm::Reranker` — `fn rerank(&self, query: &str, docs: &[(String, String)]) -> Result<Vec<f32>>` (docs are `(id, text)`; returns one score per doc, same order) and `fn model_name(&self) -> String`. `kgx_graph::rerank::MockReranker` (always compiled; score = count of query tokens appearing in the doc text, case-insensitive). `kgx_graph::rerank::FastEmbedReranker::load(model: &str) -> Result<Self>` (cfg `semantic`; `"bge-base"` → `RerankerModel::BGERerankerBase`, anything else → `RerankerModel::JINARerankerV1TurboEn`).

- [ ] **Step 1: Write the failing test**

Append to the bottom of the new file `crates/kgx-graph/src/rerank.rs` (write the whole file in Step 3; the test module is part of it — to follow strict TDD, first create the file with ONLY the test module and a `use` of the not-yet-written types):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use kgx_core::llm::Reranker;

    #[test]
    fn mock_scores_by_query_token_overlap() {
        let r = MockReranker;
        let docs = vec![
            ("a".to_string(), "flink checkpoint interval tuning".to_string()),
            ("b".to_string(), "s3 lifecycle policy".to_string()),
        ];
        let scores = r.rerank("flink checkpoint", &docs).unwrap();
        assert_eq!(scores.len(), 2);
        assert!(scores[0] > scores[1], "doc a mentions both query tokens");
        assert_eq!(r.model_name(), "mock");
    }

    #[test]
    fn mock_is_deterministic() {
        let r = MockReranker;
        let docs = vec![("x".to_string(), "kafka event bus".to_string())];
        let a = r.rerank("kafka", &docs).unwrap();
        let b = r.rerank("kafka", &docs).unwrap();
        assert_eq!(a, b);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kgx-graph rerank -- --nocapture`
Expected: compile error — `MockReranker` not found (add `pub mod rerank;` to `crates/kgx-graph/src/lib.rs` first so the module is compiled).

- [ ] **Step 3: Write minimal implementation**

Append to `crates/kgx-core/src/llm.rs`:

```rust
/// Sparse embedding: (term_id, weight) pairs. term_id is i64 for SQLite affinity.
pub type SparseVec = Vec<(i64, f32)>;

/// Sparse (SPLADE-style) text embedder for lexical-expansion retrieval.
pub trait SparseEmbedder: Send + Sync {
    fn embed_sparse(&self, texts: &[String]) -> crate::Result<Vec<SparseVec>>;
}

/// Cross-encoder relevance scorer: reads query and document together.
pub trait Reranker: Send + Sync {
    /// Score each (id, text) doc for relevance to `query`.
    /// Returns one score per doc, in the same order as `docs`.
    fn rerank(&self, query: &str, docs: &[(String, String)]) -> crate::Result<Vec<f32>>;
    fn model_name(&self) -> String;
}
```

(The `SparseEmbedder`/`SparseVec` items are used from Task 7 onward; adding them now keeps this file touched once.)

Top of `crates/kgx-graph/src/rerank.rs` (above the test module from Step 1):

```rust
use kgx_core::{llm::Reranker, Result};

/// Deterministic reranker for tests: score = number of query tokens
/// (lowercased, len > 1) that appear as substrings of the doc text.
pub struct MockReranker;

impl Reranker for MockReranker {
    fn rerank(&self, query: &str, docs: &[(String, String)]) -> Result<Vec<f32>> {
        let tokens: Vec<String> = query
            .split_whitespace()
            .filter(|t| t.len() > 1)
            .map(|t| t.to_lowercase())
            .collect();
        Ok(docs
            .iter()
            .map(|(_, text)| {
                let hay = text.to_lowercase();
                tokens.iter().filter(|t| hay.contains(t.as_str())).count() as f32
            })
            .collect())
    }

    fn model_name(&self) -> String {
        "mock".into()
    }
}

/// Local ONNX cross-encoder via fastembed. Downloads once, cached in
/// the fastembed cache dir, then fully offline.
#[cfg(feature = "semantic")]
pub struct FastEmbedReranker {
    model: fastembed::TextRerank,
    name: String,
}

#[cfg(feature = "semantic")]
impl FastEmbedReranker {
    pub fn load(model: &str) -> Result<Self> {
        let (m, name) = match model {
            "bge-base" => (fastembed::RerankerModel::BGERerankerBase, "bge-base"),
            _ => (
                fastembed::RerankerModel::JINARerankerV1TurboEn,
                "jina-turbo",
            ),
        };
        let model = fastembed::TextRerank::try_new(
            fastembed::RerankInitOptions::new(m).with_show_download_progress(false),
        )
        .map_err(|e| kgx_core::KgError::Other(format!("failed to load reranker: {e}")))?;
        Ok(Self {
            model,
            name: name.into(),
        })
    }
}

#[cfg(feature = "semantic")]
impl Reranker for FastEmbedReranker {
    fn rerank(&self, query: &str, docs: &[(String, String)]) -> Result<Vec<f32>> {
        let texts: Vec<&str> = docs.iter().map(|(_, t)| t.as_str()).collect();
        let results = self
            .model
            .rerank(query, texts, false, None)
            .map_err(|e| kgx_core::KgError::Other(format!("rerank error: {e}")))?;
        // fastembed returns results sorted by score; map back to input order.
        let mut scores = vec![0.0f32; docs.len()];
        for r in results {
            if r.index < scores.len() {
                scores[r.index] = r.score;
            }
        }
        Ok(scores)
    }

    fn model_name(&self) -> String {
        self.name.clone()
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p kgx-graph rerank && cargo build --workspace`
Expected: 2 tests PASS, workspace builds.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-core/src/llm.rs crates/kgx-graph/src/rerank.rs crates/kgx-graph/src/lib.rs
git commit -m "feat(retrieval): Reranker + SparseEmbedder traits; mock and fastembed rerankers"
```

---

### Task 2: rerank env selection in `kgx-llm::select`

**Files:**
- Modify: `crates/kgx-llm/src/select.rs`

**Interfaces:**
- Consumes: `kgx_core::llm::Reranker`, `kgx_graph::rerank::{MockReranker, FastEmbedReranker}` (Task 1).
- Produces: `RerankChoice { Off, Mock, FastEmbed(String) }`, `rerank_choice(var: Option<&str>, model_var: Option<&str>, semantic_built: bool) -> RerankChoice`, `reranker_from_env() -> Option<Box<dyn Reranker>>`, `rerank_topk_from_env() -> usize` (default 30).

- [ ] **Step 1: Write the failing test**

Append inside the existing `mod tests` in `crates/kgx-llm/src/select.rs`:

```rust
    #[test]
    fn rerank_choice_defaults_on_when_semantic_built() {
        assert_eq!(
            rerank_choice(None, None, true),
            RerankChoice::FastEmbed("jina-turbo".into())
        );
        assert_eq!(rerank_choice(None, None, false), RerankChoice::Off);
    }

    #[test]
    fn rerank_choice_off_mock_and_model_override() {
        assert_eq!(rerank_choice(Some("off"), None, true), RerankChoice::Off);
        assert_eq!(rerank_choice(Some("mock"), None, true), RerankChoice::Mock);
        assert_eq!(
            rerank_choice(None, Some("bge-base"), true),
            RerankChoice::FastEmbed("bge-base".into())
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kgx-llm rerank_choice`
Expected: compile error — `rerank_choice` not found.

- [ ] **Step 3: Write minimal implementation**

Add to `crates/kgx-llm/src/select.rs` (below the `EmbedChoice` block), and add `Reranker` to the existing `use kgx_core::llm::{...}` import:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RerankChoice {
    Off,
    Mock,
    FastEmbed(String),
}

/// Pure selection logic. `var` = KGX_RERANK, `model_var` = KGX_RERANK_MODEL.
pub fn rerank_choice(
    var: Option<&str>,
    model_var: Option<&str>,
    semantic_built: bool,
) -> RerankChoice {
    match var {
        Some("off") => RerankChoice::Off,
        Some("mock") => RerankChoice::Mock,
        _ if semantic_built => {
            RerankChoice::FastEmbed(model_var.unwrap_or("jina-turbo").to_string())
        }
        _ => RerankChoice::Off,
    }
}

pub fn reranker_from_env() -> Option<Box<dyn kgx_core::llm::Reranker>> {
    let var = std::env::var("KGX_RERANK").ok();
    let model_var = std::env::var("KGX_RERANK_MODEL").ok();
    match rerank_choice(var.as_deref(), model_var.as_deref(), cfg!(feature = "semantic")) {
        RerankChoice::Off => None,
        RerankChoice::Mock => Some(Box::new(kgx_graph::rerank::MockReranker)),
        #[cfg(feature = "semantic")]
        RerankChoice::FastEmbed(model) => match kgx_graph::rerank::FastEmbedReranker::load(&model)
        {
            Ok(r) => Some(Box::new(r)),
            Err(e) => {
                eprintln!("warning: reranker failed to load, rerank stage disabled: {e}");
                None
            }
        },
        #[cfg(not(feature = "semantic"))]
        RerankChoice::FastEmbed(_) => None,
    }
}

pub fn rerank_topk_from_env() -> usize {
    std::env::var("KGX_RERANK_TOPK")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p kgx-llm`
Expected: all tests PASS (existing embed_choice tests + 2 new).

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-llm/src/select.rs
git commit -m "feat(retrieval): KGX_RERANK env selection (off/mock/jina-turbo/bge-base)"
```

---

### Task 3: `Retrievers` bundle + rerank stage in `search()`

**Files:**
- Create: `crates/kgx-retrieval/src/rerank.rs`
- Modify: `crates/kgx-retrieval/src/hybrid.rs`, `crates/kgx-retrieval/src/lib.rs`
- Modify (callers): `crates/kgx-cli/src/commands/search.rs`, `crates/kgx-cli/src/commands/ask.rs`, `crates/kgx-mcp/src/tools/nl_query.rs`, `crates/kgx-mcp/src/tools/deep_search.rs`, `crates/kgx-dream/src/passes/orphan_repair.rs`, `crates/kgx-dream/src/passes/open_questions.rs`, plus any `search(` call in `crates/kgx-retrieval/tests/hybrid.rs`
- Test: `crates/kgx-retrieval/tests/rerank_stage.rs`

**Interfaces:**
- Consumes: `Reranker` trait + `MockReranker` (Task 1).
- Produces: `kgx_retrieval::Retrievers<'a> { embedder, llm, reranker, sparse }` with `Retrievers::new(&dyn Embedder)` and builder methods `with_llm/with_reranker/with_sparse` (each takes an `Option<&'a dyn _>`); **new search signature** `search(brain: &Brain, r: &Retrievers, query: &str, opts: SearchOpts) -> Result<Vec<SearchHit>>`; `SearchOpts` gains `rerank_topk: usize` (Default = 30); `apply_rerank(reranker: &dyn Reranker, brain: &Brain, query: &str, fused: Vec<(String, f32)>, topk: usize) -> Result<Vec<(String, f32)>>`. The `sparse` field is `Option<&'a dyn SparseEmbedder>` and stays unused until Task 10.

- [ ] **Step 1: Write the failing test**

Create `crates/kgx-retrieval/tests/rerank_stage.rs`:

```rust
use kgx_graph::{build::build_full, rerank::MockReranker, Brain};
use kgx_retrieval::{search, Mode, Retrievers, SearchOpts};
use kgx_vault::scan::scan_vault;

fn fixture_brain(dir: &std::path::Path) -> Brain {
    // Three notes: two mention "flink", one strongly matches the query text.
    let notes_dir = dir.join("notes");
    std::fs::create_dir_all(&notes_dir).unwrap();
    for (slug, title, body) in [
        ("a", "flink checkpoint tuning", "flink checkpoint interval set to 60s"),
        ("b", "flink deployment", "flink runs on kubernetes"),
        ("c", "s3 lifecycle", "tier to glacier after 90 days"),
    ] {
        std::fs::write(
            notes_dir.join(format!("{slug}.md")),
            format!(
                "---\ntype: fact\nid: 01TESTRERANK{:0>14}\ntitle: {title}\nstatus: active\ntags: [t]\nlinks: []\n---\n{body}\n",
                slug.to_uppercase()
            ),
        )
        .unwrap();
    }
    let notes = scan_vault(dir).unwrap();
    let mut brain = Brain::open(&dir.join("brain.sqlite")).unwrap();
    let embedder = kgx_graph::embed::MockEmbedder::new();
    build_full(&mut brain, &notes, &embedder).unwrap();
    brain
}

#[test]
fn rerank_signal_present_and_best_overlap_ranks_first() {
    let tmp = tempfile::tempdir().unwrap();
    let brain = fixture_brain(tmp.path());
    let embedder = kgx_graph::embed::MockEmbedder::new();
    let reranker = MockReranker;
    let r = Retrievers::new(&embedder).with_reranker(Some(&reranker));
    let hits = search(
        &brain,
        &r,
        "flink checkpoint interval",
        SearchOpts {
            mode: Mode::Keyword,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!hits.is_empty());
    assert!(
        hits[0].signals.contains(&"rerank".to_string()),
        "top hit should carry the rerank signal: {:?}",
        hits[0].signals
    );
    // MockReranker scores token overlap: note a mentions flink+checkpoint+interval.
    assert!(hits[0].id.ends_with('A'), "hits: {hits:?}");
}

#[test]
fn no_reranker_means_no_rerank_signal() {
    let tmp = tempfile::tempdir().unwrap();
    let brain = fixture_brain(tmp.path());
    let embedder = kgx_graph::embed::MockEmbedder::new();
    let r = Retrievers::new(&embedder);
    let hits = search(
        &brain,
        &r,
        "flink checkpoint interval",
        SearchOpts {
            mode: Mode::Keyword,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(hits.iter().all(|h| !h.signals.contains(&"rerank".to_string())));
}
```

Note: `scan_vault` does not validate id format (verified: `crates/kgx-vault/src/scan.rs` only sorts by id), and `format!("01TESTRERANK{:0>14}", …)` yields distinct 26-char ids per note.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kgx-retrieval --test rerank_stage`
Expected: compile error — `Retrievers` not found.

- [ ] **Step 3: Implement `Retrievers` + rerank stage**

Create `crates/kgx-retrieval/src/rerank.rs`:

```rust
use kgx_core::{llm::Reranker, KgError, Result};
use kgx_graph::Brain;

/// Rerank the top `topk` fused candidates with a cross-encoder; keep the
/// remainder below them in fused order. Doc text = first 512 chars of
/// the note's raw_text (the brain does not store titles separately).
pub fn apply_rerank(
    reranker: &dyn Reranker,
    brain: &Brain,
    query: &str,
    fused: Vec<(String, f32)>,
    topk: usize,
) -> Result<Vec<(String, f32)>> {
    if fused.is_empty() || topk == 0 {
        return Ok(fused);
    }
    let head_len = topk.min(fused.len());
    let (head, tail) = fused.split_at(head_len);

    let mut docs: Vec<(String, String)> = Vec::with_capacity(head_len);
    for (id, _) in head {
        let body: String = brain
            .conn()
            .query_row(
                "SELECT raw_text FROM notes WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .map_err(|e| KgError::Brain(e.to_string()))?;
        let snippet: String = body.chars().take(512).collect();
        docs.push((id.clone(), snippet));
    }

    let scores = reranker.rerank(query, &docs)?;
    let mut reranked: Vec<(String, f32)> = docs
        .into_iter()
        .zip(scores)
        .map(|((id, _), s)| (id, s))
        .collect();
    reranked.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    reranked.extend(tail.iter().cloned());
    Ok(reranked)
}
```

In `crates/kgx-retrieval/src/hybrid.rs`:

1. Add the bundle (below `SearchOpts`), add `rerank_topk` to `SearchOpts` + its `Default` (value 30), and import `kgx_core::llm::{Reranker, SparseEmbedder}`:

```rust
/// Bundle of model handles for the search pipeline. Only `embedder` is
/// required; every optional stage degrades to a no-op when absent.
pub struct Retrievers<'a> {
    pub embedder: &'a dyn Embedder,
    pub llm: Option<&'a dyn LlmProvider>,
    pub reranker: Option<&'a dyn Reranker>,
    pub sparse: Option<&'a dyn SparseEmbedder>,
}

impl<'a> Retrievers<'a> {
    pub fn new(embedder: &'a dyn Embedder) -> Self {
        Self {
            embedder,
            llm: None,
            reranker: None,
            sparse: None,
        }
    }
    pub fn with_llm(mut self, llm: Option<&'a dyn LlmProvider>) -> Self {
        self.llm = llm;
        self
    }
    pub fn with_reranker(mut self, reranker: Option<&'a dyn Reranker>) -> Self {
        self.reranker = reranker;
        self
    }
    pub fn with_sparse(mut self, sparse: Option<&'a dyn SparseEmbedder>) -> Self {
        self.sparse = sparse;
        self
    }
}
```

2. Change the signature and body plumbing of `search`:

```rust
pub fn search(
    brain: &Brain,
    r: &Retrievers,
    query: &str,
    opts: SearchOpts,
) -> Result<Vec<SearchHit>> {
```

Inside the body replace every `embedder` with `r.embedder` and the `llm` parameter use with `r.llm` (the `if opts.rerank_llm { if let Some(llm) = llm …}` block becomes `r.llm`). Delete the old `embedder: &dyn Embedder` / `llm: Option<&dyn LlmProvider>` parameters.

3. Insert the cross-encoder stage AFTER the `filter_entities` block and BEFORE the `rerank_llm` block:

```rust
    // Stage 4: local cross-encoder rerank of the fused head.
    if let Some(reranker) = r.reranker {
        let head = opts.rerank_topk.min(fused.len());
        fused = crate::rerank::apply_rerank(reranker, brain, query, fused, opts.rerank_topk)?;
        for (id, _) in fused.iter().take(head) {
            signals_for
                .entry(id.clone())
                .or_default()
                .push("rerank".into());
        }
    }
```

4. In `crates/kgx-retrieval/src/lib.rs`:

```rust
pub mod community_summary;
pub mod global;
pub mod hybrid;
pub mod ppr;
pub mod rerank;
pub mod rrf;
pub use hybrid::{search, Mode, Retrievers, SearchHit, SearchOpts};
```

- [ ] **Step 4: Update the six callers**

Pattern for each (adapt variable names in place; `reranker_from_env` only in CLI/MCP, **not** in dream passes):

`crates/kgx-cli/src/commands/search.rs` — replace the `search(&brain, embedder.as_ref(), query, SearchOpts{…}, llm.as_deref())` call:

```rust
    let reranker = kgx_llm::select::reranker_from_env();
    let r = kgx_retrieval::Retrievers::new(embedder.as_ref())
        .with_llm(llm.as_deref())
        .with_reranker(reranker.as_deref());
    let hits = search(
        &brain,
        &r,
        query,
        SearchOpts {
            mode: m,
            limit,
            expand_ppr: true,
            filter_entities: false,
            rerank_graph,
            rerank_llm,
            rerank_topk: kgx_llm::select::rerank_topk_from_env(),
        },
    )?;
```

`crates/kgx-cli/src/commands/ask.rs`, `crates/kgx-mcp/src/tools/nl_query.rs`, `crates/kgx-mcp/src/tools/deep_search.rs` — same transformation: build `Retrievers::new(<embedder>).with_llm(<existing llm arg>).with_reranker(reranker.as_deref())` with `let reranker = kgx_llm::select::reranker_from_env();` above it, keep every existing `SearchOpts` field value, and add `rerank_topk: kgx_llm::select::rerank_topk_from_env()`.

`crates/kgx-dream/src/passes/orphan_repair.rs`, `crates/kgx-dream/src/passes/open_questions.rs` — dream stays cheap (no reranker, no LLM in search):

```rust
    let r = kgx_retrieval::Retrievers::new(ctx.embedder);
    let hits = search(brain, &r, <existing query expr>, SearchOpts { <existing fields>, rerank_topk: 0 })?;
```

(Whatever the existing embedder/llm expressions are in those passes, keep them — only the call shape changes. If a struct-update `..Default::default()` is already used, `rerank_topk` is covered by Default.)

`crates/kgx-retrieval/tests/hybrid.rs` — update existing `search(...)` calls to the new shape with `Retrievers::new(&embedder)`.

- [ ] **Step 5: Run tests**

Run: `cargo test --workspace`
Expected: all PASS including the 2 new rerank_stage tests. Fix any missed caller the compiler reports.

- [ ] **Step 6: Commit**

```bash
git add -A crates/
git commit -m "feat(retrieval): Retrievers bundle + default cross-encoder rerank stage"
```

---

### Task 4: un-gate PPR from the dense embedder

**Files:**
- Modify: `crates/kgx-graph/src/query.rs` (add `has_edges`), `crates/kgx-retrieval/src/hybrid.rs`
- Test: `crates/kgx-retrieval/tests/rerank_stage.rs` (append)

**Interfaces:**
- Produces: `kgx_graph::query::has_edges(brain: &Brain) -> Result<bool>`.

- [ ] **Step 1: Write the failing test**

Append to `crates/kgx-retrieval/tests/rerank_stage.rs` — this requires linked notes, so extend the fixture: change the body of note `b` in `fixture_brain` to `"flink runs on kubernetes. See [[flink checkpoint tuning]]"` (creates an edge b→a via wikilink title resolution). Then:

```rust
#[test]
fn ppr_runs_in_keyword_mode_when_graph_has_edges() {
    let tmp = tempfile::tempdir().unwrap();
    let brain = fixture_brain(tmp.path());
    let embedder = kgx_graph::embed::MockEmbedder::new();
    let r = Retrievers::new(&embedder);
    let hits = search(
        &brain,
        &r,
        "kubernetes",
        SearchOpts {
            mode: Mode::Keyword,
            ..Default::default()
        },
    )
    .unwrap();
    // "kubernetes" only matches note b lexically; PPR should surface
    // its neighbor a with a ppr signal.
    assert!(
        hits.iter()
            .any(|h| h.signals.contains(&"ppr".to_string())),
        "ppr should fire without a dense embedder: {hits:?}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kgx-retrieval --test rerank_stage ppr_runs`
Expected: FAIL — assertion (today the PPR block is inside `has_vector`).

- [ ] **Step 3: Implement**

Append to `crates/kgx-graph/src/query.rs`:

```rust
pub fn has_edges(brain: &Brain) -> Result<bool> {
    brain
        .conn()
        .query_row("SELECT EXISTS(SELECT 1 FROM edges LIMIT 1)", [], |r| {
            r.get(0)
        })
        .map_err(|e| KgError::Brain(e.to_string()))
}
```

In `crates/kgx-retrieval/src/hybrid.rs`, change the PPR gate line

```rust
    if opts.expand_ppr && has_vector && !fused.is_empty() {
```

to

```rust
    if opts.expand_ppr && !fused.is_empty() && kgx_graph::query::has_edges(brain)? {
```

(`has_vector` becomes unused — delete the variable and its assignment; the semantic branch no longer needs it.)

- [ ] **Step 4: Run tests**

Run: `cargo test --workspace`
Expected: PASS. If an existing hybrid test asserted PPR absence in keyword mode, update it to the new behavior (PPR now legitimately fires).

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-graph/src/query.rs crates/kgx-retrieval/
git commit -m "fix(retrieval): run PPR whenever the graph has edges, not only with dense vectors"
```

---

### Task 5: `kg status` retrieval line

**Files:**
- Modify: `crates/kgx-llm/src/select.rs`, `crates/kgx-cli/src/commands/status.rs`

**Interfaces:**
- Produces: `kgx_llm::select::retrieval_label() -> String`, e.g. `"bm25+like+tags+dense | ppr | rerank(jina-turbo)"`; `StatusSnapshot` gains `pub retrieval: String`.

- [ ] **Step 1: Write the failing test**

Append inside `mod tests` in `crates/kgx-cli/src/commands/status.rs`:

```rust
    #[test]
    fn retrieval_label_lists_stages() {
        let label = kgx_llm::select::retrieval_label();
        assert!(label.contains("bm25"), "always-on keyword stage: {label}");
        assert!(label.contains("ppr"), "graph stage always listed: {label}");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kgx-cli retrieval_label`
Expected: compile error — `retrieval_label` not found.

- [ ] **Step 3: Implement**

Append to `crates/kgx-llm/src/select.rs` (the `sparse_choice` referenced here arrives in Task 8 — until then, gate on rerank only; this exact code compiles now and Task 8 extends it):

```rust
/// One-line summary of active retrieval stages for `kg status`.
pub fn retrieval_label() -> String {
    let mut candidates = String::from("bm25+like+tags");
    let var = std::env::var("KGX_EMBED").ok();
    if matches!(
        embed_choice(var.as_deref(), cfg!(feature = "semantic"), cfg!(feature = "candle")),
        EmbedChoice::FastEmbed | EmbedChoice::MiniLm
    ) {
        candidates.push_str("+dense");
    }
    let rerank = {
        let var = std::env::var("KGX_RERANK").ok();
        let model = std::env::var("KGX_RERANK_MODEL").ok();
        match rerank_choice(var.as_deref(), model.as_deref(), cfg!(feature = "semantic")) {
            RerankChoice::Off => String::from("rerank(off)"),
            RerankChoice::Mock => String::from("rerank(mock)"),
            RerankChoice::FastEmbed(m) => format!("rerank({m})"),
        }
    };
    format!("{candidates} | ppr | {rerank}")
}
```

In `crates/kgx-cli/src/commands/status.rs`: add `pub retrieval: String,` to `StatusSnapshot`, set `retrieval: kgx_llm::select::retrieval_label(),` in `snapshot()`, and extend the human printer:

```rust
        println!(
            "nodes={} edges={} orphans={} pending={} embedder={}\nretrieval: {}",
            s.nodes, s.edges, s.orphans, s.pending_diffs, s.embedder, s.retrieval
        )
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p kgx-cli && cargo test -p smoke`
Expected: PASS (cli_status integration test tolerates the added JSON field).

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-llm/src/select.rs crates/kgx-cli/src/commands/status.rs
git commit -m "feat(status): show active retrieval stages"
```

---

### Task 6: Phase-1 bench checkpoint

**Files:**
- Modify: `bench/results.json` (regenerated)

**Interfaces:**
- Consumes: the `kg` binary with Tasks 1–5 landed; existing `bench/gen_corpus.py`, `bench/bench.py`.

- [ ] **Step 1: Build release binary and corpus**

```bash
cargo build --release
python3 bench/gen_corpus.py /tmp/kgx-corpus
rm -rf /tmp/kgx-bench-vault && cp -r /tmp/kgx-corpus /tmp/kgx-bench-vault
cd /tmp/kgx-bench-vault && PATH=/Users/harry/Desktop/kgx/target/release:$PATH kg index --full
```

Expected: index completes; first run downloads the MiniLM embed model (~40 MB) and the jina-turbo reranker (~38 MB). If the sandbox blocks the download, re-run with network permission — models must be cached before timing.

Note: the reranker downloads lazily at first `kg search`, not at index. Warm it once before timing: `cd /tmp/kgx-bench-vault && PATH=…:$PATH kg search "warmup" >/dev/null`.

- [ ] **Step 2: Run the bench**

```bash
cd /Users/harry/Desktop/kgx && PATH=$PWD/target/release:$PATH \
  python3 bench/bench.py /tmp/kgx-bench-vault /tmp/kgx-bench-vault/gold.json bench/results.json
```

Expected: `recall_at_5 >= 0.85` and `mrr >= 0.7` in `with_kgx.aggregate` (embeddings + rerank should lift the 0.733/0.7 keyword baseline). Check `p95_latency_ms`; if ≥ 200, retry with `KGX_RERANK_TOPK=15` and record which setting was used in the commit message. If recall regressed instead, STOP and investigate before committing (compare per_query against the old results to see which question flipped).

- [ ] **Step 3: Commit**

```bash
git add bench/results.json
git commit -m "bench: phase-1 checkpoint (dense default + cross-encoder rerank + ppr un-gate)"
```

---

## Phase 2 — SPLADE sparse signal

### Task 7: `sparse_postings` storage (schema v3) + scoring

**Files:**
- Modify: `crates/kgx-graph/src/schema.rs`, `crates/kgx-graph/src/migrate.rs`, `crates/kgx-graph/src/build.rs` (DELETE lines only), `crates/kgx-graph/src/lib.rs` (add `pub mod sparse;`)
- Create: `crates/kgx-graph/src/sparse.rs`

**Interfaces:**
- Produces: `kgx_graph::sparse::replace_sparse(conn: &rusqlite::Connection, note_id: &str, sv: &SparseVec) -> Result<()>`; `sparse_search(brain: &Brain, query_sparse: &SparseVec, limit: usize) -> Result<Vec<(String, f32)>>` (score = Σ q_weight·doc_weight, desc, ties by id); `index_sparse(brain: &Brain, notes: &[&kgx_core::Note], sparse: &dyn SparseEmbedder) -> Result<usize>` (embeds `"{title}\n{body}"` like the dense path, returns count indexed).

- [ ] **Step 1: Write the failing test**

Create the test module at the bottom of `crates/kgx-graph/src/sparse.rs` (file starts with only this + imports):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Brain;

    fn mem_brain() -> Brain {
        let dir = tempfile::tempdir().unwrap();
        let brain = Brain::open(&dir.join("b.sqlite")).unwrap();
        std::mem::forget(dir); // keep tempdir alive for the test process
        brain
    }

    #[test]
    fn dot_product_scoring_hand_computed() {
        let brain = mem_brain();
        // note X: terms {1: 0.5, 2: 2.0}; note Y: terms {2: 1.0}
        replace_sparse(brain.conn(), "X", &vec![(1, 0.5), (2, 2.0)]).unwrap();
        replace_sparse(brain.conn(), "Y", &vec![(2, 1.0)]).unwrap();
        // query {1: 1.0, 2: 1.0} → X = 1.0*0.5 + 1.0*2.0 = 2.5, Y = 1.0
        let hits = sparse_search(&brain, &vec![(1, 1.0), (2, 1.0)], 10).unwrap();
        assert_eq!(hits[0].0, "X");
        assert!((hits[0].1 - 2.5).abs() < 1e-6);
        assert_eq!(hits[1].0, "Y");
        assert!((hits[1].1 - 1.0).abs() < 1e-6);
    }

    #[test]
    fn replace_overwrites_previous_postings() {
        let brain = mem_brain();
        replace_sparse(brain.conn(), "X", &vec![(1, 1.0)]).unwrap();
        replace_sparse(brain.conn(), "X", &vec![(9, 1.0)]).unwrap();
        assert!(sparse_search(&brain, &vec![(1, 1.0)], 10).unwrap().is_empty());
        assert_eq!(sparse_search(&brain, &vec![(9, 1.0)], 10).unwrap().len(), 1);
    }
}
```

Note: if `Brain::open` takes `&Path` and the tempdir juggling is awkward, mirror how existing `crates/kgx-graph/tests/*.rs` construct brains — hold the `TempDir` in a variable inside each test instead of `mem_brain()`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kgx-graph sparse`
Expected: compile error — `replace_sparse` not found (add `pub mod sparse;` to lib.rs first).

- [ ] **Step 3: Implement**

`crates/kgx-graph/src/schema.rs` — bump the version and add the table to the `SCHEMA` const:

```rust
pub const SCHEMA_VERSION: i32 = 3;
```

and inside the `SCHEMA` string, after the `communities` table line:

```sql
CREATE TABLE IF NOT EXISTS sparse_postings (
  term_id INTEGER NOT NULL, note_id TEXT NOT NULL, weight REAL NOT NULL,
  PRIMARY KEY (term_id, note_id));
CREATE INDEX IF NOT EXISTS idx_sparse_note ON sparse_postings(note_id);
```

(If battletest-gaps WS2 already set `SCHEMA_VERSION` to 2 on this branch, 2→3 is the change; if it is still 1, going 1→3 is fine — versions are allocations and the forward-fill below is idempotent.)

`crates/kgx-graph/src/migrate.rs` — before the final `Ok(current.max(1))`, add a generic forward-fill (all DDL in `SCHEMA` is `IF NOT EXISTS`, so re-running the batch is safe; this coexists with any WS2 ALTER logic already present):

```rust
    if current < SCHEMA_VERSION {
        conn.execute_batch(crate::schema::SCHEMA)
            .map_err(|e| KgError::Brain(e.to_string()))?;
        conn.execute(
            "INSERT OR IGNORE INTO schema_version (version, applied_at) VALUES (?1, ?2)",
            rusqlite::params![SCHEMA_VERSION, kgx_core::util::now_iso()],
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
        return Ok(SCHEMA_VERSION);
    }
```

`crates/kgx-graph/src/build.rs` — two DELETE additions: in `build_full`'s `execute_batch` string append `DELETE FROM sparse_postings;`; in `build_incremental`'s per-note loop (next to the `DELETE FROM edges WHERE src_id=?1` line) add:

```rust
        tx.execute("DELETE FROM sparse_postings WHERE note_id=?1", params![n.fm.id])
            .map_err(|e| KgError::Brain(e.to_string()))?;
```

`crates/kgx-graph/src/sparse.rs` above the tests:

```rust
use crate::Brain;
use kgx_core::llm::{SparseEmbedder, SparseVec};
use kgx_core::{KgError, Note, Result};
use std::collections::HashMap;

pub fn replace_sparse(
    conn: &rusqlite::Connection,
    note_id: &str,
    sv: &SparseVec,
) -> Result<()> {
    conn.execute(
        "DELETE FROM sparse_postings WHERE note_id=?1",
        rusqlite::params![note_id],
    )
    .map_err(|e| KgError::Brain(e.to_string()))?;
    let mut stmt = conn
        .prepare("INSERT OR REPLACE INTO sparse_postings (term_id, note_id, weight) VALUES (?1, ?2, ?3)")
        .map_err(|e| KgError::Brain(e.to_string()))?;
    for (term, w) in sv {
        stmt.execute(rusqlite::params![term, note_id, w])
            .map_err(|e| KgError::Brain(e.to_string()))?;
    }
    Ok(())
}

/// Dot-product scoring over the inverted index: one indexed lookup per
/// query term, accumulated in memory (queries have ~20–100 terms).
pub fn sparse_search(
    brain: &Brain,
    query_sparse: &SparseVec,
    limit: usize,
) -> Result<Vec<(String, f32)>> {
    if query_sparse.is_empty() {
        return Ok(vec![]);
    }
    let mut scores: HashMap<String, f32> = HashMap::new();
    let mut stmt = brain
        .conn()
        .prepare("SELECT note_id, weight FROM sparse_postings WHERE term_id=?1")
        .map_err(|e| KgError::Brain(e.to_string()))?;
    for (term, qw) in query_sparse {
        let rows = stmt
            .query_map(rusqlite::params![term], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)? as f32))
            })
            .map_err(|e| KgError::Brain(e.to_string()))?;
        for row in rows {
            let (id, w) = row.map_err(|e| KgError::Brain(e.to_string()))?;
            *scores.entry(id).or_insert(0.0) += qw * w;
        }
    }
    let mut out: Vec<(String, f32)> = scores.into_iter().collect();
    out.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    out.truncate(limit);
    Ok(out)
}

/// Embed and store sparse postings for `notes`. Returns notes indexed.
pub fn index_sparse(
    brain: &Brain,
    notes: &[&Note],
    sparse: &dyn SparseEmbedder,
) -> Result<usize> {
    if notes.is_empty() {
        return Ok(0);
    }
    let texts: Vec<String> = notes
        .iter()
        .map(|n| format!("{}\n{}", n.fm.title, n.body))
        .collect();
    let vecs = sparse.embed_sparse(&texts)?;
    for (n, sv) in notes.iter().zip(&vecs) {
        replace_sparse(brain.conn(), &n.fm.id, sv)?;
    }
    Ok(notes.len())
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p kgx-graph`
Expected: 2 new sparse tests + all existing PASS (migrate forward-fill must not break `ensure_schema` tests if any).

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-graph/
git commit -m "feat(sparse): sparse_postings inverted index (schema v3) with dot-product search"
```

---

### Task 8: sparse embedders + env selection

**Files:**
- Create: `crates/kgx-graph/src/sparse_embed.rs`
- Modify: `crates/kgx-graph/src/lib.rs` (add `pub mod sparse_embed;`), `crates/kgx-llm/src/select.rs`

**Interfaces:**
- Consumes: `SparseEmbedder`/`SparseVec` traits (Task 1).
- Produces: `kgx_graph::sparse_embed::MockSparseEmbedder` (FNV token hash → `term_id = (h % 100_000) as i64`, weight = token count in text); `FastEmbedSparse::load() -> Result<Self>` (cfg semantic, SPLADE++ default model); `kgx_llm::select::{SparseChoice, sparse_choice(var, semantic_built) -> SparseChoice, sparse_from_env() -> Option<Box<dyn SparseEmbedder>>}`; `retrieval_label()` extended with a `+sparse` segment.

- [ ] **Step 1: Write the failing tests**

Test module for `crates/kgx-graph/src/sparse_embed.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use kgx_core::llm::SparseEmbedder;

    #[test]
    fn mock_sparse_is_deterministic_and_counts_tokens() {
        let e = MockSparseEmbedder;
        let a = e.embed_sparse(&["kafka kafka bus".into()]).unwrap();
        let b = e.embed_sparse(&["kafka kafka bus".into()]).unwrap();
        assert_eq!(a, b);
        // two distinct terms; "kafka" has weight 2.0
        assert_eq!(a[0].len(), 2);
        assert!(a[0].iter().any(|(_, w)| (*w - 2.0).abs() < 1e-6));
    }
}
```

And in `crates/kgx-llm/src/select.rs` `mod tests`:

```rust
    #[test]
    fn sparse_choice_defaults_on_when_semantic_built() {
        assert_eq!(sparse_choice(None, true), SparseChoice::FastEmbed);
        assert_eq!(sparse_choice(None, false), SparseChoice::Off);
        assert_eq!(sparse_choice(Some("off"), true), SparseChoice::Off);
        assert_eq!(sparse_choice(Some("mock"), true), SparseChoice::Mock);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p kgx-graph sparse_embed; cargo test -p kgx-llm sparse_choice`
Expected: compile errors — types not found.

- [ ] **Step 3: Implement**

`crates/kgx-graph/src/sparse_embed.rs` (above the tests):

```rust
use kgx_core::llm::{SparseEmbedder, SparseVec};
use kgx_core::Result;
use std::collections::BTreeMap;

/// Deterministic sparse embedder for tests: FNV-hash each token
/// (lowercased, len > 1) into a term id, weight = occurrence count.
pub struct MockSparseEmbedder;

impl SparseEmbedder for MockSparseEmbedder {
    fn embed_sparse(&self, texts: &[String]) -> Result<Vec<SparseVec>> {
        Ok(texts
            .iter()
            .map(|t| {
                let mut counts: BTreeMap<i64, f32> = BTreeMap::new();
                for word in t.split_whitespace().filter(|w| w.len() > 1) {
                    let h = word.to_lowercase().bytes().fold(
                        1469598103934665603u64,
                        |a, b| (a ^ b as u64).wrapping_mul(1099511628211),
                    );
                    *counts.entry((h % 100_000) as i64).or_insert(0.0) += 1.0;
                }
                counts.into_iter().collect()
            })
            .collect())
    }
}

/// SPLADE++ learned-sparse embedder via fastembed (local ONNX,
/// ~130 MB one-time download).
#[cfg(feature = "semantic")]
pub struct FastEmbedSparse {
    model: fastembed::SparseTextEmbedding,
}

#[cfg(feature = "semantic")]
impl FastEmbedSparse {
    pub fn load() -> Result<Self> {
        let model = fastembed::SparseTextEmbedding::try_new(
            fastembed::SparseInitOptions::new(fastembed::SparseModel::SPLADEPPV1)
                .with_show_download_progress(false),
        )
        .map_err(|e| kgx_core::KgError::Other(format!("failed to load SPLADE model: {e}")))?;
        Ok(Self { model })
    }
}

#[cfg(feature = "semantic")]
impl SparseEmbedder for FastEmbedSparse {
    fn embed_sparse(&self, texts: &[String]) -> Result<Vec<SparseVec>> {
        let out = self
            .model
            .embed(texts.to_vec(), None)
            .map_err(|e| kgx_core::KgError::Other(format!("splade error: {e}")))?;
        Ok(out
            .into_iter()
            .map(|se| {
                se.indices
                    .into_iter()
                    .zip(se.values)
                    .map(|(i, v)| (i as i64, v))
                    .collect()
            })
            .collect())
    }
}
```

`crates/kgx-llm/src/select.rs` additions:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SparseChoice {
    Off,
    Mock,
    FastEmbed,
}

/// Pure selection logic. `var` = KGX_SPARSE.
pub fn sparse_choice(var: Option<&str>, semantic_built: bool) -> SparseChoice {
    match var {
        Some("off") => SparseChoice::Off,
        Some("mock") => SparseChoice::Mock,
        _ if semantic_built => SparseChoice::FastEmbed,
        _ => SparseChoice::Off,
    }
}

pub fn sparse_from_env() -> Option<Box<dyn kgx_core::llm::SparseEmbedder>> {
    let var = std::env::var("KGX_SPARSE").ok();
    match sparse_choice(var.as_deref(), cfg!(feature = "semantic")) {
        SparseChoice::Off => None,
        SparseChoice::Mock => Some(Box::new(kgx_graph::sparse_embed::MockSparseEmbedder)),
        #[cfg(feature = "semantic")]
        SparseChoice::FastEmbed => match kgx_graph::sparse_embed::FastEmbedSparse::load() {
            Ok(s) => Some(Box::new(s)),
            Err(e) => {
                eprintln!("warning: SPLADE failed to load, sparse stage disabled: {e}");
                None
            }
        },
        #[cfg(not(feature = "semantic"))]
        SparseChoice::FastEmbed => None,
    }
}
```

And extend `retrieval_label()`: after the `+dense` push, add

```rust
    let svar = std::env::var("KGX_SPARSE").ok();
    if !matches!(
        sparse_choice(svar.as_deref(), cfg!(feature = "semantic")),
        SparseChoice::Off
    ) {
        candidates.push_str("+sparse");
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p kgx-graph && cargo test -p kgx-llm`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-graph/ crates/kgx-llm/src/select.rs
git commit -m "feat(sparse): SPLADE++ + mock sparse embedders with KGX_SPARSE selection"
```

---

### Task 9: index sparse postings in `kg index`

**Files:**
- Modify: `crates/kgx-cli/src/commands/index.rs`
- Test: `crates/kgx-cli/tests/cli_index.rs` (append)

**Interfaces:**
- Consumes: `kgx_graph::sparse::index_sparse` (Task 7), `kgx_llm::select::sparse_from_env` (Task 8).

- [ ] **Step 1: Write the failing test**

Append to `crates/kgx-cli/tests/cli_index.rs`:

```rust
#[test]
fn index_full_populates_sparse_postings_with_mock() {
    let d = common::copy_fixture();
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .env("KGX_EMBED", "mock")
        .env("KGX_SPARSE", "mock")
        .env("KGX_RERANK", "off")
        .args(["index", "--full", "--json"])
        .current_dir(d.path())
        .assert()
        .success();
    let conn = rusqlite::Connection::open(d.path().join(".kg/brain.sqlite")).unwrap();
    let n: i64 = conn
        .query_row("SELECT count(*) FROM sparse_postings", [], |r| r.get(0))
        .unwrap();
    assert!(n > 0, "sparse postings should be written by kg index");
}
```

(If `rusqlite` is not already a dev-dependency of `kgx-cli`, add `rusqlite = { version = "0.31", features = ["bundled"] }` under `[dev-dependencies]` in `crates/kgx-cli/Cargo.toml`.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kgx-cli --test cli_index sparse`
Expected: FAIL — count is 0 (nothing writes postings yet).

- [ ] **Step 3: Implement**

In `crates/kgx-cli/src/commands/index.rs`, after the `build_full`/`build_incremental` call succeeds (both paths), add:

```rust
    // Sparse (SPLADE) postings: post-build step so the brain build API
    // stays unchanged. Skipped with a visible reason when unavailable.
    if let Some(sparse) = kgx_llm::select::sparse_from_env() {
        let target: Vec<&kgx_core::Note> = if full_rebuild {
            notes.iter().collect()
        } else {
            notes
                .iter()
                .filter(|n| changed_ids.contains(&n.fm.id))
                .collect()
        };
        kgx_graph::sparse::index_sparse(&brain, &target, sparse.as_ref())?;
    }
    // Spec rule: models download at index time, never mid-search. Loading
    // the reranker here warms its cache; the handle is dropped immediately.
    // Gated on a real dense embedder so mock-mode tests stay offline.
    if embedder.is_semantic() {
        let _ = kgx_llm::select::reranker_from_env();
    }
```

Adapt the two variable names to the actual ones in `index.rs` (`full_rebuild` = whatever boolean distinguishes the full path; `changed_ids` = the incremental id list; read the file first — if the full/incremental split is two separate blocks, put the full variant in one and the filtered variant in the other).

- [ ] **Step 4: Run tests**

Run: `cargo test -p kgx-cli --test cli_index`
Expected: PASS (new + existing).

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-cli/
git commit -m "feat(sparse): kg index writes SPLADE postings (post-build step)"
```

---

### Task 10: sparse ranking in hybrid search

**Files:**
- Modify: `crates/kgx-retrieval/src/hybrid.rs`
- Modify (callers, one line each): `crates/kgx-cli/src/commands/search.rs`, `crates/kgx-cli/src/commands/ask.rs`, `crates/kgx-mcp/src/tools/nl_query.rs`, `crates/kgx-mcp/src/tools/deep_search.rs` — add `.with_sparse(sparse.as_deref())` with `let sparse = kgx_llm::select::sparse_from_env();` above (dream passes stay sparse-less).
- Test: `crates/kgx-retrieval/tests/rerank_stage.rs` (append)

**Interfaces:**
- Consumes: `Retrievers.sparse` (Task 3), `sparse_search`/`replace_sparse` (Task 7), `MockSparseEmbedder` (Task 8).

- [ ] **Step 1: Write the failing test**

Append to `crates/kgx-retrieval/tests/rerank_stage.rs`:

```rust
#[test]
fn sparse_ranking_contributes_signal() {
    let tmp = tempfile::tempdir().unwrap();
    let brain = fixture_brain(tmp.path());
    // Manually index postings with the mock sparse embedder (unit-level
    // stand-in for `kg index`).
    let sparse = kgx_graph::sparse_embed::MockSparseEmbedder;
    let notes = kgx_vault::scan::scan_vault(tmp.path()).unwrap();
    let refs: Vec<&kgx_core::Note> = notes.iter().collect();
    kgx_graph::sparse::index_sparse(&brain, &refs, &sparse).unwrap();

    let embedder = kgx_graph::embed::MockEmbedder::new();
    let r = Retrievers::new(&embedder).with_sparse(Some(&sparse));
    let hits = search(
        &brain,
        &r,
        "glacier",
        SearchOpts {
            mode: Mode::Hybrid,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        hits.iter().any(|h| h.signals.contains(&"sparse".to_string())),
        "sparse signal expected: {hits:?}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kgx-retrieval --test rerank_stage sparse`
Expected: FAIL — no `sparse` signal (stage not wired).

- [ ] **Step 3: Implement**

In `crates/kgx-retrieval/src/hybrid.rs`, inside the `Keyword | Hybrid` candidate block, after the tag-expansion ranking, add Ranking 4:

```rust
        // Ranking 4: SPLADE sparse (learned term expansion).
        if let Some(sparse) = r.sparse {
            match sparse.embed_sparse(&[query.to_string()]) {
                Ok(mut qv) if !qv.is_empty() => {
                    let q = qv.remove(0);
                    if let Ok(sp) = kgx_graph::sparse::sparse_search(brain, &q, 50) {
                        if !sp.is_empty() {
                            for (id, _) in &sp {
                                signals_for
                                    .entry(id.clone())
                                    .or_default()
                                    .push("sparse".into());
                            }
                            rankings.push(sp.into_iter().map(|(id, _)| id).collect());
                            ks.push(60.0);
                        }
                    }
                }
                Ok(_) => {}
                Err(e) => eprintln!("warning: sparse query embed failed, stage skipped: {e}"),
            }
        }
```

Then the four caller one-liners listed under **Files**.

- [ ] **Step 4: Run tests**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit + Phase-2 bench checkpoint**

```bash
git add -A crates/
git commit -m "feat(sparse): SPLADE ranking joins RRF fusion (signal: sparse)"
# Re-run the Task 6 bench sequence (rebuild release, regen corpus, reindex, bench).
# First run downloads the SPLADE model (~130 MB). Expect vocab-mismatch
# improvement on the 4 hard questions; recall_at_5 must not drop.
git add bench/results.json
git commit -m "bench: phase-2 checkpoint (sparse signal active)"
```

---

## Phase 3 — Entity-seeded PPR + expanded bench

> Requires battletest-gaps WS2 (POLE entity notes + typed edges) merged. The seeding works with plain `type: entity` notes either way; typed edges just make it stronger.

### Task 11: entity-seeded PPR

**Files:**
- Modify: `crates/kgx-graph/src/knn.rs` (make `cosine` pub, add `entity_scores`), `crates/kgx-retrieval/src/ppr.rs` (add `select_entity_seeds`), `crates/kgx-retrieval/src/hybrid.rs` (wire seeds)
- Test: `crates/kgx-retrieval/src/ppr.rs` tests module + `crates/kgx-graph/src/knn.rs` tests module

**Interfaces:**
- Produces: `kgx_graph::knn::entity_scores(brain: &Brain, query_emb: &[f32]) -> Result<Vec<(String, f32)>>` (cosine vs `type='entity'` notes, desc); `kgx_retrieval::ppr::select_entity_seeds(scored: &[(String, f32)], threshold: f32, cap: usize) -> Vec<(String, f32)>` (filter ≥ threshold, take cap, weight `0.5/(i+1)`).

- [ ] **Step 1: Write the failing tests**

In `crates/kgx-retrieval/src/ppr.rs`, add at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_seeds_filter_cap_and_weight() {
        let scored = vec![
            ("e1".to_string(), 0.9),
            ("e2".to_string(), 0.7),
            ("e3".to_string(), 0.59), // below threshold
        ];
        let seeds = select_entity_seeds(&scored, 0.60, 5);
        assert_eq!(seeds.len(), 2);
        assert_eq!(seeds[0], ("e1".to_string(), 0.5));
        assert_eq!(seeds[1], ("e2".to_string(), 0.25));
    }

    #[test]
    fn entity_seeds_respect_cap() {
        let scored: Vec<(String, f32)> =
            (0..10).map(|i| (format!("e{i}"), 0.9)).collect();
        assert_eq!(select_entity_seeds(&scored, 0.60, 5).len(), 5);
    }
}
```

In `crates/kgx-graph/src/knn.rs`, add a tests module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_basics() {
        assert!((cosine(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!(cosine(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 0.0]), 0.0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p kgx-retrieval select_entity_seeds; cargo test -p kgx-graph cosine_basics`
Expected: compile errors — `select_entity_seeds` not found; `cosine` private.

- [ ] **Step 3: Implement**

`crates/kgx-graph/src/knn.rs`: change `fn cosine` to `pub fn cosine`, then append:

```rust
/// Cosine similarity of the query against all entity-note embeddings.
/// Used for HippoRAG-style PPR seeding; sorted descending.
pub fn entity_scores(brain: &Brain, query_emb: &[f32]) -> Result<Vec<(String, f32)>> {
    let mut stmt = brain
        .conn()
        .prepare("SELECT id, embedding FROM notes WHERE type='entity' AND embedding IS NOT NULL")
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| {
            let id: String = r.get(0)?;
            let blob: Vec<u8> = r.get(1)?;
            Ok((id, blob))
        })
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let mut scored: Vec<(String, f32)> = Vec::new();
    for row in rows {
        let (id, blob) = row.map_err(|e| KgError::Brain(e.to_string()))?;
        scored.push((id, cosine(query_emb, &blob_to_f32(&blob))));
    }
    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    Ok(scored)
}
```

`crates/kgx-retrieval/src/ppr.rs`, above the new tests:

```rust
/// HippoRAG-style seed selection: entities whose embedding cosine to the
/// query is >= `threshold`, capped, weighted at half the BM25 harmonic
/// scale (0.5/(i+1)).
pub fn select_entity_seeds(
    scored: &[(String, f32)],
    threshold: f32,
    cap: usize,
) -> Vec<(String, f32)> {
    scored
        .iter()
        .filter(|(_, s)| *s >= threshold)
        .take(cap)
        .enumerate()
        .map(|(i, (id, _))| (id.clone(), 0.5 / (i + 1) as f32))
        .collect()
}
```

`crates/kgx-retrieval/src/hybrid.rs` — in the semantic branch, keep the query embedding available for seeding: change

```rust
    if matches!(opts.mode, Mode::Semantic | Mode::Hybrid) && r.embedder.is_semantic() {
        let q = r.embedder.embed(&[query.to_string()])?.remove(0);
```

so that `q` survives the block (declare `let mut query_emb: Option<Vec<f32>> = None;` before the block and set `query_emb = Some(q.clone());` inside it). Then, in the PPR block, extend the seeds after `bm25_weighted_seeds` is chosen:

```rust
        let mut seeds: Vec<(String, f32)> = if !bm25_weighted_seeds.is_empty() {
            bm25_weighted_seeds
        } else {
            fused
                .iter()
                .take(5)
                .enumerate()
                .map(|(i, (id, _))| (id.clone(), 1.0 / (i + 1) as f32))
                .collect()
        };
        // Entity seeding (HippoRAG-style): only with a real dense embedder.
        if let Some(q) = &query_emb {
            if let Ok(scored) = kgx_graph::knn::entity_scores(brain, q) {
                seeds.extend(crate::ppr::select_entity_seeds(&scored, 0.60, 5));
            }
        }
        let ppr = personalized(brain, &seeds, 0.85, 20)?;
```

- [ ] **Step 4: Run tests**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-graph/src/knn.rs crates/kgx-retrieval/
git commit -m "feat(retrieval): HippoRAG-style entity-seeded PPR (threshold 0.60, cap 5)"
```

---

### Task 12: expanded gold set (15 → 45)

**Files:**
- Modify: `bench/gen_corpus.py`

**Interfaces:**
- Produces: `gold.json` entries gain `"cohort": "v1"|"v2"`; 30 new v2 entries in categories `vocab-mismatch`, `multi-hop`, `temporal`, `entity-relation`.

- [ ] **Step 1: Add cohort to the helper and mark existing questions v1**

In `bench/gen_corpus.py`, change `g()`:

```python
def g(question, *title_keywords, patterns=None, category="?", sprint=1, limit=1, cohort="v1"):
    return {
        "question": question,
        "relevant_note_ids": find_ids(titles_index, *title_keywords, limit=limit),
        "expected_patterns": patterns or [title_keywords[0]],
        "category": category,
        "evidence_sprint": sprint,
        "cohort": cohort,
    }
```

- [ ] **Step 2: Append the v2 questions**

After the existing `gold = [...]` list, add (`v2` helper avoids repeating `cohort=`):

```python
def g2(question, *kw, **kwargs):
    kwargs.setdefault("cohort", "v2")
    return g(question, *kw, **kwargs)

gold += [
    # --- vocab-mismatch: paraphrases with minimal lexical overlap ---
    g2("Which storage layer did the lakehouse standardize on?", "iceberg", "table format", category="vocab-mismatch", sprint=2),
    g2("How do we page the on-call when invoicing jobs break?", "pagerduty", category="vocab-mismatch", sprint=2),
    g2("What slows down our stream processing jobs?", "backpressure", category="vocab-mismatch", sprint=6),
    g2("How quickly must analyst ad-hoc queries come back?", "trino catalog", category="vocab-mismatch", sprint=1),
    g2("Where do old files get archived to cut costs?", "glacier", category="vocab-mismatch", sprint=12),
    g2("How do we prevent duplicate charges flowing through the event bus?", "exactly", category="vocab-mismatch", sprint=14),
    g2("Who handles paging and incident response?", "cara", category="vocab-mismatch", sprint=1),
    g2("What tool did we standardize scheduled data jobs on?", "airflow", "batch", category="vocab-mismatch", sprint=10),
    g2("Why do we merge small data files overnight?", "compaction", category="vocab-mismatch", sprint=24),
    g2("What guards against double-charging customers when jobs retry?", "idempotency", category="vocab-mismatch", sprint=12),
    # --- multi-hop: answer reachable via an entity/edge hop ---
    g2("Which storage-tiering decision did the Platform Lead record?", "glacier", category="multi-hop", sprint=12),
    g2("What did the Senior SRE learn about stream stability?", "backpressure", category="multi-hop", sprint=6),
    g2("Which incident hit the federated query engine?", "trino", "oom", category="multi-hop", sprint=13),
    g2("What checkpoint tuning touched the event bus topic?", "checkpoint", "60s", category="multi-hop", sprint=1),
    g2("What work moved the page_views job onto the batch orchestrator?", "page_views", category="multi-hop", sprint=1),
    g2("Which ADR protects the legacy OLTP billing table during cutover?", "backfill", "billing", category="multi-hop", sprint=8),
    g2("Who owns the ETL pipelines that feed the warehouse?", "david", category="multi-hop", sprint=1),
    g2("Which decision came from the architecture owner about federated queries?", "trino", "federated", category="multi-hop", sprint=6),
    g2("What alerting routes to the platform on-call rotation?", "pagerduty", category="multi-hop", sprint=2),
    g2("Which partition layout did the ETL engineer land for events?", "partition spec", category="multi-hop", sprint=1),
    # --- temporal: supersession chains ---
    g2("What decision replaced the Airflow batch orchestration ADR?", "glacier", category="temporal", sprint=12),
    g2("Which ADR revised the billing backfill approach?", "exactly", category="temporal", sprint=14),
    g2("What is the current partition spec guidance for the events table?", "partition spec", category="temporal", sprint=20, limit=2),
    g2("Which early cron-to-Airflow migration fact is now superseded?", "page_views", category="temporal", sprint=22, limit=2),
    g2("When did the original Trino catalog fact become stale?", "trino catalog", category="temporal", sprint=24, limit=2),
    # --- entity-relation ---
    g2("Who is the Platform Lead?", "bob", category="entity-relation", sprint=1),
    g2("Who owns SLOs and alerting?", "cara", category="entity-relation", sprint=1),
    g2("Who runs the Spark and Airflow ETL pipelines?", "david", category="entity-relation", sprint=1),
    g2("Who decided to tier storage to Glacier?", "glacier", category="entity-relation", sprint=12),
    g2("Who introduced the exactly-once Kafka connector policy?", "exactly", category="entity-relation", sprint=14),
]
```

- [ ] **Step 3: Regenerate and verify resolution**

```bash
python3 bench/gen_corpus.py /tmp/kgx-corpus
python3 - <<'EOF'
import json
gold = json.load(open('/tmp/kgx-corpus/gold.json'))
print("total:", len(gold))
from collections import Counter
print(Counter(e["category"] for e in gold))
unresolved = [e["question"] for e in gold if not e["relevant_note_ids"]]
print("unresolved:", unresolved)
EOF
```

Expected: total = 45 (the generator drops unresolved entries — if total < 45, the printed `unresolved` list is empty but the count is short; adjust the failing questions' `title_keywords` until all 30 resolve against actual note titles; `find_ids` needs every keyword as a substring of one title).

- [ ] **Step 4: Commit**

```bash
git add bench/gen_corpus.py
git commit -m "bench: expand gold set to 45 questions (vocab-mismatch, multi-hop, temporal, entity-relation)"
```

---

### Task 13: per-category bench + gates

**Files:**
- Modify: `bench/bench.py`, `bench/results.json` (regenerated), `bench/manifest.json` (regenerated)

**Interfaces:**
- Produces: `results.json` `with_kgx` gains `"by_category": {<cat>: {n, recall_at_5, mrr}}` and `"gates": {passed: bool, checks: [...]}`; `bench.py --gates` exits 1 on any floor failure.

- [ ] **Step 1: Implement category aggregation + gates**

In `bench/bench.py`: carry `cohort` through per_query (in `run_with_kgx`, add `"cohort": entry.get("cohort", "v1"),` next to the `category` line). Then add:

```python
FLOORS = {
    "v1_recall_at_5": 0.85,
    "vocab-mismatch": 0.70,
    "multi-hop": 0.60,
    "temporal": 0.60,
    "entity-relation": 0.70,
    "p95_latency_ms": 200.0,
}

def by_category(per_query):
    cats = {}
    for q in per_query:
        cats.setdefault(q["category"], []).append(q)
    return {
        c: {
            "n": len(qs),
            "recall_at_5": round(sum(q["recall"] for q in qs) / len(qs), 4),
            "mrr": round(sum(q["mrr"] for q in qs) / len(qs), 4),
        }
        for c, qs in cats.items()
    }

def check_gates(with_kgx):
    per_query = with_kgx["per_query"]
    cats = by_category(per_query)
    v1 = [q for q in per_query if q.get("cohort", "v1") == "v1"]
    checks = []
    v1_recall = sum(q["recall"] for q in v1) / len(v1) if v1 else 0.0
    checks.append({"gate": "v1_recall_at_5", "value": round(v1_recall, 4),
                   "floor": FLOORS["v1_recall_at_5"], "pass": v1_recall >= FLOORS["v1_recall_at_5"]})
    for cat in ("vocab-mismatch", "multi-hop", "temporal", "entity-relation"):
        if cat in cats:
            v = cats[cat]["recall_at_5"]
            checks.append({"gate": cat, "value": v, "floor": FLOORS[cat], "pass": v >= FLOORS[cat]})
    p95 = with_kgx["aggregate"].get("p95_latency_ms", 0)
    checks.append({"gate": "p95_latency_ms", "value": p95,
                   "floor": FLOORS["p95_latency_ms"], "pass": p95 < FLOORS["p95_latency_ms"]})
    return {"passed": all(c["pass"] for c in checks), "checks": checks}
```

Wire into the main flow: after `with_kgx` is computed, set `with_kgx["by_category"] = by_category(with_kgx["per_query"])` and `with_kgx["gates"] = check_gates(with_kgx)`; print the category table and each gate line (`PASS`/`FAIL`); if `"--gates" in sys.argv` and not `gates["passed"]`, `sys.exit(1)`.

Also add `--category <name>` filtering (spec requirement) — right after `gold` is loaded:

```python
if "--category" in sys.argv:
    want = sys.argv[sys.argv.index("--category") + 1]
    gold = [e for e in gold if e.get("category") == want]
```

(Place the flag parsing before positional-arg handling breaks; the existing script reads positionals by index, so strip the flag pair from `sys.argv` after reading it: `i = sys.argv.index("--category"); del sys.argv[i:i+2]` — do the same for `--gates` with `del sys.argv[i:i+1]`, and read both flags before `VAULT`/`GOLD`/`OUT_JSON` are assigned by moving those three assignments below the flag handling.)

- [ ] **Step 2: Run the full expanded bench**

Repeat the Task 6 sequence (release build, gen corpus, fresh vault, `kg index --full`, warmup search), then:

```bash
cd /Users/harry/Desktop/kgx && PATH=$PWD/target/release:$PATH \
  python3 bench/bench.py /tmp/kgx-bench-vault /tmp/kgx-bench-vault/gold.json bench/results.json --gates
echo "exit=$?"
```

Expected: exit=0 with all gates PASS. If a category misses its floor, tune in this order and re-run: (1) `KGX_RERANK_TOPK` 30→50 for recall vs latency trade; (2) entity-seed threshold 0.60→0.50 for multi-hop/entity-relation (constant lives in `hybrid.rs`, Task 11 step 3); (3) sparse RRF k 60→40 for vocab-mismatch (Task 10 step 3). Each knob change is one commit with the bench delta in the message. If floors remain unreachable after those three knobs, STOP and report actual vs floor per category — the floors came from the spec and may need a spec-level revisit, which is the user's call, not the executor's.

- [ ] **Step 3: Commit**

```bash
git add bench/bench.py bench/results.json bench/manifest.json
git commit -m "bench: per-category aggregates + acceptance gates (45-question set)"
```

---

### Task 14: docs sync + final verification

**Files:**
- Modify: `README.md` (search/env-var sections), `AGENTS.md` (if it documents search modes), `skills/claude/.claude/skills/kgx/SKILL.md` + the other three harness skill files ONLY IF they enumerate env vars (grep first — do not add tool-name changes; the 9 MCP tool names are unchanged)

**Interfaces:** none — documentation and gate.

- [ ] **Step 1: Update docs**

In `README.md`, find the search/semantic section (grep `KGX_EMBED`) and document the pipeline + new env vars, matching Global Constraints verbatim:

```markdown
### Retrieval pipeline

`kg search` runs a four-stage local pipeline: BM25 + LIKE + tags + dense
vector + SPLADE sparse candidates → reciprocal-rank fusion → personalized
PageRank over the note graph (seeded by BM25 hits and query-matched
entities) → local cross-encoder rerank. All models are local ONNX
(fastembed), downloaded once and cached; no LLM tokens are spent per query.

| Env var | Values | Default |
|---|---|---|
| `KGX_EMBED` | `fastembed` / `minilm` / `mock` / `off` | `fastembed` |
| `KGX_SPARSE` | on / `off` / `mock` | on |
| `KGX_RERANK` | on / `off` / `mock` | on |
| `KGX_RERANK_MODEL` | `jina-turbo` / `bge-base` | `jina-turbo` |
| `KGX_RERANK_TOPK` | integer | `30` |

`kg status` prints the active stages (`retrieval: bm25+like+tags+dense+sparse | ppr | rerank(jina-turbo)`).
```

Run `grep -rn "KGX_EMBED\|semantic search" AGENTS.md skills/` and mirror the same env table wherever search configuration is already documented (do not introduce new sections in files that don't discuss search).

- [ ] **Step 2: Full verification**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
```

Expected: all clean. Fix anything that isn't.

- [ ] **Step 3: Commit**

```bash
git add README.md AGENTS.md skills/
git commit -m "docs: retrieval pipeline stages and env vars"
```

---

## Execution notes

- Tasks 1–10 need only WS1 (already on `kgx-battletest-gaps`). Task 11+ wants WS2 merged; entity seeding still works on plain `type: entity` notes if WS2 is delayed.
- Tasks 6, 10 (checkpoint), and 13 download models (~40 + 38 + 130 MB) and need network on first run; everything after is offline.
- The Stop hook (`.kgx/hooks/verify-finished.sh`) runs fmt + `git diff --check` + workspace tests on finish — run `cargo fmt --all` before every commit.
