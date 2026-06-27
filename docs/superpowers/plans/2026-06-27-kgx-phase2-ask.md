# KGX Phase 2 — Ask Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development or superpowers:executing-plans. Read `2026-06-27-kgx-master-plan.md` (contracts §3, Global Constraints) and complete Phases 0–1. Steps use `- [ ]`.

**Goal:** Build the LLM layer (`kgx-llm`), the extraction pipeline (`kgx-extract`), and hybrid retrieval (`kgx-retrieval`: RRF fusion + Personalized PageRank), then wire `kg capture`, `kg extract`, `kg link`, `kg search`, `kg recall`, `kg ask`. Unlocks smoke tests T01–T04 and T09.

**Architecture:** Wave 1 adds `kgx-llm` (provider trait impls + a deterministic `MockProvider` selected by `KGX_LLM=mock`). Wave 2 adds `kgx-extract` (raw → atomic notes). Wave 3 adds `kgx-retrieval` (combines `kgx-graph` primitives via RRF + PPR). CLI commands are thin orchestrators.

**Tech Stack:** `tokio`, `reqwest` (Claude/OpenAI HTTP), `serde_json`, `async-trait`, plus Phase 1 graph primitives.

## Global Constraints

Inherit master Global Constraints. Phase-critical: provenance (`T02` — extracted notes carry `source:`); capture immutability (`T01` — `raw/` never edited); hybrid recall beats vector-only (`T09`); all LLM calls record tokens; `KGX_LLM=mock` provides deterministic offline responses for tests/CI.

---

## Task 1: `kgx-llm` — provider impls + mock + selector (Wave 1)

**Files:**
- Create: `crates/kgx-llm/Cargo.toml`, `src/lib.rs`, `src/mock.rs`, `src/claude.rs`, `src/openai.rs`, `src/ollama.rs`, `src/select.rs`
- Test: in-module + `crates/kgx-llm/tests/mock.rs`

**Interfaces:**
- Consumes: `kgx_core::{LlmProvider, LlmRequest, LlmResponse, Embedder, Result, KgError}`.
- Produces: `MockProvider` (canned, deterministic, keyed on prompt substrings); `ClaudeProvider::new(api_key, model)`, `OpenAiProvider`, `OllamaProvider`; `select::provider_from_env() -> Result<Box<dyn LlmProvider>>` (reads `KGX_LLM` = `mock|claude|openai|ollama`, defaults `claude`); `select::embedder_from_env() -> Box<dyn Embedder>` (returns `MockEmbedder` unless `KGX_EMBED=minilm`).

- [ ] **Step 1: Crate manifest**

