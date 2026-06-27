# KGX Phase 1 — Brain Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development or superpowers:executing-plans. Read `2026-06-27-kgx-master-plan.md` (contracts §3, waves, Global Constraints) and complete Phase 0 first. Steps use `- [ ]`.

**Goal:** Build the derived SQLite "brain" (`.kg/brain.sqlite`): schema, deterministic `kg index --full`, `--incremental`, FTS5/BM25, local embeddings + vector KNN; plus `kgx-tokens` accounting. Proves the rebuild contract (T10) and token accounting (T16).

**Architecture:** Wave 1 adds `kgx-tokens` (depends only on core). Wave 2 adds `kgx-graph` (depends on core + vault). `kg index` (a Wave-5 CLI command, but buildable now since its deps exist) wires vault scan → graph build. Embeddings run locally via `candle` behind the `kgx_core::Embedder` trait, with a deterministic `MockEmbedder` for tests.

**Tech Stack:** `rusqlite` (bundled SQLite + FTS5), `sqlite-vec`, `candle-core`/`candle-transformers` + `tokenizers` (MiniLM), `serde_json`, `criterion`.

## Global Constraints

Inherit master Global Constraints. Phase-critical: determinism (sort before insert; `T10`); embeddings are 384-dim little-endian `f32` BLOBs; `.kg/` is git-ignored and fully rebuildable; every LLM/embed op records tokens via `kgx-tokens`.

---

## Task 1: `kgx-tokens` — metrics types + JSONL persistence (Wave 1)

**Files:**
- Create: `crates/kgx-tokens/Cargo.toml`, `crates/kgx-tokens/src/lib.rs`, `crates/kgx-tokens/src/record.rs`, `crates/kgx-tokens/src/aggregate.rs`
- Test: in-module + `crates/kgx-tokens/tests/persist.rs`

**Interfaces:**
- Consumes: `kgx_core::Result`.
- Produces: `TokenRecord { model, operation, command, input_tokens, output_tokens, elapsed_ms, correlation_id, ts }`; `record::append(kg_dir: &Path, &TokenRecord) -> Result<()>` (appends one JSONL line to `metrics.log`); `aggregate::summarize(kg_dir, since_days, group_by) -> Result<Vec<TokenAgg>>`; `TokenAgg { key, input_tokens, output_tokens, count }`; `GroupBy { Operation, Command, Day }`.

- [ ] **Step 1: Crate manifest**

```toml
# crates/kgx-tokens/Cargo.toml
[package]
name = "kgx-tokens"
edition.workspace = true
version.workspace = true
license.workspace = true
[dependencies]
kgx-core = { path = "../kgx-core" }
serde.workspace = true
serde_json.workspace = true
[dev-dependencies]
tempfile.workspace = true
```
Append to workspace members.

- [ ] **Step 2: Write failing tests**

```rust
// crates/kgx-tokens/tests/persist.rs
use kgx_tokens::record::{append, TokenRecord};
use kgx_tokens::aggregate::{summarize, GroupBy};
fn rec(op: &str, i: u32, o: u32) -> TokenRecord {
    TokenRecord { model: "mock".into(), operation: op.into(), command: "index".into(),
        input_tokens: i, output_tokens: o, elapsed_ms: 5, correlation_id: "c1".into(),
        ts: "2026-06-27T10:00:00Z".into() }
}
#[test]
fn append_then_aggregate_by_operation() {
    let d = tempfile::tempdir().unwrap();
    append(d.path(), &rec("embed", 100, 0)).unwrap();
    append(d.path(), &rec("embed", 50, 0)).unwrap();
    append(d.path(), &rec("extract", 200, 80)).unwrap();
    let mut aggs = summarize(d.path(), 30, GroupBy::Operation).unwrap();
    aggs.sort_by(|a, b| a.key.cmp(&b.key));
    assert_eq!(aggs.len(), 2);
    let embed = aggs.iter().find(|a| a.key == "embed").unwrap();
    assert_eq!(embed.input_tokens, 150);
    assert_eq!(embed.count, 2);
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p kgx-tokens`
Expected: FAIL.

- [ ] **Step 4: Implement record.rs**

```rust
// crates/kgx-tokens/src/record.rs
use std::path::Path;
use std::io::Write;
use kgx_core::{Result, KgError};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenRecord {
    pub model: String, pub operation: String, pub command: String,
    pub input_tokens: u32, pub output_tokens: u32, pub elapsed_ms: u64,
    pub correlation_id: String, pub ts: String,
}
pub fn append(kg_dir: &Path, r: &TokenRecord) -> Result<()> {
    std::fs::create_dir_all(kg_dir).map_err(|e| KgError::Io { path: kg_dir.display().to_string(), source: e })?;
    let path = kg_dir.join("metrics.log");
    let line = serde_json::to_string(r).map_err(|e| KgError::Other(e.to_string()))?;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path)
        .map_err(|e| KgError::Io { path: path.display().to_string(), source: e })?;
    writeln!(f, "{line}").map_err(|e| KgError::Io { path: path.display().to_string(), source: e })?;
    Ok(())
}
```

- [ ] **Step 5: Implement aggregate.rs**

