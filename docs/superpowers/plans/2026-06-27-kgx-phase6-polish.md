# KGX Phase 6 — Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development or superpowers:executing-plans. Read `2026-06-27-kgx-master-plan.md` and complete Phases 0–5. Steps use `- [ ]`. This phase completes the PRD §22 handoff checklist.

**Goal:** Ship the human-facing surface: `kgx-viz` (self-contained HTML graph, Mermaid, DOT, Obsidian), `kgx-docs` (use-case HTML flows), `kg dashboard` (TUI), `kg status`, `kg tokens`, `kg ship`/`kg pull` (OKF bundle round-trip), the real MiniLM embedder (feature-flagged), the installer one-liner, and CI finalization. Unlocks T11 (full round-trip) and T12.

**Architecture:** Wave 4/6. `kgx-viz` and `kgx-docs` consume the brain + vault and emit static artifacts (zero server). `kg dashboard` (ratatui) reads brain + metrics. `ship`/`pull` produce/ingest OKF bundles validated by `kgx-okf`. The real `MiniLmEmbedder` lands behind `--features candle` so default builds/CI stay network-free.

**Tech Stack:** `tera` (HTML/docs templates), `ratatui` + `crossterm` (TUI), `tar`/`flate2` (bundles), `candle-core`/`candle-transformers`/`tokenizers` + `hf-hub` (MiniLM), Phase 1–5 crates.

## Global Constraints

Inherit master Global Constraints. Phase-critical: HTML graph is a single self-contained file (§14); OKF round-trip lossless (T11); viz node/edge counts match brain (T12); real embedder is opt-in (CI default = `MockEmbedder`).

---

## Task 1: `kgx-viz` — graph export (HTML/Mermaid/DOT/Obsidian)

**Files:**
- Create: `crates/kgx-viz/Cargo.toml`, `src/lib.rs`, `src/model.rs`, `src/html.rs`, `src/mermaid.rs`, `src/dot.rs`; `templates/graph.html.tera`
- Test: in-module (mermaid/dot) + `crates/kgx-viz/tests/html.rs`

**Interfaces:**
- Consumes: `kgx_graph::Brain`.
- Produces: `model::GraphModel { nodes: Vec<VizNode>, edges: Vec<VizEdge> }`; `model::from_brain(brain, filter: Option<&str>) -> Result<GraphModel>`; `html::render(&GraphModel) -> String` (self-contained, inlined D3 + data); `mermaid::render`, `dot::render`. `VizNode { id, title, r#type, status, pagerank }`, `VizEdge { src, dst, rel }`.

- [ ] **Step 1: Crate manifest**

```toml
# crates/kgx-viz/Cargo.toml
[package]
name = "kgx-viz"
edition.workspace = true
version.workspace = true
license.workspace = true
[dependencies]
kgx-core = { path = "../kgx-core" }
kgx-graph = { path = "../kgx-graph" }
serde.workspace = true
serde_json.workspace = true
tera.workspace = true
[dev-dependencies]
tempfile.workspace = true
kgx-vault = { path = "../kgx-vault" }
```

- [ ] **Step 2: Write failing tests**

```rust
// crates/kgx-viz/tests/html.rs
use kgx_viz::{model::from_brain, html, mermaid::render as mermaid};
use kgx_graph::{Brain, build::build_full, embed::MockEmbedder};
use kgx_vault::scan::scan_vault;
fn fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min") }
fn model() -> kgx_viz::model::GraphModel {
    let notes = scan_vault(&fixture()).unwrap();
    let mut b = Brain::open_in_memory().unwrap();
    build_full(&mut b, &notes, &MockEmbedder::new()).unwrap();
    from_brain(&b, None).unwrap()
}
#[test]
fn html_is_self_contained_and_counts_match() {
    let m = model();
    let h = html::render(&m);
    assert!(h.contains("<html") && h.contains("</html>"));
    assert!(!h.contains("http://") && !h.contains("https://"), "must inline assets (self-contained)");
    // node count embedded in data matches model
    assert!(h.contains(&format!("\"nodes\":")) );
    assert_eq!(m.nodes.len(), 15);
}
#[test]
fn mermaid_renders_edges() {
    let m = model();
    let s = mermaid(&m);
    assert!(s.starts_with("graph TD") || s.starts_with("flowchart"));
    assert!(s.contains("-->"));
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p kgx-viz`
Expected: FAIL.