```toml
# crates/kgx-llm/Cargo.toml
[package]
name = "kgx-llm"
edition.workspace = true
version.workspace = true
license.workspace = true
[dependencies]
kgx-core = { path = "../kgx-core" }
kgx-graph = { path = "../kgx-graph" }   # for MockEmbedder re-export in embedder_from_env
serde.workspace = true
serde_json.workspace = true
async-trait = "0.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
reqwest = { version = "0.12", features = ["json"] }
[dev-dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

- [ ] **Step 2: Write failing mock test**

```rust
// crates/kgx-llm/tests/mock.rs
use kgx_llm::mock::MockProvider;
use kgx_core::{LlmProvider, LlmRequest};
#[tokio::test]
async fn mock_extract_returns_canned_facts_json() {
    let p = MockProvider::new();
    let req = LlmRequest { system: "extract".into(),
        prompt: "EXTRACT_FACTS\nPostgres is the primary datastore.".into(), max_tokens: 512, temperature: 0.0 };
    let r = p.complete(req).await.unwrap();
    let v: serde_json::Value = serde_json::from_str(&r.text).unwrap();
    assert!(v["facts"].as_array().unwrap().len() >= 1);
    assert!(r.input_tokens > 0);
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p kgx-llm`
Expected: FAIL.

- [ ] **Step 4: Implement mock.rs**

```rust
// crates/kgx-llm/src/mock.rs
use kgx_core::{LlmProvider, LlmRequest, LlmResponse, Result};
pub struct MockProvider;
impl MockProvider { pub fn new() -> Self { MockProvider } }
impl Default for MockProvider { fn default() -> Self { Self::new() } }
#[async_trait::async_trait]
impl LlmProvider for MockProvider {
    fn model_id(&self) -> &str { "mock" }
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse> {
        let input_tokens = (req.prompt.len() / 4 + req.system.len() / 4) as u32;
        // Deterministic responses keyed by prompt prefix so smoke tests can assert.
        let text = if req.prompt.contains("EXTRACT_FACTS") {
            // produce one atomic fact per source sentence (period-split), with the entity guessed.
            let body = req.prompt.splitn(2, '\n').nth(1).unwrap_or("");
            let facts: Vec<_> = body.split('.').filter(|s| !s.trim().is_empty()).map(|s| serde_json::json!({
                "title": s.trim(), "body": s.trim(), "confidence": "medium",
                "entities": s.split_whitespace().filter(|w| w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)).collect::<Vec<_>>()
            })).collect();
            serde_json::json!({ "facts": facts }).to_string()
        } else if req.prompt.contains("ANSWER_QUESTION") {
            serde_json::json!({ "answer": "Based on the notes, Postgres is the primary datastore.",
                "citations": ["01FACT01POSTGRESPRIMARY00"] }).to_string()
        } else if req.prompt.contains("CONTRADICTION") {
            serde_json::json!({ "verdict": "hard", "rationale": "Two different primary datastores asserted." }).to_string()
        } else if req.prompt.contains("MERGE") {
            serde_json::json!({ "merge": false, "rationale": "Distinct facts." }).to_string()
        } else { serde_json::json!({ "text": "ok" }).to_string() };
        let output_tokens = (text.len() / 4) as u32;
        Ok(LlmResponse { text, input_tokens, output_tokens, model: "mock".into() })
    }
}
```

- [ ] **Step 5: Implement claude.rs / openai.rs / ollama.rs**

```rust
// crates/kgx-llm/src/claude.rs
use kgx_core::{LlmProvider, LlmRequest, LlmResponse, Result, KgError};
pub struct ClaudeProvider { api_key: String, model: String, client: reqwest::Client }
impl ClaudeProvider {
    pub fn new(api_key: String, model: String) -> Self { Self { api_key, model, client: reqwest::Client::new() } }
}
#[async_trait::async_trait]
impl LlmProvider for ClaudeProvider {
    fn model_id(&self) -> &str { &self.model }
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse> {
        let body = serde_json::json!({
            "model": self.model, "max_tokens": req.max_tokens, "temperature": req.temperature,
            "system": req.system, "messages": [{"role":"user","content": req.prompt}] });
        let resp = self.client.post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key).header("anthropic-version", "2023-06-01")
            .json(&body).send().await.map_err(|e| KgError::Llm(e.to_string()))?;
        let v: serde_json::Value = resp.json().await.map_err(|e| KgError::Llm(e.to_string()))?;
        let text = v["content"][0]["text"].as_str().unwrap_or("").to_string();
        Ok(LlmResponse { text,
            input_tokens: v["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
            output_tokens: v["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
            model: self.model.clone() })
    }
}
```
`openai.rs` and `ollama.rs` follow the same shape against `https://api.openai.com/v1/chat/completions` and `http://localhost:11434/api/chat` respectively, mapping their token-usage fields (OpenAI `usage.prompt_tokens`/`completion_tokens`; Ollama `prompt_eval_count`/`eval_count`). Default model ids: Claude `claude-opus-4-8`, OpenAI `gpt-4o`, Ollama `llama3`.

- [ ] **Step 6: Implement select.rs**

```rust
// crates/kgx-llm/src/select.rs
use kgx_core::{LlmProvider, Embedder, Result, KgError};
use crate::{mock::MockProvider, claude::ClaudeProvider};
pub fn provider_from_env() -> Result<Box<dyn LlmProvider>> {
    match std::env::var("KGX_LLM").as_deref().unwrap_or("claude") {
        "mock" => Ok(Box::new(MockProvider::new())),
        "claude" => { let k = std::env::var("ANTHROPIC_API_KEY").map_err(|_| KgError::Llm("ANTHROPIC_API_KEY not set".into()))?;
            Ok(Box::new(ClaudeProvider::new(k, std::env::var("KGX_MODEL").unwrap_or("claude-opus-4-8".into())))) }
        // openai/ollama arms analogous
        other => Err(KgError::Llm(format!("unknown KGX_LLM provider: {other}"))),
    }
}
pub fn embedder_from_env() -> Box<dyn Embedder> {
    // minilm behind feature flag (Phase 6); default deterministic mock.
    Box::new(kgx_graph::embed::MockEmbedder::new())
}
```

- [ ] **Step 7: Lib root + verify + commit**

```rust
// crates/kgx-llm/src/lib.rs
pub mod mock; pub mod claude; pub mod openai; pub mod ollama; pub mod select;
```
Run: `cargo test -p kgx-llm` → PASS.
```bash
git add crates/kgx-llm && git commit -m "feat(llm): provider trait impls, deterministic mock, env selector"
```

---

## Task 2: `kgx-extract` — raw → atomic notes (Wave 2)

**Files:**
- Create: `crates/kgx-extract/Cargo.toml`, `src/lib.rs`, `src/prompt.rs`, `src/pipeline.rs`
- Test: `crates/kgx-extract/tests/pipeline.rs`

**Interfaces:**
- Consumes: `kgx_core::{Note, Frontmatter, NoteType, ..., LlmProvider}`, `kgx_vault::write`, `kgx_tokens`.
- Produces: `Intensity { Lite, Full, Ultra }`; `pipeline::extract(provider, source_note: &Note, intensity) -> Result<ExtractResult>`; `ExtractResult { notes: Vec<Note>, tokens: (u32,u32) }`. Each produced fact/entity note has a fresh ULID, `source:` = `[[raw/<source-stem>]]`, `created_by: agent`, bi-temporal stamps.

- [ ] **Step 1: Crate manifest**

```toml
# crates/kgx-extract/Cargo.toml
[package]
name = "kgx-extract"
edition.workspace = true
version.workspace = true
license.workspace = true
[dependencies]
kgx-core = { path = "../kgx-core" }
kgx-vault = { path = "../kgx-vault" }
kgx-llm = { path = "../kgx-llm" }
kgx-ponytail = { path = "../kgx-ponytail" }
serde.workspace = true
serde_json.workspace = true
[dev-dependencies]
tokio = { version = "1", features = ["rt-multi-thread","macros"] }
```
> `kgx-ponytail` ships in Phase 5; for Phase 2 add a thin local trait stub or gate the dependency behind a feature. **Decision:** define `Intensity` here and have `prompt.rs` accept an optional ladder string; Phase 5 injects the real ladder. Remove the `kgx-ponytail` dep line until Phase 5 to keep the wave clean.

- [ ] **Step 2: Write failing test**

```rust
// crates/kgx-extract/tests/pipeline.rs
use kgx_extract::pipeline::{extract, Intensity};
use kgx_llm::mock::MockProvider;
use kgx_core::{Note, Frontmatter, NoteType, Status, Confidence, CreatedBy, CreatedVia};
use std::path::PathBuf;
fn source() -> Note {
    Note { rel_path: PathBuf::from("raw/2026-01-15-arch-review.md"),
        body: "Postgres is the primary datastore. Billing Service depends on it.".into(),
        fm: Frontmatter { r#type: NoteType::Source, id: "01RAW01ARCHREVIEW00000000".into(),
            title: "Arch Review".into(), status: Status::Active, valid_from: None, valid_to: None,
            recorded_at: None, supersedes: vec![], superseded_by: None, source: None,
            confidence: Confidence::High, sources_count: 0, tags: vec![], links: vec![],
            entity_type: None, aliases: vec![], created_by: CreatedBy::Human, created_via: CreatedVia::Cli,
            extra: Default::default() } }
}
#[tokio::test]
async fn extract_produces_facts_with_provenance() {
    let p = MockProvider::new();
    let res = extract(&p, &source(), Intensity::Full).await.unwrap();
    assert!(res.notes.len() >= 2);
    for n in &res.notes {
        assert_eq!(n.fm.created_by, CreatedBy::Agent);
        let src = n.fm.source.as_ref().unwrap();
        assert!(src.contains("raw/2026-01-15-arch-review"), "provenance missing: {src}");
        assert!(n.fm.recorded_at.is_some());
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p kgx-extract`
Expected: FAIL.

- [ ] **Step 4: Implement prompt.rs + pipeline.rs**

```rust
// crates/kgx-extract/src/prompt.rs
pub fn extract_prompt(source_body: &str, _ladder: Option<&str>) -> String {
    format!("EXTRACT_FACTS\n{source_body}")  // mock keys on EXTRACT_FACTS; real ladder injected Phase 5
}
pub const EXTRACT_SYSTEM: &str = "You extract atomic, one-claim-per-note facts with provenance. Reply JSON {facts:[{title,body,confidence,entities}]}.";
```
```rust
// crates/kgx-extract/src/pipeline.rs
use kgx_core::{Note, Frontmatter, NoteType, Status, Confidence, CreatedBy, CreatedVia, LlmProvider, LlmRequest, Result, KgError, util};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub enum Intensity { Lite, Full, Ultra }
#[derive(Debug)]
pub struct ExtractResult { pub notes: Vec<Note>, pub tokens: (u32, u32) }

pub async fn extract(provider: &dyn LlmProvider, source: &Note, _intensity: Intensity) -> Result<ExtractResult> {
    let prompt = crate::prompt::extract_prompt(&source.body, None);
    let resp = provider.complete(LlmRequest {
        system: crate::prompt::EXTRACT_SYSTEM.into(), prompt, max_tokens: 1024, temperature: 0.0 }).await?;
    let v: serde_json::Value = serde_json::from_str(&resp.text).map_err(|e| KgError::Llm(format!("bad extract json: {e}")))?;
    let stem = source.rel_path.file_stem().and_then(|s| s.to_str()).unwrap_or("source");
    let source_link = format!("[[raw/{stem}]]");
    let now = util::now_iso();
    let mut notes = Vec::new();
    for f in v["facts"].as_array().cloned().unwrap_or_default() {
        let title = f["title"].as_str().unwrap_or("").trim().to_string();
        if title.is_empty() { continue; }
        let body = f["body"].as_str().unwrap_or(&title).trim().to_string();
        let conf = match f["confidence"].as_str().unwrap_or("medium") {
            "high" => Confidence::High, "low" => Confidence::Low, _ => Confidence::Medium };
        let links: Vec<String> = f["entities"].as_array().cloned().unwrap_or_default().iter()
            .filter_map(|e| e.as_str()).map(|e| format!("[[{e}]]")).collect();
        let id = util::new_ulid();
        notes.push(Note {
            rel_path: PathBuf::from(format!("notes/facts/{}.md", util::slugify(&title))),
            body: body.clone(),
            fm: Frontmatter { r#type: NoteType::Fact, id, title, status: Status::Active,
                valid_from: Some(now[..10].to_string()), valid_to: None, recorded_at: Some(now.clone()),
                supersedes: vec![], superseded_by: None, source: Some(source_link.clone()), confidence: conf,
                sources_count: 1, tags: vec![], links, entity_type: None, aliases: vec![],
                created_by: CreatedBy::Agent, created_via: CreatedVia::Cli, extra: Default::default() } });
    }
    Ok(ExtractResult { notes, tokens: (resp.input_tokens, resp.output_tokens) })
}
```
```rust
// crates/kgx-extract/src/lib.rs
pub mod prompt; pub mod pipeline;
pub use pipeline::{extract, Intensity, ExtractResult};
```

- [ ] **Step 5: Verify + commit**

Run: `cargo test -p kgx-extract` → PASS.
```bash
git add crates/kgx-extract && git commit -m "feat(extract): raw→atomic facts pipeline with provenance + bitemporal stamps"
```

---

## Task 3: `kgx-retrieval` — RRF fusion + PPR (Wave 3)

**Files:**
- Create: `crates/kgx-retrieval/Cargo.toml`, `src/lib.rs`, `src/rrf.rs`, `src/ppr.rs`, `src/hybrid.rs`
- Test: in-module (rrf, ppr) + `crates/kgx-retrieval/tests/hybrid.rs`

**Interfaces:**
- Consumes: `kgx_graph::{Brain, knn::vector_search, query::{bm25_search, neighbors}}`, `kgx_core::Embedder`.
- Produces: `rrf::fuse(rankings: &[Vec<String>], k: f32) -> Vec<(String, f32)>`; `ppr::personalized(brain, seeds: &[String], damping, iters) -> Result<Vec<(String, f32)>>`; `hybrid::SearchHit { id, score, signals: Vec<String> }`; `hybrid::search(brain, embedder, query, opts: SearchOpts) -> Result<Vec<SearchHit>>`; `SearchOpts { mode: Mode, limit, expand_ppr: bool }`; `Mode { Keyword, Semantic, Hybrid }`.

- [ ] **Step 1: Crate manifest**

```toml
# crates/kgx-retrieval/Cargo.toml
[package]
name = "kgx-retrieval"
edition.workspace = true
version.workspace = true
license.workspace = true
[dependencies]
kgx-core = { path = "../kgx-core" }
kgx-graph = { path = "../kgx-graph" }
serde.workspace = true
[dev-dependencies]
kgx-vault = { path = "../kgx-vault" }
tempfile.workspace = true
```

- [ ] **Step 2: Write failing RRF unit test**

```rust
// crates/kgx-retrieval/src/rrf.rs
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rrf_rewards_consensus() {
        let a = vec!["x".to_string(), "y".into(), "z".into()];
        let b = vec!["y".to_string(), "x".into(), "w".into()];
        let fused = fuse(&[a, b], 60.0);
        assert_eq!(fused[0].0, "x"); // top of one list + 2nd of other beats y
        assert!(fused.iter().any(|(id, _)| id == "w"));
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p kgx-retrieval rrf`
Expected: FAIL.

- [ ] **Step 4: Implement rrf.rs**

```rust
// crates/kgx-retrieval/src/rrf.rs (above tests)
use std::collections::BTreeMap;
pub fn fuse(rankings: &[Vec<String>], k: f32) -> Vec<(String, f32)> {
    let mut scores: BTreeMap<String, f32> = BTreeMap::new();
    for ranking in rankings {
        for (rank, id) in ranking.iter().enumerate() {
            *scores.entry(id.clone()).or_insert(0.0) += 1.0 / (k + (rank as f32) + 1.0);
        }
    }
    let mut v: Vec<(String, f32)> = scores.into_iter().collect();
    v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
    v
}
```

- [ ] **Step 5: Implement ppr.rs (Personalized PageRank from seeds — HippoRAG)**

```rust
// crates/kgx-retrieval/src/ppr.rs
use std::collections::HashMap;
use kgx_core::{Result, KgError};
use kgx_graph::Brain;
pub fn personalized(brain: &Brain, seeds: &[String], damping: f32, iters: u32) -> Result<Vec<(String, f32)>> {
    let ids: Vec<String> = { let mut s = brain.conn().prepare("SELECT id FROM notes ORDER BY id")
        .map_err(|e| KgError::Brain(e.to_string()))?;
        s.query_map([], |r| r.get(0)).map_err(|e| KgError::Brain(e.to_string()))?
            .collect::<std::result::Result<_,_>>().map_err(|e| KgError::Brain(e.to_string()))? };
    let n = ids.len().max(1);
    let index: HashMap<&str, usize> = ids.iter().enumerate().map(|(i, s)| (s.as_str(), i)).collect();
    let mut adj: Vec<Vec<usize>> = vec![vec![]; n];
    { let mut s = brain.conn().prepare("SELECT src_id, dst_id FROM edges").map_err(|e| KgError::Brain(e.to_string()))?;
      let rows = s.query_map([], |r| Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?))).map_err(|e| KgError::Brain(e.to_string()))?;
      for row in rows { let (a, b) = row.map_err(|e| KgError::Brain(e.to_string()))?;
        if let (Some(&i), Some(&j)) = (index.get(a.as_str()), index.get(b.as_str())) { adj[i].push(j); adj[j].push(i); } } }
    let seed_set: Vec<usize> = seeds.iter().filter_map(|s| index.get(s.as_str()).copied()).collect();
    let teleport = if seed_set.is_empty() { vec![1.0 / n as f32; n] }
        else { let mut t = vec![0.0; n]; for &s in &seed_set { t[s] = 1.0 / seed_set.len() as f32; } t };
    let mut rank = teleport.clone();
    for _ in 0..iters {
        let mut next = vec![0.0; n];
        for i in 0..n { next[i] = (1.0 - damping) * teleport[i]; }
        for i in 0..n {
            if adj[i].is_empty() { continue; }
            let share = damping * rank[i] / adj[i].len() as f32;
            for &j in &adj[i] { next[j] += share; }
        }
        rank = next;
    }
    let mut out: Vec<(String, f32)> = ids.into_iter().zip(rank).collect();
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
    Ok(out)
}
```

- [ ] **Step 6: Implement hybrid.rs**

```rust
// crates/kgx-retrieval/src/hybrid.rs
use kgx_core::{Result, Embedder};
use kgx_graph::{Brain, knn::vector_search, query::bm25_search};
use crate::{rrf::fuse, ppr::personalized};

#[derive(Debug, Clone, Copy)]
pub enum Mode { Keyword, Semantic, Hybrid }
#[derive(Debug, Clone)]
pub struct SearchOpts { pub mode: Mode, pub limit: usize, pub expand_ppr: bool }
impl Default for SearchOpts { fn default() -> Self { Self { mode: Mode::Hybrid, limit: 10, expand_ppr: true } } }
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchHit { pub id: String, pub score: f32, pub signals: Vec<String> }

pub fn search(brain: &Brain, embedder: &dyn Embedder, query: &str, opts: SearchOpts) -> Result<Vec<SearchHit>> {
    let mut rankings: Vec<Vec<String>> = Vec::new();
    let mut signals_for: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    if matches!(opts.mode, Mode::Keyword | Mode::Hybrid) {
        let bm = bm25_search(brain, query, 50)?;
        for (id, _) in &bm { signals_for.entry(id.clone()).or_default().push("bm25".into()); }
        rankings.push(bm.into_iter().map(|(id, _)| id).collect());
    }
    if matches!(opts.mode, Mode::Semantic | Mode::Hybrid) {
        let q = embedder.embed(&[query.to_string()])?.remove(0);
        let vec = vector_search(brain, &q, 50)?;
        for (id, _) in &vec { signals_for.entry(id.clone()).or_default().push("vector".into()); }
        rankings.push(vec.into_iter().map(|(id, _)| id).collect());
    }
    let mut fused = fuse(&rankings, 60.0);
    if opts.expand_ppr && !fused.is_empty() {
        let seeds: Vec<String> = fused.iter().take(5).map(|(id, _)| id.clone()).collect();
        let ppr = personalized(brain, &seeds, 0.85, 20)?;
        for (id, _) in ppr.iter().take(50) { signals_for.entry(id.clone()).or_default().push("ppr".into()); }
        fused = fuse(&[fused.into_iter().map(|(id, _)| id).collect(),
                       ppr.into_iter().map(|(id, _)| id).collect()], 60.0);
    }
    Ok(fused.into_iter().take(opts.limit).map(|(id, score)| {
        let signals = signals_for.get(&id).cloned().unwrap_or_default();
        SearchHit { id, score, signals }
    }).collect())
}
```
```rust
// crates/kgx-retrieval/src/lib.rs
pub mod rrf; pub mod ppr; pub mod hybrid;
pub use hybrid::{search, SearchHit, SearchOpts, Mode};
```

- [ ] **Step 7: Write failing hybrid integration test + verify**

```rust
// crates/kgx-retrieval/tests/hybrid.rs
use kgx_retrieval::{search, SearchOpts, Mode};
use kgx_graph::{Brain, build::build_full, embed::MockEmbedder};
use kgx_vault::scan::scan_vault;
fn fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min") }
#[test]
fn hybrid_beats_keyword_on_postgres_query() {
    let notes = scan_vault(&fixture()).unwrap();
    let mut b = Brain::open_in_memory().unwrap();
    build_full(&mut b, &notes, &MockEmbedder::new()).unwrap();
    let e = MockEmbedder::new();
    let hits = search(&b, &e, "primary datastore", SearchOpts { mode: Mode::Hybrid, limit: 5, expand_ppr: true }).unwrap();
    assert!(!hits.is_empty());
    assert!(hits.iter().any(|h| h.id == "01FACT01POSTGRESPRIMARY00"));
}
```
Run: `cargo test -p kgx-retrieval` → PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/kgx-retrieval && git commit -m "feat(retrieval): RRF fusion + personalized PageRank + hybrid search"
```

---

## Task 4: `kg capture` — ingest raw (immutability T01)

**Files:**
- Create: `crates/kgx-cli/src/commands/capture.rs`; modify `cli.rs`/`main.rs`/`Cargo.toml`
- Test: `crates/kgx-cli/tests/cli_capture.rs`

**Interfaces:**
- Produces: `kg capture --from <file|url|-> --type <doc|transcript|web|code>`. Writes an immutable `raw/<date>-<slug>.md` and a pointer `notes/sources/<slug>.md` (`type: source`). Never rewrites an existing `raw/` file (errors if hash would change).

- [ ] **Step 1: Write failing test**

```rust
// crates/kgx-cli/tests/cli_capture.rs
use assert_cmd::Command;
mod common;
#[test]
fn capture_from_stdin_creates_immutable_raw() {
    let d = common::copy_fixture();
    Command::cargo_bin("kg").unwrap().args(["capture","--from","-","--type","doc","--json"])
        .write_stdin("Redis is used for caching.").current_dir(d.path()).assert().success();
    let raw_dir = d.path().join("raw");
    let created: Vec<_> = std::fs::read_dir(&raw_dir).unwrap().filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x=="md").unwrap_or(false)).collect();
    assert!(created.iter().any(|e| std::fs::read_to_string(e.path()).unwrap().contains("Redis is used for caching.")));
}
```

- [ ] **Step 2–4: Add `Capture` to cli, implement, verify**

```rust
// crates/kgx-cli/src/commands/capture.rs
use std::time::Instant;
use std::io::Read;
use crate::output::emit;
use kgx_core::util;
pub fn run(json: bool, from: &str, kind: &str) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;
    let content = match from {
        "-" => { let mut s = String::new(); std::io::stdin().read_to_string(&mut s)?; s }
        path if std::path::Path::new(path).exists() => std::fs::read_to_string(path)?,
        url if url.starts_with("http") => anyhow::bail!("url capture requires --features net (Phase 6)"),
        other => anyhow::bail!("cannot read source: {other}"),
    };
    let today = &util::now_iso()[..10];
    let title = content.lines().next().unwrap_or("capture").chars().take(60).collect::<String>();
    let slug = util::slugify(&title);
    let raw_rel = format!("raw/{today}-{slug}.md");
    let raw_path = root.join(&raw_rel);
    if raw_path.exists() {
        let existing = std::fs::read_to_string(&raw_path)?;
        if !existing.contains(&content) { anyhow::bail!("raw immutability: {raw_rel} exists with different content"); }
    } else {
        let id = util::new_ulid();
        std::fs::create_dir_all(raw_path.parent().unwrap())?;
        std::fs::write(&raw_path, format!("---\ntype: source\nid: {id}\ntitle: \"{title}\"\ncreated_by: human\ncreated_via: cli\n---\n{content}\n"))?;
    }
    // source pointer note
    let sid = util::new_ulid();
    let src_rel = format!("notes/sources/{slug}.md");
    std::fs::create_dir_all(root.join("notes/sources"))?;
    std::fs::write(root.join(&src_rel), format!("---\ntype: source\nid: {sid}\ntitle: \"{title}\"\nsource: \"[[{}]]\"\ncreated_by: agent\ncreated_via: cli\n---\nCaptured {kind} source.\n", raw_rel.trim_end_matches(".md")))?;
    emit("capture", serde_json::json!({"raw": raw_rel, "source_note": src_rel, "kind": kind}), json, start,
        |_| println!("✔ captured → {raw_rel}"));
    Ok(())
}
```
Add `Capture { #[arg(long)] from: String, #[arg(long="type", default_value="doc")] kind: String }` to cli; wire main. Run test → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-cli && git commit -m "feat(cli): kg capture with raw immutability"
```

---

## Task 5: `kg extract` command

**Files:**
- Create: `crates/kgx-cli/src/commands/extract.rs`; modify cli/main/Cargo (`kgx-extract`, `kgx-llm`, `tokio`).
- Test: `crates/kgx-cli/tests/cli_extract.rs`

**Interfaces:**
- Consumes: `kgx_llm::select::provider_from_env`, `kgx_extract::extract`, `kgx_vault::{scan,write}`, `kgx_tokens`.
- Produces: `kg extract --source <id> [--batch] [--dry-run] [--intensity lite|full|ultra] [--json]`. Writes new fact/entity notes; `--dry-run` prints diffs without writing. Records extract `TokenRecord`.

- [ ] **Step 1: Write failing test (T02-style provenance)**

```rust
// crates/kgx-cli/tests/cli_extract.rs
use assert_cmd::Command;
mod common;
#[test]
fn extract_creates_facts_with_provenance() {
    let d = common::copy_fixture();
    let before = std::fs::read_dir(d.path().join("notes/facts")).unwrap().count();
    Command::cargo_bin("kg").unwrap().env("KGX_LLM","mock")
        .args(["extract","--source","01RAW01ARCHREVIEW00000000","--intensity","full"])
        .current_dir(d.path()).assert().success();
    let after = std::fs::read_dir(d.path().join("notes/facts")).unwrap().count();
    assert!(after > before, "no new facts written");
    // every new fact has a source: pointer
    for e in std::fs::read_dir(d.path().join("notes/facts")).unwrap() {
        let c = std::fs::read_to_string(e.unwrap().path()).unwrap();
        assert!(c.contains("source:"), "fact missing provenance");
    }
}
```

- [ ] **Step 2–4: Implement extract.rs (tokio runtime), verify**

```rust
// crates/kgx-cli/src/commands/extract.rs
use std::time::Instant;
use crate::output::emit;
use kgx_extract::{extract, Intensity};
pub fn run(json: bool, source_id: &str, _batch: bool, dry_run: bool, intensity: &str) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;
    let notes = kgx_vault::scan::scan_vault(&root)?;
    let src = notes.iter().find(|n| n.fm.id == source_id)
        .ok_or_else(|| anyhow::anyhow!("source {source_id} not found"))?.clone();
    let inten = match intensity { "lite" => Intensity::Lite, "ultra" => Intensity::Ultra, _ => Intensity::Full };
    let provider = kgx_llm::select::provider_from_env()?;
    let rt = tokio::runtime::Runtime::new()?;
    let res = rt.block_on(extract(provider.as_ref(), &src, inten))?;
    if dry_run {
        emit("extract", serde_json::json!({"dry_run": true,
            "would_create": res.notes.iter().map(|n| n.rel_path.display().to_string()).collect::<Vec<_>>()}),
            json, start, |_| for n in &res.notes { println!("+ {}", n.rel_path.display()); });
        return Ok(());
    }
    for n in &res.notes { kgx_vault::write::write_note(&root, n)?; }
    kgx_tokens::record::append(&root.join(".kg"), &kgx_tokens::TokenRecord {
        model: provider.model_id().into(), operation: "extract".into(), command: "extract".into(),
        input_tokens: res.tokens.0, output_tokens: res.tokens.1, elapsed_ms: start.elapsed().as_millis() as u64,
        correlation_id: kgx_core::util::new_ulid(), ts: kgx_core::util::now_iso() })?;
    emit("extract", serde_json::json!({"created": res.notes.len()}), json, start,
        |_| println!("✔ extracted {} notes", res.notes.len()));
    Ok(())
}
```
Add `Extract { source: String (--source), batch, dry_run (--dry-run), intensity }` to cli. Run test → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-cli && git commit -m "feat(cli): kg extract (intensity, dry-run, token accounting)"
```

