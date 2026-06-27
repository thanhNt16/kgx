# KGX Phase 3 — Dream Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development or superpowers:executing-plans. Read `2026-06-27-kgx-master-plan.md` (contracts §3, esp. `ProposedDiff`/`Severity`/`FileChange`) and complete Phases 0–2. Steps use `- [ ]`.

**Goal:** Implement the consolidation engine `kgx-dream` — all 7 passes as **pure functions** `(vault + brain) → Vec<ProposedDiff>` — plus `kg dream` (Ralph-loop bounded, stages diffs on a `kg/dream` git branch, never auto-commits to main) and `kg review` (approve/reject gate, `--ponytail-audit` hook). Unlocks T05–T08, T14, T15.

**Architecture:** Wave 3. Each pass is a free function returning `Vec<ProposedDiff>`; passes never touch the filesystem — `kg dream` collects diffs, serializes them to `.kg/staged_diffs.json`, and applies approved ones to files on a dedicated git branch. `Severity::Hard` (hard contradictions) blocks auto-application (T07). The Ralph loop runs passes up to `--max-iterations`, stopping early on `<promise>DONE</promise>` (T15).

**Tech Stack:** Phase 2 retrieval + llm; `git2` (or shelling `git` via `kgx-rtk` in Phase 5) for branch management.

## Global Constraints

Inherit master Global Constraints. Phase-critical: supersession-not-deletion (T05, T14 — files retained, status flipped); review gate (T08 — main untouched until approve); hard contradictions block auto-commit (T07); Ralph loop bounded (T15); ADD-only merge bias (T06 — canonical kept, edges repointed, history retained).

---

## Task 1: `kgx-dream` scaffold + pass trait + context

**Files:**
- Create: `crates/kgx-dream/Cargo.toml`, `src/lib.rs`, `src/context.rs`, `src/passes/mod.rs`
- Test: in-module

**Interfaces:**
- Consumes: `kgx_core::{Note, ProposedDiff, DiffKind, Severity, FileChange, LlmProvider, util}`, `kgx_graph::Brain`, `kgx_retrieval`.
- Produces: `DreamContext<'a> { notes: &'a [Note], brain: &'a Brain, provider: &'a dyn LlmProvider, embedder: &'a dyn Embedder }`; trait-like async pass signature `async fn run(ctx: &DreamContext<'_>) -> Result<Vec<ProposedDiff>>` per pass module; `PassId` enum `{ Dedup, Contradiction, Supersession, Staleness, Community, OrphanRepair, OpenQuestions }`.

- [ ] **Step 1: Crate manifest**

```toml
# crates/kgx-dream/Cargo.toml
[package]
name = "kgx-dream"
edition.workspace = true
version.workspace = true
license.workspace = true
[dependencies]
kgx-core = { path = "../kgx-core" }
kgx-vault = { path = "../kgx-vault" }
kgx-graph = { path = "../kgx-graph" }
kgx-retrieval = { path = "../kgx-retrieval" }
kgx-llm = { path = "../kgx-llm" }
serde.workspace = true
serde_json.workspace = true
[dev-dependencies]
tokio = { version = "1", features = ["rt-multi-thread","macros"] }
tempfile.workspace = true
```

- [ ] **Step 2: Implement context.rs + PassId**

```rust
// crates/kgx-dream/src/context.rs
use kgx_core::{Note, LlmProvider, Embedder};
use kgx_graph::Brain;
pub struct DreamContext<'a> {
    pub notes: &'a [Note], pub brain: &'a Brain,
    pub provider: &'a dyn LlmProvider, pub embedder: &'a dyn Embedder,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassId { Dedup, Contradiction, Supersession, Staleness, Community, OrphanRepair, OpenQuestions }
impl PassId {
    pub fn name(self) -> &'static str { match self {
        PassId::Dedup=>"dedup", PassId::Contradiction=>"contradiction", PassId::Supersession=>"supersession",
        PassId::Staleness=>"staleness", PassId::Community=>"community", PassId::OrphanRepair=>"orphan_repair",
        PassId::OpenQuestions=>"open_questions" } }
    pub fn parse(s: &str) -> Option<PassId> { match s {
        "dedup"=>Some(Self::Dedup), "contradiction"=>Some(Self::Contradiction), "supersession"=>Some(Self::Supersession),
        "staleness"=>Some(Self::Staleness), "community"=>Some(Self::Community), "orphan_repair"=>Some(Self::OrphanRepair),
        "open_questions"=>Some(Self::OpenQuestions), _=>None } }
    pub fn all() -> [PassId; 7] { [Self::Dedup, Self::Contradiction, Self::Supersession, Self::Staleness, Self::Community, Self::OrphanRepair, Self::OpenQuestions] }
}
```
```rust
// crates/kgx-dream/src/lib.rs
pub mod context; pub mod passes; pub mod run;
pub use context::{DreamContext, PassId};
```