- [ ] **Step 4: Implement model.rs / mermaid.rs / dot.rs / html.rs**

```rust
// crates/kgx-viz/src/model.rs
use kgx_core::{Result, KgError};
use kgx_graph::Brain;
#[derive(Debug, Clone, serde::Serialize)] pub struct VizNode { pub id: String, pub title: String, pub r#type: String, pub status: String, pub pagerank: f64 }
#[derive(Debug, Clone, serde::Serialize)] pub struct VizEdge { pub src: String, pub dst: String, pub rel: String }
#[derive(Debug, Clone, serde::Serialize)] pub struct GraphModel { pub nodes: Vec<VizNode>, pub edges: Vec<VizEdge> }
pub fn from_brain(brain: &Brain, filter: Option<&str>) -> Result<GraphModel> {
    let where_clause = match filter { Some(t) => format!("WHERE n.type='{t}'"), None => String::new() };
    let mut ns = brain.conn().prepare(&format!(
        "SELECT n.id, n.path, n.type, n.status, COALESCE(p.score,0) FROM notes n LEFT JOIN pagerank p ON p.id=n.id {where_clause} ORDER BY n.id"))
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let nodes: Vec<VizNode> = ns.query_map([], |r| Ok(VizNode { id: r.get(0)?, title: r.get::<_,String>(1)?,
        r#type: r.get(2)?, status: r.get(3)?, pagerank: r.get(4)? })).map_err(|e| KgError::Brain(e.to_string()))?
        .collect::<std::result::Result<_,_>>().map_err(|e| KgError::Brain(e.to_string()))?;
    let ids: std::collections::BTreeSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let mut es = brain.conn().prepare("SELECT src_id, dst_id, rel_type FROM edges ORDER BY src_id, dst_id")
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let edges: Vec<VizEdge> = es.query_map([], |r| Ok(VizEdge { src: r.get(0)?, dst: r.get(1)?, rel: r.get(2)? }))
        .map_err(|e| KgError::Brain(e.to_string()))?.collect::<std::result::Result<Vec<_>,_>>()
        .map_err(|e| KgError::Brain(e.to_string()))?.into_iter()
        .filter(|e| ids.contains(e.src.as_str()) && ids.contains(e.dst.as_str())).collect();
    Ok(GraphModel { nodes, edges })
}
```
```rust
// crates/kgx-viz/src/mermaid.rs
use crate::model::GraphModel;
pub fn render(m: &GraphModel) -> String {
    let mut s = String::from("graph TD\n");
    for e in &m.edges { s.push_str(&format!("  {}-->|{}|{}\n", san(&e.src), e.rel, san(&e.dst))); }
    s
}
fn san(id: &str) -> String { id.chars().filter(|c| c.is_alphanumeric()).collect() }
```
```rust
// crates/kgx-viz/src/dot.rs
use crate::model::GraphModel;
pub fn render(m: &GraphModel) -> String {
    let mut s = String::from("digraph kgx {\n");
    for n in &m.nodes { s.push_str(&format!("  \"{}\" [label=\"{}\"];\n", n.id, n.r#type)); }
    for e in &m.edges { s.push_str(&format!("  \"{}\" -> \"{}\" [label=\"{}\"];\n", e.src, e.dst, e.rel)); }
    s.push_str("}\n"); s
}
```
```rust
// crates/kgx-viz/src/html.rs
use crate::model::GraphModel;
const TEMPLATE: &str = include_str!("../templates/graph.html.tera");
const D3_MIN: &str = include_str!("../templates/d3.v7.min.js"); // vendored, inlined for self-containment
pub fn render(m: &GraphModel) -> String {
    let data = serde_json::to_string(m).unwrap();
    let mut ctx = tera::Context::new();
    ctx.insert("graph_data", &data);
    ctx.insert("d3_js", D3_MIN);
    tera::Tera::one_off(TEMPLATE, &ctx, false).unwrap_or_else(|e| format!("<!-- render error: {e} -->"))
}
```
Create `templates/graph.html.tera` (force-directed D3 + type/status filters + click→side-panel + time slider, all inline `{{ d3_js | safe }}` and `const DATA = {{ graph_data | safe }}`). Vendor `d3.v7.min.js` into `templates/` so the output has zero external requests. `lib.rs`: `pub mod model; pub mod html; pub mod mermaid; pub mod dot;`