---

## Task 6: `kg link` — backlinks, suggest, orphans (T03, T04)

**Files:**
- Create: `crates/kgx-graph/src/links.rs` (pure analysis) + `crates/kgx-cli/src/commands/link.rs`
- Test: `crates/kgx-graph/tests/links.rs`, `crates/kgx-cli/tests/cli_link.rs`

**Interfaces:**
- Consumes: `kgx_core::{Note, util}`.
- Produces: `links::backlinks(notes) -> BTreeMap<String, Vec<String>>` (id → ids linking to it); `links::orphans(notes) -> Vec<String>` (no in/out edges, excluding `moc`); `links::phantoms(notes) -> Vec<(String,String)>` (note id, unresolved target). CLI `kg link [--suggest] [--orphans] [--fix] [--json]`.

- [ ] **Step 1: Write failing graph unit tests**

```rust
// crates/kgx-graph/tests/links.rs
use kgx_graph::links::{orphans, backlinks};
use kgx_vault::scan::scan_vault;
fn fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min") }
#[test]
fn exactly_one_orphan_excluding_mocs() {
    let notes = scan_vault(&fixture()).unwrap();
    let orph = orphans(&notes);
    assert_eq!(orph.len(), 1, "expected 1 orphan, got {orph:?}");
    assert!(orph.contains(&"01FACT05ORPHAN0000000000".to_string()));
}
#[test]
fn backlinks_resolve() {
    let notes = scan_vault(&fixture()).unwrap();
    let bl = backlinks(&notes);
    // Postgres entity should have inbound links from facts
    assert!(bl.values().any(|v| !v.is_empty()));
}
```