```rust
// crates/kgx-tokens/src/aggregate.rs
use std::path::Path;
use std::collections::BTreeMap;
use kgx_core::{Result, KgError};
use crate::record::TokenRecord;

#[derive(Debug, Clone, Copy)]
pub enum GroupBy { Operation, Command, Day }
#[derive(Debug, Clone, serde::Serialize)]
pub struct TokenAgg { pub key: String, pub input_tokens: u32, pub output_tokens: u32, pub count: u32 }

pub fn summarize(kg_dir: &Path, _since_days: u32, group: GroupBy) -> Result<Vec<TokenAgg>> {
    let path = kg_dir.join("metrics.log");
    if !path.exists() { return Ok(vec![]); }
    let text = std::fs::read_to_string(&path).map_err(|e| KgError::Io { path: path.display().to_string(), source: e })?;
    let mut map: BTreeMap<String, TokenAgg> = BTreeMap::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let r: TokenRecord = serde_json::from_str(line).map_err(|e| KgError::Other(e.to_string()))?;
        let key = match group { GroupBy::Operation => r.operation.clone(),
            GroupBy::Command => r.command.clone(), GroupBy::Day => r.ts.chars().take(10).collect() };
        let e = map.entry(key.clone()).or_insert(TokenAgg { key, input_tokens: 0, output_tokens: 0, count: 0 });
        e.input_tokens += r.input_tokens; e.output_tokens += r.output_tokens; e.count += 1;
    }
    Ok(map.into_values().collect())
}
```
Note: `_since_days` filtering refined in Phase 6 `kg tokens --since`; the parameter exists now so the signature is stable.

- [ ] **Step 6: Lib root + verify + commit**

```rust
// crates/kgx-tokens/src/lib.rs
pub mod record; pub mod aggregate;
pub use record::{append, TokenRecord};
pub use aggregate::{summarize, GroupBy, TokenAgg};
```
Run: `cargo test -p kgx-tokens` → PASS.
```bash
git add crates/kgx-tokens && git commit -m "feat(tokens): JSONL token records + aggregation"
```

---

## Task 2: `kgx-graph` — schema + open/migrate (Wave 2)

**Files:**
- Create: `crates/kgx-graph/Cargo.toml`, `crates/kgx-graph/src/lib.rs`, `crates/kgx-graph/src/schema.rs`, `crates/kgx-graph/src/brain.rs`
- Test: `crates/kgx-graph/tests/schema.rs`

**Interfaces:**
- Consumes: `kgx_core::{Note, Edge, RelType, Result}`.
- Produces: `Brain` (wraps `rusqlite::Connection`); `Brain::open(path: &Path) -> Result<Brain>` (creates schema if absent), `Brain::open_in_memory() -> Result<Brain>`; the SQL schema from PRD §7 (notes, edges, notes_fts, pagerank, communities) plus a `meta(key,value)` table.

- [ ] **Step 1: Crate manifest**