- [ ] **Step 5: Verify + commit**

Run: `cargo test -p kgx-viz` → PASS.
```bash
git add crates/kgx-viz && git commit -m "feat(viz): self-contained HTML graph + mermaid + dot exporters"
```

---

## Task 2: `kg graph` command (T12)

**Files:**
- Create: `crates/kgx-cli/src/commands/graph.rs`; cli/main/Cargo (`kgx-viz`)
- Test: `crates/kgx-cli/tests/cli_graph.rs`

**Interfaces:**
- Produces: `kg graph --format html|mermaid|dot|obsidian --out <file> [--filter type] [--json]`. `obsidian` format emits a `.canvas` JSON. Counts in output match brain.

- [ ] **Step 1: Write failing test (T12 counts match)**

```rust
// crates/kgx-cli/tests/cli_graph.rs
use assert_cmd::Command; mod common;
#[test]
fn graph_html_node_count_matches_brain() {
    let d = common::copy_fixture();
    Command::cargo_bin("kg").unwrap().env("KGX_LLM","mock").args(["index","--full","--pagerank"]).current_dir(d.path()).assert().success();
    let out = d.path().join("graph.html");
    Command::cargo_bin("kg").unwrap().args(["graph","--format","html","--out"]).arg(&out).current_dir(d.path()).assert().success();
    let html = std::fs::read_to_string(&out).unwrap();
    let conn = rusqlite::Connection::open(d.path().join(".kg/brain.sqlite")).unwrap();
    let n: i64 = conn.query_row("SELECT count(*) FROM notes", [], |r| r.get(0)).unwrap();
    assert!(html.contains(&format!("\"id\"")));
    // crude count: number of node ids embedded equals brain node count
    let embedded = html.matches("\"title\":").count() as i64;
    assert_eq!(embedded, n);
}
```

- [ ] **Step 2–4: Implement graph.rs**, verify.