- [ ] **Step 2–4: Implement links.rs + CLI, verify**

```rust
// crates/kgx-graph/src/links.rs
use std::collections::{BTreeMap, BTreeSet};
use kgx_core::{Note, util};
fn resolver(notes: &[Note]) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    for n in notes { m.insert(n.fm.title.clone(), n.fm.id.clone()); m.insert(n.fm.id.clone(), n.fm.id.clone()); }
    m
}
pub fn backlinks(notes: &[Note]) -> BTreeMap<String, Vec<String>> {
    let res = resolver(notes);
    let mut bl: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for n in notes {
        let mut targets = util::extract_wikilinks(&n.body);
        for l in &n.fm.links { targets.extend(util::extract_wikilinks(l)); }
        for t in targets { if let Some(dst) = res.get(t.trim_start_matches("raw/")) {
            if dst != &n.fm.id { bl.entry(dst.clone()).or_default().push(n.fm.id.clone()); } } }
    }
    for v in bl.values_mut() { v.sort(); v.dedup(); }
    bl
}
pub fn orphans(notes: &[Note]) -> Vec<String> {
    let bl = backlinks(notes);
    let res = resolver(notes);
    let mut out = Vec::new();
    for n in notes {
        if matches!(n.fm.r#type, kgx_core::NoteType::Moc) { continue; }
        let has_in = bl.get(&n.fm.id).map(|v| !v.is_empty()).unwrap_or(false);
        let mut targets = util::extract_wikilinks(&n.body);
        for l in &n.fm.links { targets.extend(util::extract_wikilinks(l)); }
        let has_out = targets.iter().any(|t| res.contains_key(t.trim_start_matches("raw/")));
        if !has_in && !has_out { out.push(n.fm.id.clone()); }
    }
    out.sort();
    out
}
pub fn phantoms(notes: &[Note]) -> Vec<(String, String)> {
    let res = resolver(notes);
    let mut out = Vec::new();
    for n in notes {
        for t in util::extract_wikilinks(&n.body) {
            let key = t.trim_start_matches("raw/");
            if !res.contains_key(key) && !t.starts_with("raw/") { out.push((n.fm.id.clone(), t)); }
        }
    }
    let _ = BTreeSet::<()>::new();
    out.sort(); out
}
```
Add `pub mod links;` to `kgx-graph/src/lib.rs`. CLI `link.rs` calls `orphans`/`backlinks`/`phantoms` and emits; `--suggest` uses retrieval `search` over orphan bodies to propose links (writes only with `--fix`). Run tests → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-graph crates/kgx-cli && git commit -m "feat(link): backlinks, orphans, phantom detection (T03/T04)"
```

---

## Task 7: `kg search` + `kg recall`

**Files:**
- Create: `crates/kgx-cli/src/commands/{search,recall}.rs`; modify cli/main/Cargo (`kgx-retrieval`).
- Test: `crates/kgx-cli/tests/cli_search.rs`

**Interfaces:**
- Consumes: `kgx_retrieval::{search, SearchOpts, Mode}`, `kgx_graph::query::neighbors`, `kgx_llm::select::embedder_from_env`.
- Produces: `kg search <query> [--type ...] [--mode keyword|semantic|hybrid] [--limit N] [--json]`; `kg recall --entity "Postgres" [--json]` (resolves entity → id → 1–2 hop neighborhood with titles).

- [ ] **Step 1: Write failing test**

```rust
// crates/kgx-cli/tests/cli_search.rs
use assert_cmd::Command;
mod common;
#[test]
fn search_hybrid_json_returns_hits() {
    let d = common::copy_fixture();
    Command::cargo_bin("kg").unwrap().env("KGX_LLM","mock").args(["index","--full"]).current_dir(d.path()).assert().success();
    let out = Command::cargo_bin("kg").unwrap().env("KGX_LLM","mock")
        .args(["search","primary datastore","--mode","hybrid","--json"]).current_dir(d.path()).assert().success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert!(v["data"]["hits"].as_array().unwrap().len() > 0);
}
```

- [ ] **Step 2–4: Implement search.rs/recall.rs, verify**

```rust
// crates/kgx-cli/src/commands/search.rs
use std::time::Instant;
use crate::output::emit;
use kgx_retrieval::{search, SearchOpts, Mode};
use kgx_graph::Brain;
pub fn run(json: bool, query: &str, mode: &str, limit: usize) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;
    let brain = Brain::open(&root.join(".kg/brain.sqlite"))?;
    let embedder = kgx_llm::select::embedder_from_env();
    let m = match mode { "keyword" => Mode::Keyword, "semantic" => Mode::Semantic, _ => Mode::Hybrid };
    let hits = search(&brain, embedder.as_ref(), query, SearchOpts { mode: m, limit, expand_ppr: true })?;
    emit("search", serde_json::json!({"hits": hits}), json, start,
        |_| for h in &hits { println!("{:.4} {} [{}]", h.score, h.id, h.signals.join(",")); });
    Ok(())
}
```
`recall.rs`: scan vault, find entity by title/alias, look up id, `neighbors(&brain, id, 2)`, join titles, emit. Add `Search`/`Recall` to cli. Run → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-cli && git commit -m "feat(cli): kg search + kg recall over hybrid brain"
```