- [ ] **Step 3: Verify compile + commit**

Run: `cargo build -p kgx-dream`
```bash
git add crates/kgx-dream && git commit -m "chore(dream): scaffold context + pass ids"
```

---

## Task 2: Supersession pass (T05)

**Files:**
- Create: `crates/kgx-dream/src/passes/supersession.rs`
- Test: in-module

**Interfaces:**
- Produces: `passes::supersession::run(ctx) -> Result<Vec<ProposedDiff>>`. Finds active facts whose `valid_from` is older and that a newer fact contradicts on the same subject; proposes closing the old note's `valid_to` and setting `superseded_by`, plus `status: superseded`. **No file deletion** — emits `FileChange` with `before`/`after` of the same path.

- [ ] **Step 1: Write failing test**

```rust
// crates/kgx-dream/src/passes/supersession.rs
#[cfg(test)]
mod tests {
    use super::*;
    use kgx_dream::context::DreamContext;
    use kgx_graph::{Brain, build::build_full, embed::MockEmbedder};
    use kgx_llm::mock::MockProvider;
    use kgx_vault::scan::scan_vault;
    fn fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min") }
    #[tokio::test]
    async fn proposes_supersede_for_old_datastore_fact() {
        let notes = scan_vault(&fixture()).unwrap();
        let mut b = Brain::open_in_memory().unwrap();
        build_full(&mut b, &notes, &MockEmbedder::new()).unwrap();
        let p = MockProvider::new(); let e = MockEmbedder::new();
        let ctx = DreamContext { notes: &notes, brain: &b, provider: &p, embedder: &e };
        let diffs = run(&ctx).await.unwrap();
        assert!(diffs.iter().any(|d| matches!(d.kind, kgx_core::DiffKind::Supersede)));
        // the older Postgres fact (valid_from 2026-01-15) should be the one superseded
        let d = diffs.iter().find(|d| matches!(d.kind, kgx_core::DiffKind::Supersede)).unwrap();
        let after = d.files[0].after.as_ref().unwrap();
        assert!(after.contains("status: superseded"));
        assert!(after.contains("valid_to:"));
        assert!(d.files[0].before.is_some(), "must retain file (before set)");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p kgx-dream supersession`
Expected: FAIL.

- [ ] **Step 3: Implement supersession.rs**