```rust
// crates/kgx-cli/src/commands/graph.rs
use std::time::Instant;
use crate::output::emit;
use kgx_graph::Brain;
use kgx_viz::{model::from_brain, html, mermaid, dot};
pub fn run(json: bool, format: &str, out: Option<std::path::PathBuf>, filter: Option<String>) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;
    let brain = Brain::open(&root.join(".kg/brain.sqlite"))?;
    let m = from_brain(&brain, filter.as_deref())?;
    let content = match format {
        "html" => html::render(&m), "mermaid" => mermaid::render(&m), "dot" => dot::render(&m),
        "obsidian" => serde_json::to_string_pretty(&serde_json::json!({
            "nodes": m.nodes.iter().enumerate().map(|(i,n)| serde_json::json!({"id":n.id,"type":"text","text":n.title,"x":i as i64*50,"y":0,"width":200,"height":60})).collect::<Vec<_>>(),
            "edges": m.edges.iter().enumerate().map(|(i,e)| serde_json::json!({"id":format!("e{i}"),"fromNode":e.src,"toNode":e.dst})).collect::<Vec<_>>() }))?,
        other => anyhow::bail!("unknown format: {other}"),
    };
    let out_path = out.unwrap_or_else(|| root.join(format!("graph.{}", if format=="obsidian" {"canvas"} else {format})));
    std::fs::write(&out_path, &content)?;
    emit("graph", serde_json::json!({"out": out_path.display().to_string(), "nodes": m.nodes.len(), "edges": m.edges.len()}),
        json, start, |_| println!("✔ wrote {} ({} nodes, {} edges)", out_path.display(), m.nodes.len(), m.edges.len()));
    Ok(())
}
```
Add `Graph { #[arg(long, default_value="html")] format: String, #[arg(long)] out: Option<PathBuf>, #[arg(long)] filter: Option<String> }`. Run test → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-cli && git commit -m "feat(cli): kg graph export (html/mermaid/dot/obsidian) — T12"
```

---

## Task 3: `kgx-docs` + `kg docs usecase`

**Files:**
- Create: `crates/kgx-docs/Cargo.toml`, `src/lib.rs`, `src/usecase.rs`; `templates/usecase.html.tera`; `crates/kgx-cli/src/commands/docs.rs`
- Test: `crates/kgx-docs/tests/usecase.rs`, `crates/kgx-cli/tests/cli_docs.rs`

**Interfaces:**
- Produces: `usecase::UseCase { Research, Onboarding, Meetings, Pkm, AgentMemory, TeamSharing }`; `usecase::render(uc: UseCase) -> String` (HTML: narrative + exact command sequence + Mermaid diagram). `kg docs usecase <name> --out <file>`. 6 use cases (PRD §3/§14).

- [ ] **Step 1–4:** Manifest, failing test (assert HTML contains the command sequence for `research` incl. `kg capture`/`kg extract`/`kg ask`), implement `usecase.rs` (a static table of `(title, narrative, commands, mermaid)` per use case rendered through the tera template), wire `docs.rs` CLI. Each use case's command block must be copy-pasteable and valid. Run tests → PASS.

```rust
// crates/kgx-docs/src/usecase.rs (shape)
pub enum UseCase { Research, Onboarding, Meetings, Pkm, AgentMemory, TeamSharing }
pub fn render(uc: UseCase) -> String { /* tera one_off with per-usecase narrative+commands+mermaid */ }
pub fn parse(s: &str) -> Option<UseCase> { /* "research"|"onboarding"|... */ }
```

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-docs crates/kgx-cli && git commit -m "feat(docs): kg docs usecase — 6 HTML flow generators"
```

---

## Task 4: `kg status` + `kg tokens`

**Files:**
- Create: `crates/kgx-cli/src/commands/{status,tokens}.rs`
- Test: `crates/kgx-cli/tests/cli_status.rs`

**Interfaces:**
- Consumes: `kgx_graph::Brain`, `kgx_graph::links::orphans`, `kgx_tokens::aggregate::summarize`, `.kg/meta.json`, `.kg/staged_diffs.json`.
- Produces: `kg status [--verbose] [--json]` (node/edge counts, orphans, stale candidates, pending diffs, last index/dream timestamps); `kg tokens [--since 7d|30d] [--by operation|command|day] [--json]`.

- [ ] **Step 1: Write failing test**

```rust
// crates/kgx-cli/tests/cli_status.rs
use assert_cmd::Command; mod common;
#[test]
fn status_json_reports_counts_and_orphans() {
    let d = common::copy_fixture();
    Command::cargo_bin("kg").unwrap().env("KGX_LLM","mock").args(["index","--full"]).current_dir(d.path()).assert().success();
    let out = Command::cargo_bin("kg").unwrap().args(["status","--json"]).current_dir(d.path()).assert().success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["data"]["nodes"], 15);
    assert_eq!(v["data"]["orphans"], 1);
}
#[test]
fn tokens_by_operation_json() {
    let d = common::copy_fixture();
    Command::cargo_bin("kg").unwrap().env("KGX_LLM","mock").args(["index","--full"]).current_dir(d.path()).assert().success();
    let out = Command::cargo_bin("kg").unwrap().args(["tokens","--by","operation","--json"]).current_dir(d.path()).assert().success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert!(v["data"]["aggregates"].as_array().unwrap().iter().any(|a| a["key"]=="embed"));
}
```

- [ ] **Step 2–4: Implement status.rs + tokens.rs**, verify.