```toml
# crates/kgx-graph/Cargo.toml
[package]
name = "kgx-graph"
edition.workspace = true
version.workspace = true
license.workspace = true
[dependencies]
kgx-core = { path = "../kgx-core" }
kgx-vault = { path = "../kgx-vault" }
rusqlite = { version = "0.31", features = ["bundled", "blob"] }
sqlite-vec = "0.1"
serde.workspace = true
serde_json.workspace = true
[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 2: Write failing schema test**

```rust
// crates/kgx-graph/tests/schema.rs
use kgx_graph::brain::Brain;
#[test]
fn open_creates_all_tables() {
    let b = Brain::open_in_memory().unwrap();
    let tables: Vec<String> = b.conn().prepare("SELECT name FROM sqlite_master WHERE type IN ('table','view') ORDER BY name")
        .unwrap().query_map([], |r| r.get(0)).unwrap().collect::<Result<_,_>>().unwrap();
    for expected in ["communities", "edges", "meta", "notes", "notes_fts", "pagerank"] {
        assert!(tables.iter().any(|t| t == expected), "missing table {expected}; have {tables:?}");
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p kgx-graph --test schema`
Expected: FAIL.

- [ ] **Step 4: Implement schema.rs**

```rust
// crates/kgx-graph/src/schema.rs
pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS notes (
  id TEXT PRIMARY KEY, path TEXT NOT NULL, type TEXT NOT NULL, status TEXT NOT NULL,
  valid_from TEXT, valid_to TEXT, recorded_at TEXT, tags TEXT, raw_text TEXT, embedding BLOB);
CREATE TABLE IF NOT EXISTS edges (
  src_id TEXT NOT NULL, dst_id TEXT NOT NULL, rel_type TEXT NOT NULL,
  valid_from TEXT, valid_to TEXT, PRIMARY KEY (src_id, dst_id, rel_type));
CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(id, raw_text, tags, content='', tokenize='porter');
CREATE TABLE IF NOT EXISTS pagerank (id TEXT PRIMARY KEY, score REAL);
CREATE TABLE IF NOT EXISTS communities (id TEXT, community_id INTEGER, PRIMARY KEY (id, community_id));
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT);
CREATE INDEX IF NOT EXISTS idx_edges_src ON edges(src_id);
CREATE INDEX IF NOT EXISTS idx_edges_dst ON edges(dst_id);
CREATE INDEX IF NOT EXISTS idx_notes_type ON notes(type);
"#;
```

- [ ] **Step 5: Implement brain.rs (open + vec extension load)**

```rust
// crates/kgx-graph/src/brain.rs
use std::path::Path;
use rusqlite::Connection;
use kgx_core::{Result, KgError};
use crate::schema::SCHEMA;

pub struct Brain { conn: Connection }
impl Brain {
    pub fn open(path: &Path) -> Result<Brain> {
        if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
        let conn = Connection::open(path).map_err(|e| KgError::Brain(e.to_string()))?;
        Self::init(conn)
    }
    pub fn open_in_memory() -> Result<Brain> {
        let conn = Connection::open_in_memory().map_err(|e| KgError::Brain(e.to_string()))?;
        Self::init(conn)
    }
    fn init(conn: Connection) -> Result<Brain> {
        unsafe { // load sqlite-vec extension for vec0 virtual tables / vec ops
            sqlite_vec::sqlite3_vec_init();
        }
        conn.execute_batch(SCHEMA).map_err(|e| KgError::Brain(e.to_string()))?;
        Ok(Brain { conn })
    }
    pub fn conn(&self) -> &Connection { &self.conn }
    pub fn conn_mut(&mut self) -> &mut Connection { &mut self.conn }
}
```
> If the `sqlite-vec` crate API differs, load via `conn.load_extension` with the auto-init entrypoint; the loading call is the only line that changes. KNN (Task 5) uses a brute-force cosine fallback so vec0 is optional for correctness.

- [ ] **Step 6: Lib root + verify + commit**

```rust
// crates/kgx-graph/src/lib.rs
pub mod schema; pub mod brain; pub mod build; pub mod embed; pub mod knn; pub mod query;
pub use brain::Brain;
```
Stub `build`, `embed`, `knn`, `query` with `// next task`. Run: `cargo test -p kgx-graph --test schema` → PASS.
```bash
git add crates/kgx-graph && git commit -m "feat(graph): brain schema + connection open"
```

---

## Task 3: `kgx-graph` — embedder (MiniLM + deterministic mock)

**Files:**
- Modify: `crates/kgx-graph/src/embed.rs`
- Test: in-module

**Interfaces:**
- Consumes: `kgx_core::Embedder`.
- Produces: `MockEmbedder` (deterministic, hash-seeded, 384-dim — used by all tests/CI); `MiniLmEmbedder::load() -> Result<MiniLmEmbedder>` (real candle model); `f32_to_blob(&[f32]) -> Vec<u8>` and `blob_to_f32(&[u8]) -> Vec<f32>` helpers.

- [ ] **Step 1: Write failing tests**

```rust
// crates/kgx-graph/src/embed.rs
#[cfg(test)]
mod tests {
    use super::*;
    use kgx_core::Embedder;
    #[test]
    fn mock_is_deterministic_384() {
        let e = MockEmbedder::new();
        let a = e.embed(&["hello world".into()]).unwrap();
        let b = e.embed(&["hello world".into()]).unwrap();
        assert_eq!(e.dim(), 384);
        assert_eq!(a[0].len(), 384);
        assert_eq!(a, b);
        let c = e.embed(&["different".into()]).unwrap();
        assert_ne!(a[0], c[0]);
    }
    #[test]
    fn blob_roundtrip() {
        let v = vec![1.0f32, -2.5, 3.25];
        assert_eq!(blob_to_f32(&f32_to_blob(&v)), v);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p kgx-graph embed`
Expected: FAIL.

- [ ] **Step 3: Implement embed.rs**

```rust
// crates/kgx-graph/src/embed.rs (above tests)
use kgx_core::{Embedder, Result};

pub fn f32_to_blob(v: &[f32]) -> Vec<u8> { v.iter().flat_map(|f| f.to_le_bytes()).collect() }
pub fn blob_to_f32(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

/// Deterministic, network-free embedder for tests/CI. Hashes tokens into a 384-dim bag, L2-normalized.
pub struct MockEmbedder;
impl MockEmbedder { pub fn new() -> Self { MockEmbedder } }
impl Default for MockEmbedder { fn default() -> Self { Self::new() } }
impl Embedder for MockEmbedder {
    fn dim(&self) -> usize { 384 }
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|t| {
            let mut v = vec![0f32; 384];
            for word in t.split_whitespace() {
                let h = word.bytes().fold(1469598103934665603u64, |a, b| (a ^ b as u64).wrapping_mul(1099511628211));
                v[(h % 384) as usize] += 1.0;
            }
            let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
            for x in &mut v { *x /= norm; }
            v
        }).collect())
    }
}
```
> Real `MiniLmEmbedder` (candle) is added behind a `cfg(feature = "candle")` flag in Phase 6 polish (model download + tokenizer); the `Embedder` trait keeps call sites unchanged. CI always uses `MockEmbedder` (Global Constraint: no network in tests).

- [ ] **Step 4: Verify + commit**

Run: `cargo test -p kgx-graph embed` → PASS.
```bash
git add crates/kgx-graph/src/embed.rs && git commit -m "feat(graph): deterministic mock embedder + blob codecs"
```

---

## Task 4: `kgx-graph` — full build (notes, edges, FTS, embeddings)

**Files:**
- Modify: `crates/kgx-graph/src/build.rs`
- Test: `crates/kgx-graph/tests/build.rs`

**Interfaces:**
- Consumes: `Brain`, `kgx_core::{Note, Edge, RelType}`, `Embedder`, `kgx_vault::scan::scan_vault`, `kgx_core::util::extract_wikilinks`.
- Produces: `build::build_full(brain: &mut Brain, notes: &[Note], embedder: &dyn Embedder) -> Result<BuildStats>`; `BuildStats { nodes, edges, embedded }`; `build::derive_edges(notes: &[Note]) -> Vec<Edge>` (pure, from `links` + body wikilinks + `supersedes` + `source`).

- [ ] **Step 1: Write failing tests**

```rust
// crates/kgx-graph/tests/build.rs
use kgx_graph::{Brain, build::{build_full, derive_edges}, embed::MockEmbedder};
use kgx_vault::scan::scan_vault;
fn fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min")
}
#[test]
fn build_full_populates_nodes_and_edges_deterministically() {
    let notes = scan_vault(&fixture()).unwrap();
    let mut b1 = Brain::open_in_memory().unwrap();
    let mut b2 = Brain::open_in_memory().unwrap();
    let s1 = build_full(&mut b1, &notes, &MockEmbedder::new()).unwrap();
    let s2 = build_full(&mut b2, &notes, &MockEmbedder::new()).unwrap();
    assert_eq!(s1.nodes, s2.nodes);
    assert_eq!(s1.edges, s2.edges);
    assert_eq!(s1.nodes, notes.len());
    assert!(s1.edges > 0);
    // FTS populated
    let cnt: i64 = b1.conn().query_row("SELECT count(*) FROM notes_fts", [], |r| r.get(0)).unwrap();
    assert_eq!(cnt as usize, notes.len());
}
#[test]
fn derive_edges_includes_supersedes_and_source() {
    let notes = scan_vault(&fixture()).unwrap();
    let edges = derive_edges(&notes);
    assert!(edges.iter().any(|e| matches!(e.rel_type, kgx_core::RelType::DerivedFrom)));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p kgx-graph --test build`
Expected: FAIL.

- [ ] **Step 3: Implement build.rs**

```rust
// crates/kgx-graph/src/build.rs
use rusqlite::params;
use kgx_core::{Note, Edge, RelType, Result, KgError, Embedder, util};
use crate::{Brain, embed::f32_to_blob};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BuildStats { pub nodes: usize, pub edges: usize, pub embedded: usize }

/// Pure edge derivation: title-index resolves wikilinks → dst ids.
pub fn derive_edges(notes: &[Note]) -> Vec<Edge> {
    use std::collections::BTreeMap;
    let by_title: BTreeMap<&str, &str> = notes.iter().map(|n| (n.fm.title.as_str(), n.fm.id.as_str())).collect();
    let by_id: std::collections::BTreeSet<&str> = notes.iter().map(|n| n.fm.id.as_str()).collect();
    let resolve = |target: &str| -> Option<String> {
        let t = target.trim_start_matches("raw/");
        if let Some(id) = by_title.get(t) { return Some(id.to_string()); }
        if by_id.contains(t) { return Some(t.to_string()); }
        None
    };
    let mut edges = Vec::new();
    for n in notes {
        // wikilinks (body + explicit links field) → links_to / mentions_entity
        let mut targets = util::extract_wikilinks(&n.body);
        for l in &n.fm.links { targets.extend(util::extract_wikilinks(l)); }
        for t in targets {
            if let Some(dst) = resolve(&t) {
                if dst != n.fm.id {
                    edges.push(Edge { src_id: n.fm.id.clone(), dst_id: dst, rel_type: RelType::LinksTo,
                        valid_from: n.fm.valid_from.clone(), valid_to: n.fm.valid_to.clone() });
                }
            }
        }
        // supersedes
        for s in &n.fm.supersedes {
            if let Some(dst) = resolve(s) {
                edges.push(Edge { src_id: n.fm.id.clone(), dst_id: dst, rel_type: RelType::Supersedes,
                    valid_from: n.fm.valid_from.clone(), valid_to: n.fm.valid_to.clone() }); }
        }
        // source → derived_from
        if let Some(src) = &n.fm.source {
            for t in util::extract_wikilinks(src) {
                if let Some(dst) = resolve(&t) {
                    edges.push(Edge { src_id: n.fm.id.clone(), dst_id: dst, rel_type: RelType::DerivedFrom,
                        valid_from: None, valid_to: None }); }
            }
        }
    }
    edges.sort_by(|a, b| (a.src_id.clone(), a.dst_id.clone(), format!("{:?}", a.rel_type))
        .cmp(&(b.src_id.clone(), b.dst_id.clone(), format!("{:?}", b.rel_type))));
    edges.dedup();
    edges
}

pub fn build_full(brain: &mut Brain, notes: &[Note], embedder: &dyn Embedder) -> Result<BuildStats> {
    let tx = brain.conn_mut().transaction().map_err(|e| KgError::Brain(e.to_string()))?;
    tx.execute_batch("DELETE FROM notes; DELETE FROM edges; DELETE FROM notes_fts;")
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let texts: Vec<String> = notes.iter().map(|n| format!("{}\n{}", n.fm.title, n.body)).collect();
    let embeddings = embedder.embed(&texts)?;
    for (n, emb) in notes.iter().zip(&embeddings) {
        let tags = serde_json::to_string(&{ let mut t = n.fm.tags.clone(); t.sort(); t }).unwrap();
        let typ = serde_json::to_string(&n.fm.r#type).unwrap().trim_matches('"').to_string();
        let st = serde_json::to_string(&n.fm.status).unwrap().trim_matches('"').to_string();
        tx.execute("INSERT INTO notes (id,path,type,status,valid_from,valid_to,recorded_at,tags,raw_text,embedding)\
                    VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![n.fm.id, n.rel_path.display().to_string(), typ, st, n.fm.valid_from, n.fm.valid_to,
                n.fm.recorded_at, tags, n.body, f32_to_blob(emb)]).map_err(|e| KgError::Brain(e.to_string()))?;
        tx.execute("INSERT INTO notes_fts (id, raw_text, tags) VALUES (?1,?2,?3)",
            params![n.fm.id, n.body, tags]).map_err(|e| KgError::Brain(e.to_string()))?;
    }
    let edges = derive_edges(notes);
    for e in &edges {
        let rt = serde_json::to_string(&e.rel_type).unwrap().trim_matches('"').to_string();
        tx.execute("INSERT OR IGNORE INTO edges (src_id,dst_id,rel_type,valid_from,valid_to) VALUES (?1,?2,?3,?4,?5)",
            params![e.src_id, e.dst_id, rt, e.valid_from, e.valid_to]).map_err(|e| KgError::Brain(e.to_string()))?;
    }
    tx.commit().map_err(|e| KgError::Brain(e.to_string()))?;
    Ok(BuildStats { nodes: notes.len(), edges: edges.len(), embedded: embeddings.len() })
}
```

- [ ] **Step 4: Verify + commit**

Run: `cargo test -p kgx-graph --test build` → PASS.
```bash
git add crates/kgx-graph/src/build.rs crates/kgx-graph/tests/build.rs
git commit -m "feat(graph): deterministic full build (nodes, edges, FTS, embeddings)"
```

---

## Task 5: `kgx-graph` — KNN + BM25 query primitives

**Files:**
- Modify: `crates/kgx-graph/src/knn.rs`, `crates/kgx-graph/src/query.rs`
- Test: `crates/kgx-graph/tests/query.rs`

**Interfaces:**
- Consumes: `Brain`, `blob_to_f32`.
- Produces: `knn::vector_search(brain, query_emb: &[f32], limit: usize) -> Result<Vec<(String, f32)>>` (cosine, descending); `query::bm25_search(brain, query: &str, limit) -> Result<Vec<(String, f32)>>` (FTS5 `bm25()`); `query::neighbors(brain, id: &str, hops: u32) -> Result<Vec<String>>`.

- [ ] **Step 1: Write failing tests**

```rust
// crates/kgx-graph/tests/query.rs
use kgx_graph::{Brain, build::build_full, embed::MockEmbedder, knn::vector_search, query::{bm25_search, neighbors}};
use kgx_vault::scan::scan_vault;
use kgx_core::Embedder;
fn fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min") }
fn built() -> Brain {
    let notes = scan_vault(&fixture()).unwrap();
    let mut b = Brain::open_in_memory().unwrap();
    build_full(&mut b, &notes, &MockEmbedder::new()).unwrap();
    b
}
#[test]
fn bm25_finds_postgres() {
    let b = built();
    let hits = bm25_search(&b, "primary datastore Postgres", 5).unwrap();
    assert!(!hits.is_empty());
}
#[test]
fn vector_search_returns_ranked() {
    let b = built();
    let q = MockEmbedder::new().embed(&["primary datastore".into()]).unwrap().remove(0);
    let hits = vector_search(&b, &q, 3).unwrap();
    assert_eq!(hits.len().min(3), hits.len());
    assert!(hits.windows(2).all(|w| w[0].1 >= w[1].1)); // descending
}
#[test]
fn neighbors_one_hop() {
    let b = built();
    let pg = "01FACT01POSTGRESPRIMARY00";
    let n = neighbors(&b, pg, 1).unwrap();
    assert!(!n.is_empty());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p kgx-graph --test query`
Expected: FAIL.

- [ ] **Step 3: Implement knn.rs**

```rust
// crates/kgx-graph/src/knn.rs
use kgx_core::{Result, KgError};
use crate::{Brain, embed::blob_to_f32};
fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
}
pub fn vector_search(brain: &Brain, query_emb: &[f32], limit: usize) -> Result<Vec<(String, f32)>> {
    let mut stmt = brain.conn().prepare("SELECT id, embedding FROM notes WHERE embedding IS NOT NULL")
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let rows = stmt.query_map([], |r| {
        let id: String = r.get(0)?; let blob: Vec<u8> = r.get(1)?; Ok((id, blob))
    }).map_err(|e| KgError::Brain(e.to_string()))?;
    let mut scored: Vec<(String, f32)> = Vec::new();
    for row in rows { let (id, blob) = row.map_err(|e| KgError::Brain(e.to_string()))?;
        scored.push((id, cosine(query_emb, &blob_to_f32(&blob)))); }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
    scored.truncate(limit);
    Ok(scored)
}
```
> Brute-force cosine is correct and deterministic for vault scales (PRD non-goal: >100K nodes → Neo4j). `sqlite-vec` `vec0` can replace this later behind the same signature for speed.

- [ ] **Step 4: Implement query.rs**

```rust
// crates/kgx-graph/src/query.rs
use kgx_core::{Result, KgError};
use crate::Brain;
pub fn bm25_search(brain: &Brain, query: &str, limit: usize) -> Result<Vec<(String, f32)>> {
    let mut stmt = brain.conn().prepare(
        "SELECT id, bm25(notes_fts) AS score FROM notes_fts WHERE notes_fts MATCH ?1 ORDER BY score LIMIT ?2")
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let rows = stmt.query_map(rusqlite::params![query, limit as i64], |r| {
        let id: String = r.get(0)?; let score: f64 = r.get(1)?; Ok((id, -score as f32)) // bm25 lower=better → negate
    }).map_err(|e| KgError::Brain(e.to_string()))?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(|e| KgError::Brain(e.to_string()))
}
pub fn neighbors(brain: &Brain, id: &str, hops: u32) -> Result<Vec<String>> {
    use std::collections::BTreeSet;
    let mut frontier: BTreeSet<String> = BTreeSet::from([id.to_string()]);
    let mut seen = frontier.clone();
    for _ in 0..hops {
        let mut next = BTreeSet::new();
        for node in &frontier {
            let mut stmt = brain.conn().prepare(
                "SELECT dst_id FROM edges WHERE src_id=?1 UNION SELECT src_id FROM edges WHERE dst_id=?1")
                .map_err(|e| KgError::Brain(e.to_string()))?;
            let rows = stmt.query_map([node], |r| r.get::<_, String>(0)).map_err(|e| KgError::Brain(e.to_string()))?;
            for r in rows { let n = r.map_err(|e| KgError::Brain(e.to_string()))?;
                if seen.insert(n.clone()) { next.insert(n); } }
        }
        frontier = next;
    }
    let mut out: Vec<String> = seen.into_iter().filter(|n| n != id).collect();
    out.sort();
    Ok(out)
}
```

- [ ] **Step 5: Verify + commit**

Run: `cargo test -p kgx-graph --test query` → PASS.
```bash
git add crates/kgx-graph/src/{knn,query}.rs crates/kgx-graph/tests/query.rs
git commit -m "feat(graph): cosine KNN, BM25, neighbor expansion"
```

---

## Task 6: `kgx-graph` — incremental build + PageRank

**Files:**
- Modify: `crates/kgx-graph/src/build.rs`; create `crates/kgx-graph/src/pagerank.rs`
- Test: `crates/kgx-graph/tests/incremental.rs`

**Interfaces:**
- Consumes: `Brain`, `BuildStats`, `derive_edges`, `Embedder`.
- Produces: `build::build_incremental(brain, notes, changed_ids: &[String], embedder) -> Result<BuildStats>` (re-embeds only changed notes + recomputes their 1–2 hop edges); `pagerank::compute(brain, damping: f32, iters: u32) -> Result<()>` (writes `pagerank` table via petgraph).

- [ ] **Step 1: Write failing tests**

```rust
// crates/kgx-graph/tests/incremental.rs
use kgx_graph::{Brain, build::{build_full, build_incremental}, embed::MockEmbedder, pagerank};
use kgx_vault::scan::scan_vault;
fn fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min") }
#[test]
fn incremental_matches_full_for_single_change() {
    let notes = scan_vault(&fixture()).unwrap();
    let mut full = Brain::open_in_memory().unwrap();
    build_full(&mut full, &notes, &MockEmbedder::new()).unwrap();
    let mut inc = Brain::open_in_memory().unwrap();
    build_full(&mut inc, &notes, &MockEmbedder::new()).unwrap();
    let changed = vec![notes[0].fm.id.clone()];
    build_incremental(&mut inc, &notes, &changed, &MockEmbedder::new()).unwrap();
    let n_full: i64 = full.conn().query_row("SELECT count(*) FROM notes", [], |r| r.get(0)).unwrap();
    let n_inc: i64 = inc.conn().query_row("SELECT count(*) FROM notes", [], |r| r.get(0)).unwrap();
    assert_eq!(n_full, n_inc);
}
#[test]
fn pagerank_writes_scores() {
    let notes = scan_vault(&fixture()).unwrap();
    let mut b = Brain::open_in_memory().unwrap();
    build_full(&mut b, &notes, &MockEmbedder::new()).unwrap();
    pagerank::compute(&mut b, 0.85, 20).unwrap();
    let cnt: i64 = b.conn().query_row("SELECT count(*) FROM pagerank WHERE score > 0", [], |r| r.get(0)).unwrap();
    assert!(cnt > 0);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p kgx-graph --test incremental`
Expected: FAIL.

- [ ] **Step 3: Implement build_incremental**

```rust
// append to crates/kgx-graph/src/build.rs
pub fn build_incremental(brain: &mut Brain, notes: &[Note], changed_ids: &[String], embedder: &dyn Embedder)
    -> Result<BuildStats> {
    use std::collections::BTreeSet;
    let changed: BTreeSet<&str> = changed_ids.iter().map(|s| s.as_str()).collect();
    if changed.is_empty() { return Ok(BuildStats { nodes: 0, edges: 0, embedded: 0 }); }
    let subset: Vec<&Note> = notes.iter().filter(|n| changed.contains(n.fm.id.as_str())).collect();
    let texts: Vec<String> = subset.iter().map(|n| format!("{}\n{}", n.fm.title, n.body)).collect();
    let embs = embedder.embed(&texts)?;
    let tx = brain.conn_mut().transaction().map_err(|e| KgError::Brain(e.to_string()))?;
    for (n, emb) in subset.iter().zip(&embs) {
        let tags = serde_json::to_string(&{ let mut t = n.fm.tags.clone(); t.sort(); t }).unwrap();
        let typ = serde_json::to_string(&n.fm.r#type).unwrap().trim_matches('"').to_string();
        let st = serde_json::to_string(&n.fm.status).unwrap().trim_matches('"').to_string();
        tx.execute("INSERT INTO notes (id,path,type,status,valid_from,valid_to,recorded_at,tags,raw_text,embedding)\
            VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)\
            ON CONFLICT(id) DO UPDATE SET path=?2,type=?3,status=?4,valid_from=?5,valid_to=?6,recorded_at=?7,tags=?8,raw_text=?9,embedding=?10",
            params![n.fm.id, n.rel_path.display().to_string(), typ, st, n.fm.valid_from, n.fm.valid_to,
                n.fm.recorded_at, tags, n.body, f32_to_blob(emb)]).map_err(|e| KgError::Brain(e.to_string()))?;
        tx.execute("DELETE FROM notes_fts WHERE id=?1", params![n.fm.id]).map_err(|e| KgError::Brain(e.to_string()))?;
        tx.execute("INSERT INTO notes_fts (id, raw_text, tags) VALUES (?1,?2,?3)",
            params![n.fm.id, n.body, tags]).map_err(|e| KgError::Brain(e.to_string()))?;
        tx.execute("DELETE FROM edges WHERE src_id=?1", params![n.fm.id]).map_err(|e| KgError::Brain(e.to_string()))?;
    }
    // recompute edges for changed sources against the full note set (1-hop targets)
    let all_edges = derive_edges(notes);
    for e in all_edges.iter().filter(|e| changed.contains(e.src_id.as_str())) {
        let rt = serde_json::to_string(&e.rel_type).unwrap().trim_matches('"').to_string();
        tx.execute("INSERT OR IGNORE INTO edges (src_id,dst_id,rel_type,valid_from,valid_to) VALUES (?1,?2,?3,?4,?5)",
            params![e.src_id, e.dst_id, rt, e.valid_from, e.valid_to]).map_err(|e| KgError::Brain(e.to_string()))?;
    }
    tx.commit().map_err(|e| KgError::Brain(e.to_string()))?;
    Ok(BuildStats { nodes: subset.len(), edges: all_edges.iter().filter(|e| changed.contains(e.src_id.as_str())).count(), embedded: embs.len() })
}
```

- [ ] **Step 4: Implement pagerank.rs (petgraph)**

Add `petgraph = "0.6"` to `kgx-graph` deps.
```rust
// crates/kgx-graph/src/pagerank.rs
use std::collections::HashMap;
use petgraph::graph::{DiGraph, NodeIndex};
use kgx_core::{Result, KgError};
use crate::Brain;
pub fn compute(brain: &mut Brain, damping: f32, iters: u32) -> Result<()> {
    let mut g: DiGraph<String, ()> = DiGraph::new();
    let mut idx: HashMap<String, NodeIndex> = HashMap::new();
    { let mut s = brain.conn().prepare("SELECT id FROM notes ORDER BY id").map_err(|e| KgError::Brain(e.to_string()))?;
      let rows = s.query_map([], |r| r.get::<_, String>(0)).map_err(|e| KgError::Brain(e.to_string()))?;
      for r in rows { let id = r.map_err(|e| KgError::Brain(e.to_string()))?; let n = g.add_node(id.clone()); idx.insert(id, n); } }
    { let mut s = brain.conn().prepare("SELECT src_id, dst_id FROM edges").map_err(|e| KgError::Brain(e.to_string()))?;
      let rows = s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).map_err(|e| KgError::Brain(e.to_string()))?;
      for r in rows { let (a, b) = r.map_err(|e| KgError::Brain(e.to_string()))?;
        if let (Some(&x), Some(&y)) = (idx.get(&a), idx.get(&b)) { g.add_edge(x, y, ()); } } }
    let n = g.node_count().max(1) as f32;
    let mut rank: HashMap<NodeIndex, f32> = g.node_indices().map(|i| (i, 1.0 / n)).collect();
    for _ in 0..iters {
        let mut next: HashMap<NodeIndex, f32> = g.node_indices().map(|i| (i, (1.0 - damping) / n)).collect();
        for node in g.node_indices() {
            let out: Vec<_> = g.neighbors_directed(node, petgraph::Direction::Outgoing).collect();
            if out.is_empty() { continue; }
            let share = damping * rank[&node] / out.len() as f32;
            for o in out { *next.get_mut(&o).unwrap() += share; }
        }
        rank = next;
    }
    let tx = brain.conn_mut().transaction().map_err(|e| KgError::Brain(e.to_string()))?;
    tx.execute("DELETE FROM pagerank", []).map_err(|e| KgError::Brain(e.to_string()))?;
    for (ni, score) in &rank {
        tx.execute("INSERT INTO pagerank (id, score) VALUES (?1, ?2)", rusqlite::params![g[*ni], *score as f64])
            .map_err(|e| KgError::Brain(e.to_string()))?;
    }
    tx.commit().map_err(|e| KgError::Brain(e.to_string()))?;
    Ok(())
}
```
Add `pub mod pagerank;` to lib.rs.

- [ ] **Step 5: Verify + commit**

Run: `cargo test -p kgx-graph --test incremental` → PASS.
```bash
git add crates/kgx-graph && git commit -m "feat(graph): incremental build + PageRank"
```

---

## Task 7: `kg index` command + change detection

**Files:**
- Create: `crates/kgx-cli/src/commands/index.rs`; modify `crates/kgx-cli/src/{cli.rs,main.rs}`
- Modify: `crates/kgx-cli/Cargo.toml` (add `kgx-graph`, `kgx-tokens`)
- Test: `crates/kgx-cli/tests/cli_index.rs`

**Interfaces:**
- Consumes: `kgx_graph::{Brain, build_full, build_incremental, pagerank}`, `kgx_vault::scan::scan_vault`, `kgx_graph::embed::MockEmbedder`, `kgx_tokens`.
- Produces: `kg index [--full|--incremental] [--pagerank] [--communities] [--json]`. Writes `.kg/brain.sqlite`, `.kg/meta.json`. Records an embed `TokenRecord`.

- [ ] **Step 1: Write failing test**

```rust
// crates/kgx-cli/tests/cli_index.rs
use assert_cmd::Command;
mod common; // copy_fixture helper (reuse from cli_validate via a shared module)
#[test]
fn index_full_builds_brain() {
    let d = common::copy_fixture();
    let out = Command::cargo_bin("kg").unwrap().env("KGX_LLM", "mock")
        .args(["index", "--full", "--pagerank", "--json"]).current_dir(d.path()).assert().success();
    assert!(d.path().join(".kg/brain.sqlite").exists());
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["command"], "index");
    assert!(v["data"]["nodes"].as_u64().unwrap() >= 15);
    // token record written
    assert!(d.path().join(".kg/metrics.log").exists());
}
```
Factor `copy_fixture` into `crates/kgx-cli/tests/common/mod.rs`.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p kgx-cli --test cli_index`
Expected: FAIL.

- [ ] **Step 3: Add `Index` to cli.rs**

```rust
// in Commands enum
/// Build/refresh .kg/brain.sqlite
Index {
    #[arg(long)] full: bool,
    #[arg(long)] incremental: bool,
    #[arg(long)] pagerank: bool,
    #[arg(long)] communities: bool,
},
```

- [ ] **Step 4: Implement index.rs**

```rust
// crates/kgx-cli/src/commands/index.rs
use std::time::Instant;
use crate::output::emit;
use kgx_graph::{Brain, build::build_full, embed::MockEmbedder, pagerank};
use kgx_tokens::record::{append, TokenRecord};

pub fn run(json: bool, full: bool, _incremental: bool, do_pagerank: bool, _communities: bool) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;
    let kg_dir = root.join(".kg");
    let notes = kgx_vault::scan::scan_vault(&root)?;
    let mut brain = Brain::open(&kg_dir.join("brain.sqlite"))?;
    let embedder = MockEmbedder::new(); // real MiniLM swapped in via feature flag (Phase 6)
    // Phase 1 ships --full; --incremental change-detection refined in Phase 2 (uses git/mtime).
    let stats = build_full(&mut brain, &notes, &embedder)?;
    let _ = full;
    if do_pagerank { pagerank::compute(&mut brain, 0.85, 30)?; }
    // token accounting: embeddings counted as input tokens (approx by chars/4)
    let approx_in: u32 = notes.iter().map(|n| (n.body.len() / 4) as u32).sum();
    append(&kg_dir, &TokenRecord { model: "mock-embed".into(), operation: "embed".into(), command: "index".into(),
        input_tokens: approx_in, output_tokens: 0, elapsed_ms: start.elapsed().as_millis() as u64,
        correlation_id: kgx_core::util::new_ulid(), ts: kgx_core::util::now_iso() })?;
    std::fs::write(kg_dir.join("meta.json"), serde_json::to_string_pretty(&serde_json::json!({
        "last_index": kgx_core::util::now_iso(), "nodes": stats.nodes, "edges": stats.edges }))?)?;
    emit("index", stats, json, start, |s| println!("✔ indexed {} nodes, {} edges", s.nodes, s.edges));
    Ok(())
}
```
Wire into `main.rs` match arm. Add `kgx-graph`, `kgx-tokens` to `crates/kgx-cli/Cargo.toml`.

- [ ] **Step 5: Verify + commit**

Run: `cargo test -p kgx-cli --test cli_index` → PASS.
```bash
git add crates/kgx-cli && git commit -m "feat(cli): kg index builds brain + records tokens"
```

---

## Task 8: Smoke T10 (rebuild determinism) + T16 (token accounting)

**Files:**
- Create: `tests/smoke/t10_rebuild.rs` (replace Phase 0 placeholder), `tests/smoke/t16_tokens.rs`

**Interfaces:** Consumes the `kg` binary end-to-end.

- [ ] **Step 1: Write T10 — `rm -rf .kg && kg index --full` is deterministic**

```rust
// tests/smoke/t10_rebuild.rs
use assert_cmd::Command;
mod common;
fn node_count(db: &std::path::Path) -> i64 {
    let c = rusqlite::Connection::open(db).unwrap();
    c.query_row("SELECT count(*) FROM notes", [], |r| r.get(0)).unwrap()
}
#[test]
fn t10_rebuild_is_deterministic() {
    let d = common::copy_fixture();
    Command::cargo_bin("kg").unwrap().env("KGX_LLM","mock").args(["index","--full"]).current_dir(d.path()).assert().success();
    let n1 = node_count(&d.path().join(".kg/brain.sqlite"));
    std::fs::remove_dir_all(d.path().join(".kg")).unwrap();
    Command::cargo_bin("kg").unwrap().env("KGX_LLM","mock").args(["index","--full"]).current_dir(d.path()).assert().success();
    let n2 = node_count(&d.path().join(".kg/brain.sqlite"));
    assert_eq!(n1, n2);
    assert_eq!(n1, 15);
}
```
Add `rusqlite` (bundled) to smoke `[dev-dependencies]`.

- [ ] **Step 2: Write T16 — token log matches operations**

```rust
// tests/smoke/t16_tokens.rs
use assert_cmd::Command;
mod common;
#[test]
fn t16_index_writes_embed_token_record() {
    let d = common::copy_fixture();
    Command::cargo_bin("kg").unwrap().env("KGX_LLM","mock").args(["index","--full"]).current_dir(d.path()).assert().success();
    let log = std::fs::read_to_string(d.path().join(".kg/metrics.log")).unwrap();
    assert!(log.lines().any(|l| l.contains("\"operation\":\"embed\"") && l.contains("\"command\":\"index\"")));
}
```

- [ ] **Step 3: Verify + commit**

Run: `cargo test --workspace --test 'smoke*' -- --test-threads=1` → PASS.
```bash
git add tests/smoke && git commit -m "test(smoke): T10 rebuild determinism, T16 token accounting"
```

---

## Self-Review (Phase 1)

- **Spec coverage:** brain schema (§7) ✔ Task 2; `index --full/--inc` ✔ Tasks 4,6,7; BM25/embeddings/KNN ✔ Tasks 3,5; PageRank ✔ Task 6; token accounting ✔ Tasks 1,7,8; T10 ✔, T16 ✔ Task 8.
- **Type consistency:** `Brain`, `BuildStats`, `build_full`/`build_incremental`, `vector_search`, `bm25_search`, `neighbors`, `pagerank::compute`, `TokenRecord` names match what Phase 2/3 consume.
- **Deferred (documented):** `--incremental` git/mtime change detection → Phase 2 Task on `kg index` refinement; communities → Phase 4; real MiniLM embedder → Phase 6 feature flag.
- **Placeholder scan:** none; every code step complete.