```rust
// crates/kgx-dream/src/passes/supersession.rs (above tests)
use kgx_core::{Note, ProposedDiff, DiffKind, Severity, FileChange, Result, util};
use crate::context::DreamContext;
use kgx_vault::write::render_note;

/// Pairs of active facts sharing a tag/subject where one is strictly newer → propose superseding the older.
pub async fn run(ctx: &DreamContext<'_>) -> Result<Vec<ProposedDiff>> {
    let mut diffs = Vec::new();
    let facts: Vec<&Note> = ctx.notes.iter()
        .filter(|n| matches!(n.fm.r#type, kgx_core::NoteType::Fact) && matches!(n.fm.status, kgx_core::Status::Active))
        .collect();
    for (i, a) in facts.iter().enumerate() {
        for b in facts.iter().skip(i + 1) {
            if !share_subject(a, b) { continue; }
            let (older, newer) = order_by_valid_from(a, b);
            // ask the LLM whether `newer` supersedes `older`
            if !llm_supersedes(ctx, older, newer).await? { continue; }
            let mut updated = (*older).clone();
            updated.fm.status = kgx_core::Status::Superseded;
            updated.fm.valid_to = Some(newer.fm.valid_from.clone().unwrap_or_else(|| util::now_iso()[..10].to_string()));
            updated.fm.superseded_by = Some(newer.fm.id.clone());
            diffs.push(ProposedDiff {
                id: util::new_ulid(), pass: "supersession".into(), kind: DiffKind::Supersede,
                severity: Severity::Soft, rationale: format!("'{}' superseded by newer '{}'", older.fm.title, newer.fm.title),
                files: vec![FileChange { rel_path: older.rel_path.display().to_string(),
                    before: Some(render_note(older)), after: Some(render_note(&updated)) }],
            });
        }
    }
    diffs.sort_by(|x, y| x.files[0].rel_path.cmp(&y.files[0].rel_path));
    Ok(diffs)
}
fn share_subject(a: &Note, b: &Note) -> bool {
    a.fm.tags.iter().any(|t| b.fm.tags.contains(t))
}
fn order_by_valid_from<'a>(a: &'a Note, b: &'a Note) -> (&'a Note, &'a Note) {
    let av = a.fm.valid_from.clone().unwrap_or_default();
    let bv = b.fm.valid_from.clone().unwrap_or_default();
    if av <= bv { (a, b) } else { (b, a) }
}
async fn llm_supersedes(ctx: &DreamContext<'_>, older: &Note, newer: &Note) -> Result<bool> {
    // Mock keys on "CONTRADICTION"; a hard verdict on same-subject pair implies supersession of the older.
    let prompt = format!("CONTRADICTION\nOLD: {}\nNEW: {}", older.body, newer.body);
    let resp = ctx.provider.complete(kgx_core::LlmRequest {
        system: "Reply JSON {verdict: agree|soft|scope|hard, rationale}".into(),
        prompt, max_tokens: 256, temperature: 0.0 }).await?;
    let v: serde_json::Value = serde_json::from_str(&resp.text).unwrap_or(serde_json::json!({"verdict":"agree"}));
    Ok(matches!(v["verdict"].as_str(), Some("hard") | Some("scope")))
}
```
Add `pub mod supersession;` to `passes/mod.rs`.

- [ ] **Step 4: Verify + commit**

Run: `cargo test -p kgx-dream supersession` → PASS.
```bash
git add crates/kgx-dream/src/passes && git commit -m "feat(dream): supersession pass (T05) — retain file, flip status"
```

---

## Task 3: Contradiction pass (T07)

**Files:**
- Create: `crates/kgx-dream/src/passes/contradiction.rs`
- Test: in-module

**Interfaces:**
- Produces: `passes::contradiction::run(ctx) -> Result<Vec<ProposedDiff>>`. For same-subject active fact pairs, classifies via LLM into `agree|soft|scope|hard`. Emits `DiffKind::FlagContradiction` with `Severity` mapped (`hard→Hard`, `scope→Scope`, `soft→Soft`). Hard ones carry `severity: Hard` so the review gate blocks auto-apply.

- [ ] **Step 1: Write failing test**

```rust
// crates/kgx-dream/src/passes/contradiction.rs
#[cfg(test)]
mod tests {
    use super::*;
    use kgx_dream::context::DreamContext;
    use kgx_graph::{Brain, build::build_full, embed::MockEmbedder};
    use kgx_llm::mock::MockProvider;
    use kgx_vault::scan::scan_vault;
    fn fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min") }
    #[tokio::test]
    async fn flags_hard_contradiction_with_blocking_severity() {
        let notes = scan_vault(&fixture()).unwrap();
        let mut b = Brain::open_in_memory().unwrap();
        build_full(&mut b, &notes, &MockEmbedder::new()).unwrap();
        let p = MockProvider::new(); let e = MockEmbedder::new();
        let ctx = DreamContext { notes: &notes, brain: &b, provider: &p, embedder: &e };
        let diffs = run(&ctx).await.unwrap();
        assert!(diffs.iter().any(|d| matches!(d.severity, kgx_core::Severity::Hard)));
    }
}
```

- [ ] **Step 2–4: Implement, verify**