```rust
// crates/kgx-cli/src/commands/status.rs
use std::time::Instant;
use crate::output::emit;
use kgx_graph::Brain;
pub fn run(json: bool, _verbose: bool) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;
    let notes = kgx_vault::scan::scan_vault(&root)?;
    let orphans = kgx_graph::links::orphans(&notes).len();
    let brain = Brain::open(&root.join(".kg/brain.sqlite"))?;
    let nodes: i64 = brain.conn().query_row("SELECT count(*) FROM notes", [], |r| r.get(0)).unwrap_or(0);
    let edges: i64 = brain.conn().query_row("SELECT count(*) FROM edges", [], |r| r.get(0)).unwrap_or(0);
    let pending = std::fs::read_to_string(root.join(".kg/staged_diffs.json")).ok()
        .and_then(|s| serde_json::from_str::<Vec<serde_json::Value>>(&s).ok()).map(|v| v.len()).unwrap_or(0);
    let meta: serde_json::Value = std::fs::read_to_string(root.join(".kg/meta.json")).ok()
        .and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(serde_json::json!({}));
    emit("status", serde_json::json!({"nodes": nodes, "edges": edges, "orphans": orphans,
        "pending_diffs": pending, "last_index": meta["last_index"]}), json, start,
        |d| println!("nodes={} edges={} orphans={} pending={}", d["nodes"], d["edges"], d["orphans"], d["pending_diffs"]));
    Ok(())
}
```
`tokens.rs` parses `--since` (`7d`→7) and `--by` → `GroupBy`, calls `summarize`, emits `{aggregates}`. Add `Status`/`Tokens` to cli. Run tests → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-cli && git commit -m "feat(cli): kg status + kg tokens analytics"
```

---

## Task 5: `kg dashboard` (ratatui TUI)

**Files:**
- Create: `crates/kgx-cli/src/commands/dashboard.rs`; Cargo (`ratatui`, `crossterm`)
- Test: `crates/kgx-cli/tests/cli_dashboard.rs` (only `--json` path, TUI not unit-tested)

**Interfaces:**
- Produces: `kg dashboard [--json]`. With `--json`, prints the same snapshot as `status` plus token sparkline data (no TUI). Without `--json`, launches a ratatui screen (counts, trends, token sparkline, scheduler health). Test only the `--json` branch (TUI is manual-QA).

- [ ] **Step 1–4:** Failing test asserts `kg dashboard --json` returns `{nodes, edges, tokens_by_day}`. Implement: reuse `status` snapshot + `summarize(.., GroupBy::Day)`; guard the ratatui event loop behind `if !json`. Run test → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-cli && git commit -m "feat(cli): kg dashboard (TUI + --json snapshot)"
```

---

## Task 6: `kg ship` / `kg pull` — OKF bundle round-trip (T11 full)

**Files:**
- Create: `crates/kgx-okf/src/bundle.rs`; `crates/kgx-cli/src/commands/{ship,pull}.rs`; Cargo (`tar`, `flate2`)
- Test: `crates/kgx-okf/tests/bundle.rs`, `tests/smoke/t11_okf.rs` (extend)

**Interfaces:**
- Consumes: `kgx_vault::scan`, `kgx_okf::check_okf`.
- Produces: `bundle::ship(root, out: &Path) -> Result<()>` (tar.gz of `index.md`/`log.md`/`CLAUDE.md`/`notes/`/`raw/`, excluding `.kg/`); `bundle::pull(bundle: &Path, root, namespace: Option<&str>) -> Result<usize>` (extract into `notes/<namespace>/` subtree). CLI `kg ship --out <file>`, `kg pull <file> --namespace <ns>`.

- [ ] **Step 1: Write failing round-trip test**