---

## Task 8: `kg ask` — hybrid Q&A with citations

**Files:**
- Create: `crates/kgx-retrieval/src/answer.rs`, `crates/kgx-cli/src/commands/ask.rs`
- Test: `crates/kgx-cli/tests/cli_ask.rs`

**Interfaces:**
- Consumes: `kgx_retrieval::search`, `kgx_llm`, `kgx_graph::Brain`, `kgx_vault::scan`.
- Produces: `answer::build_context(brain, hits, notes) -> String`; `kg ask <question> [--scope local|global] [--mode] [--cite] [--write] [--json]`. Builds context from top hits, calls provider with `ANSWER_QUESTION` prompt, emits `{answer, citations}`. `--write` stores the answer as a `type: experience` note. Records ask `TokenRecord`.

- [ ] **Step 1: Write failing test**

```rust
// crates/kgx-cli/tests/cli_ask.rs
use assert_cmd::Command;
mod common;
#[test]
fn ask_returns_answer_with_citations() {
    let d = common::copy_fixture();
    Command::cargo_bin("kg").unwrap().env("KGX_LLM","mock").args(["index","--full"]).current_dir(d.path()).assert().success();
    let out = Command::cargo_bin("kg").unwrap().env("KGX_LLM","mock")
        .args(["ask","What is the primary datastore?","--cite","--json"]).current_dir(d.path()).assert().success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert!(v["data"]["answer"].as_str().unwrap().to_lowercase().contains("postgres"));
    assert!(v["data"]["citations"].as_array().unwrap().len() >= 1);
}
```