```rust
// crates/kgx-dream/src/passes/contradiction.rs (above tests)
use kgx_core::{Note, ProposedDiff, DiffKind, Severity, FileChange, Result, util};
use crate::context::DreamContext;
pub async fn run(ctx: &DreamContext<'_>) -> Result<Vec<ProposedDiff>> {
    let mut diffs = Vec::new();
    let facts: Vec<&Note> = ctx.notes.iter()
        .filter(|n| matches!(n.fm.r#type, kgx_core::NoteType::Fact) && matches!(n.fm.status, kgx_core::Status::Active)).collect();
    for (i, a) in facts.iter().enumerate() {
        for b in facts.iter().skip(i + 1) {
            if !a.fm.tags.iter().any(|t| b.fm.tags.contains(t)) { continue; }
            let resp = ctx.provider.complete(kgx_core::LlmRequest {
                system: "Reply JSON {verdict: agree|soft|scope|hard, rationale}".into(),
                prompt: format!("CONTRADICTION\nA: {}\nB: {}", a.body, b.body), max_tokens: 256, temperature: 0.0 }).await?;
            let v: serde_json::Value = serde_json::from_str(&resp.text).unwrap_or(serde_json::json!({"verdict":"agree"}));
            let sev = match v["verdict"].as_str() {
                Some("hard") => Severity::Hard, Some("scope") => Severity::Scope,
                Some("soft") => Severity::Soft, _ => continue };
            diffs.push(ProposedDiff {
                id: util::new_ulid(), pass: "contradiction".into(), kind: DiffKind::FlagContradiction, severity: sev,
                rationale: v["rationale"].as_str().unwrap_or("conflict").to_string(),
                files: vec![FileChange { rel_path: a.rel_path.display().to_string(), before: None, after: None },
                            FileChange { rel_path: b.rel_path.display().to_string(), before: None, after: None }] });
        }
    }
    diffs.sort_by(|x, y| x.id.cmp(&y.id));
    Ok(diffs)
}
```
Run `cargo test -p kgx-dream contradiction` → PASS.
```bash
git add crates/kgx-dream/src/passes/contradiction.rs && git commit -m "feat(dream): contradiction pass (T07) — hard severity blocks auto-commit"
```

---

## Task 4: Dedup/Merge pass (T06)

**Files:**
- Create: `crates/kgx-dream/src/passes/dedup.rs`
- Test: in-module

**Interfaces:**
- Produces: `passes::dedup::run(ctx) -> Result<Vec<ProposedDiff>>`. Embedding-blocks near-duplicate notes (cosine > 0.92), asks LLM `MERGE`; on merge proposes `DiffKind::Merge`: keep canonical (lowest ULID = oldest), mark the other `status: archived` with `superseded_by: canonical` (ADD-only bias — no deletion), and emits `FileChange`s to repoint edges by updating the duplicate's links to the canonical title.

- [ ] **Step 1: Write failing test** (inject two near-identical facts into a temp vault; assert one `Merge` diff, canonical retained, duplicate archived not deleted).

- [ ] **Step 2–4: Implement dedup.rs** using `ctx.embedder` to compute cosine over note bodies, blocking pairs above threshold, then `MERGE` prompt. On merge: `before`/`after` for the duplicate flipping `status: archived` + `superseded_by`. Canonical file unchanged (no FileChange). Run test → PASS.

```rust
// signature + core blocking (body shown in full in the file)
pub async fn run(ctx: &DreamContext<'_>) -> Result<Vec<ProposedDiff>> { /* cosine blocking + MERGE prompt + archive-dup diff */ }
```

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-dream/src/passes/dedup.rs && git commit -m "feat(dream): dedup/merge pass (T06) — ADD-only, archive not delete"
```

---

## Task 5: Staleness/Archive pass (T14)

**Files:**
- Create: `crates/kgx-dream/src/passes/staleness.rs`
- Test: in-module

**Interfaces:**
- Produces: `passes::staleness::run(ctx) -> Result<Vec<ProposedDiff>>`. Flags active notes whose `source:` points to a missing `raw/` file **and** whose `valid_from` is older than a threshold (default 365 days from `util::now_iso`) → proposes `DiffKind::Archive` (`status: archived`, file retained).

- [ ] **Step 1: Write failing test** — temp vault with a fact whose `source:` references a nonexistent raw file and old `valid_from`; assert one `Archive` diff with `after` containing `status: archived` and `before` set (file retained).

- [ ] **Step 2–4: Implement staleness.rs**, verify.

```rust
pub async fn run(ctx: &DreamContext<'_>) -> Result<Vec<ProposedDiff>> {
    // dead-source detection: parse source wikilink → check raw/<stem>.md exists on disk via ctx note set
    // age check: valid_from older than threshold; emit Archive diff (before=render(old), after=render(archived))
}
```

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-dream/src/passes/staleness.rs && git commit -m "feat(dream): staleness/archive pass (T14) — file retained"
```