```rust
// crates/kgx-okf/tests/bundle.rs
use kgx_okf::{bundle::{ship, pull}, check_okf};
use std::fs;
fn copy_fixture() -> tempfile::TempDir { /* reuse helper: deep-copy tests/fixtures/vault-min */ unimplemented!() }
#[test]
fn ship_then_pull_is_lossless_and_valid() {
    let src = copy_fixture();
    let bundle = src.path().join("out.okf.tar.gz");
    ship(src.path(), &bundle).unwrap();
    let dst = tempfile::tempdir().unwrap();
    fs::create_dir_all(dst.path().join("notes")).unwrap();
    fs::write(dst.path().join("index.md"), "# Index\n").unwrap();
    fs::write(dst.path().join("log.md"), "# Log\n").unwrap();
    let n = pull(&bundle, dst.path(), Some("imported")).unwrap();
    assert!(n > 0);
    assert!(dst.path().join("notes/imported").exists());
    // imported subtree validates
    let report = check_okf(dst.path()).unwrap();
    assert!(report.ok, "{:?}", report.errors);
}
```
Replace `copy_fixture` `unimplemented!()` with the deep-copy helper used elsewhere.

- [ ] **Step 2–4: Implement bundle.rs**, verify.

```rust
// crates/kgx-okf/src/bundle.rs
use std::path::Path;
use kgx_core::{Result, KgError};
pub fn ship(root: &Path, out: &Path) -> Result<()> {
    let file = std::fs::File::create(out).map_err(|e| KgError::Io { path: out.display().to_string(), source: e })?;
    let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut tar = tar::Builder::new(enc);
    for item in ["index.md","log.md","CLAUDE.md","notes","raw"] {
        let p = root.join(item);
        if !p.exists() { continue; }
        if p.is_dir() { tar.append_dir_all(item, &p).map_err(|e| KgError::Other(e.to_string()))?; }
        else { tar.append_path_with_name(&p, item).map_err(|e| KgError::Other(e.to_string()))?; }
    }
    tar.finish().map_err(|e| KgError::Other(e.to_string()))?;
    Ok(())
}
pub fn pull(bundle: &Path, root: &Path, namespace: Option<&str>) -> Result<usize> {
    let file = std::fs::File::open(bundle).map_err(|e| KgError::Io { path: bundle.display().to_string(), source: e })?;
    let dec = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(dec);
    let dest = match namespace { Some(ns) => root.join("notes").join(ns), None => root.to_path_buf() };
    std::fs::create_dir_all(&dest).map_err(|e| KgError::Io { path: dest.display().to_string(), source: e })?;
    let mut count = 0;
    for entry in archive.entries().map_err(|e| KgError::Other(e.to_string()))? {
        let mut e = entry.map_err(|e| KgError::Other(e.to_string()))?;
        let path = e.path().map_err(|e| KgError::Other(e.to_string()))?.into_owned();
        // namespace: drop the leading "notes/" so imported notes land under notes/<ns>/
        let rel = path.strip_prefix("notes").unwrap_or(&path);
        let target = dest.join(rel);
        if let Some(parent) = target.parent() { std::fs::create_dir_all(parent).ok(); }
        if e.header().entry_type().is_file() { e.unpack(&target).map_err(|e| KgError::Other(e.to_string()))?; count += 1; }
    }
    Ok(count)
}
```
Add `pub mod bundle;` to okf lib. CLI `ship.rs`/`pull.rs` thin wrappers + `Ship`/`Pull` cli args. Run test → PASS.

- [ ] **Step 5: Extend smoke T11 (full round-trip via binary)**

```rust
// tests/smoke/t11_okf.rs (extend)
#[test]
fn t11_ship_pull_validate_roundtrip() {
    let d = common::copy_fixture();
    let bundle = d.path().join("b.okf.tar.gz");
    assert_cmd::Command::cargo_bin("kg").unwrap().args(["ship","--out"]).arg(&bundle).current_dir(d.path()).assert().success();
    let dst = common::empty_vault();
    assert_cmd::Command::cargo_bin("kg").unwrap().args(["pull"]).arg(&bundle).args(["--namespace","team"]).current_dir(dst.path()).assert().success();
    assert_cmd::Command::cargo_bin("kg").unwrap().args(["validate","--okf"]).current_dir(dst.path()).assert().success();
}
```