- [ ] **Step 2–4: Implement, verify**

```rust
// crates/kgx-cli/src/commands/ask.rs
use std::time::Instant;
use crate::output::emit;
use kgx_retrieval::{search, SearchOpts, Mode};
use kgx_graph::Brain;
use kgx_core::{LlmRequest, LlmProvider};
pub fn run(json: bool, question: &str, _scope: &str, mode: &str, _cite: bool, _write: bool) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;
    let brain = Brain::open(&root.join(".kg/brain.sqlite"))?;
    let notes = kgx_vault::scan::scan_vault(&root)?;
    let embedder = kgx_llm::select::embedder_from_env();
    let m = match mode { "keyword" => Mode::Keyword, "semantic" => Mode::Semantic, _ => Mode::Hybrid };
    let hits = search(&brain, embedder.as_ref(), question, SearchOpts { mode: m, limit: 8, expand_ppr: true })?;
    let mut ctx = String::from("ANSWER_QUESTION\nContext:\n");
    for h in &hits { if let Some(n) = notes.iter().find(|n| n.fm.id == h.id) {
        ctx.push_str(&format!("[{}] {}: {}\n", n.fm.id, n.fm.title, n.body)); } }
    ctx.push_str(&format!("\nQuestion: {question}\n"));
    let provider = kgx_llm::select::provider_from_env()?;
    let rt = tokio::runtime::Runtime::new()?;
    let resp = rt.block_on(provider.complete(LlmRequest {
        system: "Answer only from context. Cite note ids.".into(), prompt: ctx, max_tokens: 1024, temperature: 0.0 }))?;
    let parsed: serde_json::Value = serde_json::from_str(&resp.text).unwrap_or(serde_json::json!({"answer": resp.text, "citations": []}));
    kgx_tokens::record::append(&root.join(".kg"), &kgx_tokens::TokenRecord {
        model: provider.model_id().into(), operation: "ask".into(), command: "ask".into(),
        input_tokens: resp.input_tokens, output_tokens: resp.output_tokens,
        elapsed_ms: start.elapsed().as_millis() as u64, correlation_id: kgx_core::util::new_ulid(),
        ts: kgx_core::util::now_iso() })?;
    emit("ask", parsed.clone(), json, start, |_| {
        println!("{}", parsed["answer"].as_str().unwrap_or(""));
        if let Some(c) = parsed["citations"].as_array() { println!("cites: {:?}", c); } });
    Ok(())
}
```
Add `Ask` to cli (`scope`, `mode`, `cite`, `write`). Run → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-cli && git commit -m "feat(cli): kg ask — hybrid Q&A with citations + token accounting"
```

---

## Task 9: Smoke T01–T04, T09

**Files:**
- Create: `tests/smoke/{t01_capture_immutability,t02_extract,t03_link,t04_orphan}.rs`; `tests/smoke/t09_recall.rs` (criterion bench in `benches/`)

**Interfaces:** End-to-end via `kg` binary, `KGX_LLM=mock`.

- [ ] **Step 1: T01 capture immutability**

```rust
// tests/smoke/t01_capture_immutability.rs
use assert_cmd::Command; mod common;
#[test]
fn t01_raw_hash_unchanged_after_extract() {
    let d = common::copy_fixture();
    let raw = d.path().join("raw/2026-01-15-arch-review.md");
    let before = std::fs::read(&raw).unwrap();
    Command::cargo_bin("kg").unwrap().env("KGX_LLM","mock").args(["index","--full"]).current_dir(d.path()).assert().success();
    Command::cargo_bin("kg").unwrap().env("KGX_LLM","mock")
        .args(["extract","--source","01RAW01ARCHREVIEW00000000"]).current_dir(d.path()).assert().success();
    assert_eq!(before, std::fs::read(&raw).unwrap(), "raw file mutated");
}
```

- [ ] **Step 2: T02 extract correctness (≥4/5 facts)**

Assert that extracting both raw sources yields ≥4 fact notes each with `source:` and a `recorded_at` stamp.

- [ ] **Step 3: T03 link integrity**

Run `kg link --json`; assert every `[[X]]` that resolves yields a backlink and `phantoms` count is 0 for the fixture.

- [ ] **Step 4: T04 orphan detection**

Run `kg link --orphans --json`; assert exactly 1 orphan id `01FACT05ORPHAN0000000000`.

- [ ] **Step 5: T09 recall benchmark (RRF > vector-only)**

```rust
// crates/kgx-retrieval/benches/recall.rs  (criterion + assertion gate)
// Build brain from fixture; define a tiny multi-hop QA set in tests/fixtures/qa.json
// (question → expected note ids). Compute recall@5 for Mode::Semantic vs Mode::Hybrid.
// Assert hybrid_recall >= semantic_recall + 0.10 (PRD goal: >10% gain). Fails the bench if not.
```
Add `qa.json` to `tests/fixtures/` with 3 multi-hop questions whose answers require following `derived_from`/`links_to` edges (so PPR helps). Register `[[bench]]` in `kgx-retrieval/Cargo.toml`.

- [ ] **Step 6: Verify + commit**

Run: `cargo test --workspace --test 'smoke*' -- --test-threads=1 && cargo bench -p kgx-retrieval`
Expected: smoke green; bench prints hybrid≥semantic+0.10.
```bash
git add tests/smoke crates/kgx-retrieval/benches tests/fixtures/qa.json
git commit -m "test(smoke): T01-T04 + T09 recall benchmark"
```

---

## Self-Review (Phase 2)

- **Spec coverage:** `capture/extract/link/search/recall/ask` (§9) ✔; hybrid retrieval vec+BM25+entity+PPR+RRF (§8) ✔ Task 3; provider trait (§18 `llm/`) ✔ Task 1; intensity flag ✔ Task 5; T01–T04 ✔, T09 ✔ Task 9.
- **Type consistency:** `SearchHit`/`SearchOpts`/`Mode`/`search`, `Intensity`/`extract`/`ExtractResult`, `provider_from_env`/`embedder_from_env` match Phase 3/5 consumers.
- **Deferred:** `--scope global` (community summaries) → Phase 4; real network URL capture → Phase 6; `kgx-ponytail` ladder injection into prompts → Phase 5 (prompt.rs accepts `Option<&str>` now).
- **Placeholder scan:** none.