---

## Task 6: Community resummary, Orphan repair, Open questions passes

**Files:**
- Create: `crates/kgx-dream/src/passes/{community,orphan_repair,open_questions}.rs`
- Test: in-module per file

**Interfaces:**
- `passes::community::run` — reads `communities` table (populated in Phase 4; if empty, no-op returning `vec![]`), produces/updates a MOC summary note per community (`DiffKind::Resummarize` or `AddNote`).
- `passes::orphan_repair::run` — uses `kgx_graph::links::orphans` + `kgx_retrieval::search` over each orphan's body to propose `DiffKind::AddLink` diffs that insert `[[wikilinks]]` into the orphan body (the orphan's `after` gains links).
- `passes::open_questions::run` — finds `type: question` notes whose text is now answered by ≥1 active fact (via `search`) → proposes flipping the question to `status: archived` (`DiffKind::Archive`) with rationale citing the answering fact; conversely detects gaps and proposes new `type: question` notes (`DiffKind::AddNote`).

- [ ] **Step 1: Write failing tests** — orphan_repair: assert ≥1 `AddLink` diff targeting `01FACT05ORPHAN0000000000` (after Phase 2's mock search returns neighbors). community: assert no-op on empty communities table. open_questions: with a question answered by the Postgres fact, assert one `Archive` diff.

- [ ] **Step 2–4: Implement the three files**, verify each. community returns `vec![]` when `communities` empty (real summaries land when Phase 4 populates the table).

- [ ] **Step 5: Register all passes + commit**

```rust
// crates/kgx-dream/src/passes/mod.rs
pub mod supersession; pub mod contradiction; pub mod dedup; pub mod staleness;
pub mod community; pub mod orphan_repair; pub mod open_questions;
```
```bash
git add crates/kgx-dream/src/passes && git commit -m "feat(dream): community, orphan-repair, open-questions passes"
```

---

## Task 7: Dream runner — Ralph loop + staging (T15)

**Files:**
- Create: `crates/kgx-dream/src/run.rs`
- Test: `crates/kgx-dream/tests/run.rs`

**Interfaces:**
- Consumes: all passes, `DreamContext`, `PassId`.
- Produces: `run::DreamOptions { passes: Vec<PassId>, max_iterations: u32 }`; `run::dream(ctx, opts) -> Result<DreamRun>`; `DreamRun { diffs: Vec<ProposedDiff>, iterations: u32, done_signal: bool }`. Runs selected passes each iteration, accumulates diffs, stops at `max_iterations` or when a pass returns the sentinel (`done_signal` true when an iteration yields zero new diffs — the convergence/DONE condition).

- [ ] **Step 1: Write failing test (bound at max_iterations)**

```rust
// crates/kgx-dream/tests/run.rs
use kgx_dream::{run::{dream, DreamOptions}, context::DreamContext, PassId};
use kgx_graph::{Brain, build::build_full, embed::MockEmbedder};
use kgx_llm::mock::MockProvider;
use kgx_vault::scan::scan_vault;
fn fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min") }
#[tokio::test]
async fn dream_respects_max_iterations() {
    let notes = scan_vault(&fixture()).unwrap();
    let mut b = Brain::open_in_memory().unwrap();
    build_full(&mut b, &notes, &MockEmbedder::new()).unwrap();
    let p = MockProvider::new(); let e = MockEmbedder::new();
    let ctx = DreamContext { notes: &notes, brain: &b, provider: &p, embedder: &e };
    let r = dream(&ctx, DreamOptions { passes: PassId::all().to_vec(), max_iterations: 3 }).await.unwrap();
    assert!(r.iterations <= 3, "exceeded max_iterations: {}", r.iterations);
    assert!(!r.diffs.is_empty());
}
```

- [ ] **Step 2–4: Implement run.rs**, verify.

```rust
// crates/kgx-dream/src/run.rs
use kgx_core::{ProposedDiff, Result};
use crate::{context::DreamContext, passes, PassId};
#[derive(Debug, Clone)]
pub struct DreamOptions { pub passes: Vec<PassId>, pub max_iterations: u32 }
#[derive(Debug)]
pub struct DreamRun { pub diffs: Vec<ProposedDiff>, pub iterations: u32, pub done_signal: bool }
pub async fn dream(ctx: &DreamContext<'_>, opts: DreamOptions) -> Result<DreamRun> {
    let mut all = Vec::new(); let mut iterations = 0; let mut done = false;
    for _ in 0..opts.max_iterations.max(1) {
        iterations += 1;
        let mut round = Vec::new();
        for pid in &opts.passes {
            let d = match pid {
                PassId::Dedup => passes::dedup::run(ctx).await?,
                PassId::Contradiction => passes::contradiction::run(ctx).await?,
                PassId::Supersession => passes::supersession::run(ctx).await?,
                PassId::Staleness => passes::staleness::run(ctx).await?,
                PassId::Community => passes::community::run(ctx).await?,
                PassId::OrphanRepair => passes::orphan_repair::run(ctx).await?,
                PassId::OpenQuestions => passes::open_questions::run(ctx).await?,
            };
            round.extend(d);
        }
        // dedup proposals by (pass, files) to detect convergence
        let new: Vec<ProposedDiff> = round.into_iter()
            .filter(|d| !all.iter().any(|e: &ProposedDiff| e.pass == d.pass && diff_paths(e) == diff_paths(d))).collect();
        if new.is_empty() { done = true; break; }   // <promise>DONE</promise> equivalent: convergence
        all.extend(new);
    }
    Ok(DreamRun { diffs: all, iterations, done_signal: done })
}
fn diff_paths(d: &ProposedDiff) -> Vec<String> { d.files.iter().map(|f| f.rel_path.clone()).collect() }
```
Run `cargo test -p kgx-dream --test run` → PASS.
```bash
git add crates/kgx-dream/src/run.rs crates/kgx-dream/tests/run.rs
git commit -m "feat(dream): bounded Ralph-loop runner with convergence stop (T15)"
```

---

## Task 8: `kg dream` command — stage on git branch (T08)

**Files:**
- Create: `crates/kgx-cli/src/commands/dream.rs`, `crates/kgx-cli/src/git.rs`; modify cli/main/Cargo (`kgx-dream`, `git2`).
- Test: `crates/kgx-cli/tests/cli_dream.rs`

**Interfaces:**
- Consumes: `kgx_dream::{dream, DreamOptions, PassId}`, `kgx_llm::select`, `kgx_graph::Brain`.
- Produces: `kg dream [--max-iterations N] [--only set] [--intensity ...] [--json]`. Stages diffs to `.kg/staged_diffs.json`; creates/checks out `kg/dream` git branch; **does not** modify `notes/` on `main`. `git::ensure_branch(root, "kg/dream")`.

- [ ] **Step 1: Write failing test (main untouched)**

```rust
// crates/kgx-cli/tests/cli_dream.rs
use assert_cmd::Command; mod common;
#[test]
fn dream_stages_without_touching_main() {
    let d = common::copy_fixture();
    // init git so branch ops work
    std::process::Command::new("git").args(["init","-q"]).current_dir(d.path()).status().unwrap();
    std::process::Command::new("git").args(["add","-A"]).current_dir(d.path()).status().unwrap();
    std::process::Command::new("git").args(["-c","user.email=t@t","-c","user.name=t","commit","-qm","init"]).current_dir(d.path()).status().unwrap();
    Command::cargo_bin("kg").unwrap().env("KGX_LLM","mock").args(["index","--full"]).current_dir(d.path()).assert().success();
    let pg_before = std::fs::read_to_string(d.path().join("notes/facts/f-postgres-primary.md")).unwrap();
    Command::cargo_bin("kg").unwrap().env("KGX_LLM","mock")
        .args(["dream","--max-iterations","2","--json"]).current_dir(d.path()).assert().success();
    // staged diffs exist
    assert!(d.path().join(".kg/staged_diffs.json").exists());
    // file on disk (checked out branch) not yet applying supersede until review --approve
    let pg_after = std::fs::read_to_string(d.path().join("notes/facts/f-postgres-primary.md")).unwrap();
    assert_eq!(pg_before, pg_after, "dream must not apply changes before review");
}
```

- [ ] **Step 2–4: Implement git.rs + dream.rs**, verify.

```rust
// crates/kgx-cli/src/git.rs
use std::path::Path;
pub fn ensure_branch(root: &Path, name: &str) -> anyhow::Result<()> {
    let exists = std::process::Command::new("git").args(["rev-parse","--verify",name])
        .current_dir(root).output()?.status.success();
    let args: Vec<&str> = if exists { vec!["checkout", name] } else { vec!["checkout","-b", name] };
    let st = std::process::Command::new("git").args(&args).current_dir(root).status()?;
    if !st.success() { anyhow::bail!("git checkout {name} failed"); }
    Ok(())
}
```
```rust
// crates/kgx-cli/src/commands/dream.rs
use std::time::Instant;
use crate::output::emit;
use kgx_dream::{run::{dream, DreamOptions}, context::DreamContext, PassId};
use kgx_graph::Brain;
pub fn run(json: bool, max_iterations: u32, only: Option<String>) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;
    let notes = kgx_vault::scan::scan_vault(&root)?;
    let brain = Brain::open(&root.join(".kg/brain.sqlite"))?;
    let provider = kgx_llm::select::provider_from_env()?;
    let embedder = kgx_llm::select::embedder_from_env();
    let passes: Vec<PassId> = match only {
        Some(s) => s.split(',').filter_map(PassId::parse).collect(),
        None => PassId::all().to_vec() };
    let ctx = DreamContext { notes: &notes, brain: &brain, provider: provider.as_ref(), embedder: embedder.as_ref() };
    let rt = tokio::runtime::Runtime::new()?;
    let result = rt.block_on(dream(&ctx, DreamOptions { passes, max_iterations }))?;
    // stage diffs; create branch but DO NOT apply
    crate::git::ensure_branch(&root, "kg/dream").ok(); // best-effort; staging is the gate
    std::fs::write(root.join(".kg/staged_diffs.json"), serde_json::to_string_pretty(&result.diffs)?)?;
    emit("dream", serde_json::json!({"staged": result.diffs.len(), "iterations": result.iterations,
        "hard_blocks": result.diffs.iter().filter(|d| matches!(d.severity, kgx_core::Severity::Hard)).count()}),
        json, start, |_| println!("✔ staged {} diffs over {} iters (run `kg review`)", result.diffs.len(), result.iterations));
    Ok(())
}
```
Add `Dream { max_iterations (default 3), only, intensity }` to cli. Run test → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-cli && git commit -m "feat(cli): kg dream stages diffs on kg/dream branch (T08)"
```

---

## Task 9: `kg review` — approve/reject gate + ponytail-audit hook

**Files:**
- Create: `crates/kgx-cli/src/commands/review.rs`
- Test: `crates/kgx-cli/tests/cli_review.rs`

**Interfaces:**
- Consumes: `.kg/staged_diffs.json`, `kgx_vault::write` (to apply `FileChange.after`).
- Produces: `kg review [--approve <ids|all>] [--reject <ids>] [--interactive] [--ponytail-audit] [--json]`. Applies approved non-`Hard` diffs to files (writes `after`); refuses to apply `Hard` diffs (T07) unless `--approve` names them explicitly. `--ponytail-audit` flags over-broad diffs (placeholder hook; real rules in Phase 5).

- [ ] **Step 1: Write failing test (approve applies; hard blocked)**

```rust
// crates/kgx-cli/tests/cli_review.rs
use assert_cmd::Command; mod common;
#[test]
fn review_approve_all_applies_soft_but_not_hard() {
    let d = common::copy_fixture();
    std::process::Command::new("git").args(["init","-q"]).current_dir(d.path()).status().unwrap();
    Command::cargo_bin("kg").unwrap().env("KGX_LLM","mock").args(["index","--full"]).current_dir(d.path()).assert().success();
    Command::cargo_bin("kg").unwrap().env("KGX_LLM","mock").args(["dream","--max-iterations","2"]).current_dir(d.path()).assert().success();
    let out = Command::cargo_bin("kg").unwrap()
        .args(["review","--approve","all","--json"]).current_dir(d.path()).assert().success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert!(v["data"]["applied"].as_u64().unwrap() >= 1);
    assert!(v["data"]["blocked_hard"].as_u64().unwrap() >= 1, "hard contradiction must be blocked by --approve all");
    // a superseded fact file should now contain status: superseded
    let pg = std::fs::read_to_string(d.path().join("notes/facts/f-postgres-primary.md")).unwrap();
    assert!(pg.contains("superseded"));
}
```

- [ ] **Step 2–4: Implement review.rs**, verify.

```rust
// crates/kgx-cli/src/commands/review.rs
use std::time::Instant;
use crate::output::emit;
use kgx_core::{ProposedDiff, Severity};
pub fn run(json: bool, approve: Option<String>, _reject: Option<String>, _interactive: bool, ponytail_audit: bool) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;
    let staged: Vec<ProposedDiff> = serde_json::from_str(&std::fs::read_to_string(root.join(".kg/staged_diffs.json"))?)?;
    let approve_all = approve.as_deref() == Some("all");
    let approve_ids: std::collections::BTreeSet<String> =
        approve.as_deref().filter(|s| *s != "all").map(|s| s.split(',').map(|x| x.trim().to_string()).collect()).unwrap_or_default();
    let mut applied = 0u32; let mut blocked_hard = 0u32; let mut audit_flags = Vec::new();
    for d in &staged {
        let explicitly = approve_ids.contains(&d.id);
        let selected = explicitly || (approve_all && !matches!(d.severity, Severity::Hard));
        if matches!(d.severity, Severity::Hard) && !explicitly { blocked_hard += 1; continue; }
        if !selected { continue; }
        if ponytail_audit && d.files.iter().filter(|f| f.after.is_some()).count() > 3 {
            audit_flags.push(format!("{} touches {} files (over-broad)", d.id, d.files.len()));
        }
        for f in &d.files {
            if let Some(after) = &f.after { std::fs::write(root.join(&f.rel_path), after)?; applied += 1; }
        }
    }
    emit("review", serde_json::json!({"applied": applied, "blocked_hard": blocked_hard, "audit_flags": audit_flags}),
        json, start, |_| println!("✔ applied {applied}; {blocked_hard} hard diff(s) blocked"));
    Ok(())
}
```
Add `Review { approve, reject, interactive, ponytail_audit (--ponytail-audit) }` to cli. Run test → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-cli && git commit -m "feat(cli): kg review gate — apply soft, block hard (T07/T08)"
```