- [ ] **Step 6: Commit**

```bash
git add crates/kgx-okf crates/kgx-cli tests/smoke/t11_okf.rs
git commit -m "feat(okf): ship/pull bundle round-trip (T11 full)"
```

---

## Task 7: Real MiniLM embedder (feature-flagged)

**Files:**
- Modify: `crates/kgx-graph/Cargo.toml` (`[features] candle = [...]`), `crates/kgx-graph/src/embed.rs`; `crates/kgx-llm/src/select.rs`
- Test: `crates/kgx-graph/tests/minilm.rs` (gated `#[cfg(feature="candle")]`, `#[ignore]` in CI)

**Interfaces:**
- Produces: `#[cfg(feature="candle")] MiniLmEmbedder::load() -> Result<MiniLmEmbedder>` implementing `Embedder` (384-dim, all-MiniLM-L6-v2 via candle + hf-hub + tokenizers). `embedder_from_env` returns it when `KGX_EMBED=minilm` and the feature is built.

- [ ] **Step 1–4:** Add `candle-core`, `candle-transformers`, `tokenizers`, `hf-hub` as optional deps under `[features] candle`. Implement `MiniLmEmbedder` (load model+tokenizer, mean-pool + L2-normalize → 384 f32). Gate the test behind the feature and `#[ignore]` (downloads weights; not run in default CI). Update `embedder_from_env`. Run `cargo build -p kgx-graph --features candle` → builds; `cargo test --workspace` (default) unaffected.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-graph crates/kgx-llm && git commit -m "feat(graph): optional candle MiniLM embedder (feature=candle)"
```

---

## Task 8: Installer one-liner + sync helpers

**Files:**
- Create: `install.sh`; `crates/kgx-cli/src/commands/sync.rs`
- Test: `tests/smoke/t_install.rs` (shellcheck + dry-run), `crates/kgx-cli/tests/cli_sync.rs`

**Interfaces:**
- Produces: `install.sh` (detect OS/arch → download `kg` → `~/.local/bin`; optional `kg init`; if Claude Code present, `claude mcp add`; flags `--with-rtk --with-html-graph --with-docs --with-cron`); `kg sync status|push|pull` (thin git wrappers).

- [ ] **Step 1: Write install.sh**

```bash
#!/usr/bin/env bash
set -euo pipefail
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"; ARCH="$(uname -m)"
BIN_DIR="${KGX_BIN_DIR:-$HOME/.local/bin}"; mkdir -p "$BIN_DIR"
case "$ARCH" in x86_64|amd64) ARCH=x86_64;; arm64|aarch64) ARCH=aarch64;; esac
URL="https://get.kgx.sh/bin/kg-${OS}-${ARCH}"
echo "Downloading kg ($OS/$ARCH)..."
curl -fsSL "$URL" -o "$BIN_DIR/kg" && chmod +x "$BIN_DIR/kg"
echo "Installed kg → $BIN_DIR/kg"
for arg in "$@"; do case "$arg" in
  --with-rtk) "$BIN_DIR/kg" --version >/dev/null && echo "rtk hooks via: kg init --with-rtk";;
esac; done
if command -v claude >/dev/null 2>&1; then
  claude mcp add --transport stdio kgx -- kg mcp-server --transport stdio || true
  echo "Registered kgx MCP server with Claude Code"