---

## Task 10: Smoke T05, T06, T07, T08, T14, T15

**Files:**
- Create: `tests/smoke/{t05_supersede,t06_dedup,t07_contradiction,t08_review_gate,t14_stale,t15_ralph}.rs`

- [ ] **Step 1: T05** — after dream+review approve, old Postgres fact has `status: superseded`, `valid_to` set, **file still exists**.
- [ ] **Step 2: T06** — inject duplicate fact, dream proposes Merge, review keeps canonical + archives dup (both files exist).
- [ ] **Step 3: T07** — `kg dream` reports `hard_blocks ≥ 1`; `kg review --approve all` leaves the contradiction unapplied (`blocked_hard ≥ 1`).
- [ ] **Step 4: T08** — before `kg review`, `git status` on `main` shows `notes/` unchanged; staged_diffs.json present.
- [ ] **Step 5: T14** — fact with dead source + old date → dream proposes Archive; after approve, `status: archived`, file retained.
- [ ] **Step 6: T15** — `kg dream --max-iterations 3 --json`; assert reported `iterations ≤ 3`.
- [ ] **Step 7: Verify + commit**

Run: `cargo test --workspace --test 'smoke*' -- --test-threads=1` → PASS.
```bash
git add tests/smoke && git commit -m "test(smoke): T05-T08, T14, T15 dream + review gate"
```

---

## Self-Review (Phase 3)

- **Spec coverage:** all 7 dream passes (§10) ✔ Tasks 2–6; Ralph loop + cron-ready runner (§10) ✔ Task 7; `kg dream`/`kg review` (§9) ✔ Tasks 8–9; review gate + ponytail-audit hook ✔ Task 9; T05–T08, T14, T15 ✔ Task 10.
- **Type consistency:** passes all return `Vec<ProposedDiff>` (master §3); `DreamContext`, `DreamOptions`, `dream`, `PassId` names match Phase 5 cron/MCP consumers (`dream_step`).
- **Deferred:** real Leiden-backed community summaries → Phase 4 populates `communities`, this pass already reads it; ponytail-audit real rule set → Phase 5.
- **Placeholder scan:** Tasks 4–6 describe pass bodies in prose with exact signatures + tests; implementer writes the full body to satisfy the named test. All other steps have complete code.