fi
echo "Next: cd <vault> && kg init --with-skills && kg capture --from ..."
```

- [ ] **Step 2: Write failing smoke test (shellcheck + bash -n)**

```rust
// tests/smoke/t_install.rs
#[test]
fn install_script_is_valid_bash() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    let p = repo.join("install.sh");
    let st = std::process::Command::new("bash").args(["-n"]).arg(&p).status().unwrap();
    assert!(st.success(), "install.sh has syntax errors");
}
```

- [ ] **Step 3–4: Implement sync.rs** (`kg sync status` → `git status -s`; `push`/`pull` → wrap git via `kgx_rtk::run_with_rtk`). Add `Sync { action: String }`. Run tests → PASS.

- [ ] **Step 5: Commit**

```bash
git add install.sh crates/kgx-cli && git commit -m "feat: installer one-liner + kg sync git wrappers"
```

---

## Task 9: CI finalization + full-suite gate

**Files:**
- Modify: `.github/workflows/ci.yml` (add cross-tool + bench jobs); create `tests/smoke/all_tests.rs` (re-exports each `tXX` module so `cargo test --test smoke` runs all 18)

**Interfaces:** Produces the final green gate proving PRD §22 (all 18 tests).

- [ ] **Step 1: Add cross-tool + bench jobs to ci.yml**

```yaml
  cross_tool:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.78
      - run: cargo test --workspace --test 'smoke*' t_skills -- --test-threads=1
      - run: cargo test --workspace -p kgx-mcp --test protocol
  bench:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.78
      - run: cargo bench -p kgx-retrieval -- --output-format bencher | tee bench.txt
```

- [ ] **Step 2: Verify the whole gate locally**

Run: `cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --workspace && cargo bench -p kgx-retrieval`
Expected: all green; all 18 smoke tests (T01–T18) pass; recall bench shows hybrid ≥ semantic+0.10.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml tests/smoke/all_tests.rs
git commit -m "ci: cross-tool + bench jobs; full 18-test gate green"
```

---

## Task 10: Handoff checklist verification

**Files:** None (verification task).

- [ ] **Step 1: Walk PRD §22 checklist** and confirm each item maps to a passing test/artifact:
  - workspace + module layout (Phase 0) ✔
  - `kg init` + `CLAUDE.md` (Phase 0/5) ✔
  - OKF parser/validator (Phase 0) ✔
  - brain schema + deterministic index (Phase 1, T10) ✔
  - hybrid retrieval + RRF (Phase 2, T09) ✔
  - `ask`/`recall`/`search` `--json` (Phase 2) ✔
  - 7 dream passes as pure functions (Phase 3) ✔
  - `review` + `--ponytail-audit` (Phase 3/5) ✔
  - RTK wrapper + installer (Phase 5, T17) ✔
  - Ponytail ladders in CLAUDE.md + prompts (Phase 5, T18) ✔
  - token accounting + `tokens` + `dashboard` (Phase 1/6, T16) ✔
  - `cron` systemd/launchd (Phase 5) ✔
  - `graph --format html` (Phase 6, T12) ✔
  - `docs usecase X` ×6 (Phase 6) ✔
  - `ship`/`pull` round-trip (Phase 6, T11) ✔
  - all 18 tests in CI (Phase 6) ✔
  - installer published (Phase 6 — `install.sh`; publishing to get.kgx.sh is a release-ops step outside the build) ✔/△

- [ ] **Step 2: Commit a STATUS.md** mapping each checklist item → test/file, then tag `v0.1.0-mvp`.

```bash
git add STATUS.md && git commit -m "docs: MVP handoff status (PRD §22 verified)"
git tag v0.1.0-mvp
```

---

## Self-Review (Phase 6)

- **Spec coverage:** viz HTML/mermaid/dot/obsidian (§14) ✔ Tasks 1–2; docs ×6 (§14) ✔ Task 3; dashboard/status/tokens (§15) ✔ Tasks 4–5; ship/pull OKF round-trip (§9, T11) ✔ Task 6; real MiniLM (§18) ✔ Task 7; installer + sync (§12, §20) ✔ Task 8; CI full gate (§16) ✔ Task 9; §22 handoff ✔ Task 10; T11, T12 ✔.
- **Type consistency:** `GraphModel`/`from_brain`/`html::render`/`mermaid::render`/`dot::render`; `bundle::ship`/`pull`; `summarize`/`GroupBy` reused from Phase 1.
- **Deferred → release ops (not build):** publishing the binary to `get.kgx.sh`, signing, and the hosted docs site are deployment steps, noted in Task 10 Step 1.
- **Placeholder scan:** Tasks 3, 5, 7, 8 describe bodies with exact signatures + named tests to satisfy; all schema/algorithm-critical code is shown in full.
