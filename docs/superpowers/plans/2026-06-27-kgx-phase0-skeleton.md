# KGX Phase 0 — Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Read `2026-06-27-kgx-master-plan.md` first — it defines the shared contracts (§3), waves, and Global Constraints this plan assumes. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Stand up the Cargo workspace, the `kgx-core` contracts, OKF parsing/validation (`kgx-okf`), the vault filesystem layer (`kgx-vault`), and the `kg init` + `kg validate` commands — plus the shared fixture vault.

**Architecture:** Wave 0 produces `kgx-core` (contracts only). Wave 1 produces `kgx-okf` and `kgx-vault` in parallel (both depend only on core). A partial Wave 5 adds the `kgx-cli` binary with just `init` and `validate`. This gives a runnable `kg` that scaffolds and validates a vault — the first dogfoodable slice.

**Tech Stack:** `clap`, `serde`/`serde_yaml`/`serde_json`, `pulldown-cmark`, `ulid`, `tera` (init templates), `walkdir`, `tempfile` + `assert_cmd` + `insta` (tests).

## Global Constraints

Inherit all constraints from the master plan's Global Constraints section. Phase-critical ones: MSRV 1.78; crate names `kgx-<module>`; binary named `kg`; every command supports `--json` via `JsonEnvelope`; `okf_version: "0.1"`; no `unwrap` in library code; `cargo clippy -- -D warnings` clean.

---

## Task 0: Workspace bootstrap

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `.gitignore`

**Interfaces:**
- Produces: a compiling empty workspace with `[workspace.members]` that later tasks append to.

- [ ] **Step 1: Create the workspace manifest**

```toml
# Cargo.toml
[workspace]
resolver = "2"
members = ["crates/kgx-core"]

[workspace.package]
edition = "2021"
rust-version = "1.78"
license = "MIT"
version = "0.1.0"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
serde_json = "1"
thiserror = "1"
anyhow = "1"
ulid = "1"
clap = { version = "4", features = ["derive"] }
pulldown-cmark = "0.11"
walkdir = "2"
tera = "1"
tempfile = "3"
assert_cmd = "2"
insta = { version = "1", features = ["json"] }
```

- [ ] **Step 2: Pin toolchain and ignore derived files**

```toml
# rust-toolchain.toml
[toolchain]
channel = "1.78"
components = ["rustfmt", "clippy"]
```

```gitignore
# .gitignore
/target
.kg/
**/*.rs.bk
```

- [ ] **Step 3: Verify the workspace resolves**

Run: `cargo metadata --no-deps --format-version 1 >/dev/null && echo OK`
Expected: prints `OK` (no members yet beyond the placeholder dir, which the next task creates).

- [ ] **Step 4: Commit**

```bash
git init && git add Cargo.toml rust-toolchain.toml .gitignore
git commit -m "chore: bootstrap cargo workspace"
```

---

## Task 1: `kgx-core` crate scaffold

**Files:**
- Create: `crates/kgx-core/Cargo.toml`, `crates/kgx-core/src/lib.rs`

**Interfaces:**
- Produces: `kgx_core` crate skeleton re-exporting submodules added in Tasks 2–5.

- [ ] **Step 1: Create the crate manifest**

```toml
# crates/kgx-core/Cargo.toml
[package]
name = "kgx-core"
edition.workspace = true
rust-version.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
serde_yaml.workspace = true
serde_json.workspace = true
thiserror.workspace = true
ulid.workspace = true
async-trait = "0.1"
```

- [ ] **Step 2: Create the lib root**

```rust
// crates/kgx-core/src/lib.rs
pub mod error;
pub mod types;
pub mod json;
pub mod llm;
pub mod diff;
pub mod util;

pub use error::{KgError, Result};
```

- [ ] **Step 3: Stub the modules so it compiles**

Create empty files `crates/kgx-core/src/{error,types,json,llm,diff,util}.rs` (one line `// filled in next tasks` each).

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p kgx-core`
Expected: builds (empty modules).

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-core
git commit -m "chore: scaffold kgx-core crate"
```

---

## Task 2: `kgx-core` error + types contracts

**Files:**
- Modify: `crates/kgx-core/src/error.rs`, `crates/kgx-core/src/types.rs`
- Test: in-module `#[cfg(test)]`

**Interfaces:**
- Produces: `KgError`, `Result<T>`, `NoteType`, `Status`, `Confidence`, `RelType`, `Frontmatter`, `Note`, `Edge`, `CreatedBy`, `CreatedVia` (exact definitions in master plan §3).

- [ ] **Step 1: Write the failing round-trip test**

```rust
// append to crates/kgx-core/src/types.rs
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frontmatter_roundtrips_minimal() {
        let yaml = "type: fact\nid: 01J9X2ABC\ntitle: Postgres is primary\n";
        let fm: Frontmatter = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(fm.r#type, NoteType::Fact);
        assert_eq!(fm.status, Status::Active);          // defaulted
        assert_eq!(fm.confidence, Confidence::Medium);  // defaulted
        let back = serde_yaml::to_string(&fm).unwrap();
        let fm2: Frontmatter = serde_yaml::from_str(&back).unwrap();
        assert_eq!(fm2.title, "Postgres is primary");
    }
    #[test]
    fn unknown_keys_preserved() {
        let yaml = "type: fact\nid: X\ntitle: T\ncustom_key: hello\n";
        let fm: Frontmatter = serde_yaml::from_str(yaml).unwrap();
        assert!(fm.extra.contains_key("custom_key"));
    }
}
```

- [ ] **Step 2: Run it to verify failure**

Run: `cargo test -p kgx-core types::tests`
Expected: FAIL — `Frontmatter` not defined.

- [ ] **Step 3: Implement error.rs**

Paste the `KgError` + `Result` definition from master plan §3 (`crates/kgx-core/src/error.rs`).

- [ ] **Step 4: Implement types.rs**

Paste the full types block from master plan §3 (`NoteType`, `Status`, `Confidence`, `RelType`, `Frontmatter`, `Note`, `Edge`, `CreatedBy`, `CreatedVia`) above the test module. Add the default helpers the `#[serde(default = "…")]` attrs reference:

```rust
impl Status { pub fn default_active() -> Status { Status::Active } }
impl Confidence { pub fn default_medium() -> Confidence { Confidence::Medium } }
```

- [ ] **Step 5: Verify tests pass**

Run: `cargo test -p kgx-core types::tests`
Expected: PASS (2 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/kgx-core/src/{error,types}.rs
git commit -m "feat(core): frontmatter, note, edge, error contracts"
```

---

## Task 3: `kgx-core` json + util contracts

**Files:**
- Modify: `crates/kgx-core/src/json.rs`, `crates/kgx-core/src/util.rs`
- Test: in-module

**Interfaces:**
- Consumes: nothing.
- Produces: `JsonEnvelope<T>` (master §3); `util::new_ulid() -> String`, `util::extract_wikilinks(&str) -> Vec<String>`, `util::slugify(&str) -> String`, `util::now_iso() -> String`.

- [ ] **Step 1: Write failing tests**

```rust
// append to crates/kgx-core/src/util.rs
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wikilinks_extracted_and_deduped() {
        let links = extract_wikilinks("See [[Postgres]] and [[Billing Service]] and [[Postgres]].");
        assert_eq!(links, vec!["Postgres".to_string(), "Billing Service".to_string()]);
    }
    #[test]
    fn ulid_is_26_chars_and_monotonic() {
        let a = new_ulid(); let b = new_ulid();
        assert_eq!(a.len(), 26);
        assert!(b >= a); // ULIDs sort by time
    }
    #[test]
    fn slugify_basic() { assert_eq!(slugify("Postgres is Primary!"), "postgres-is-primary"); }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p kgx-core util::tests`
Expected: FAIL — functions undefined.

- [ ] **Step 3: Implement util.rs**

```rust
// crates/kgx-core/src/util.rs
use std::sync::OnceLock;

pub fn new_ulid() -> String { ulid::Ulid::new().to_string() }

pub fn now_iso() -> String {
    // RFC3339 UTC without bringing chrono: ulid carries ms; use std time.
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    // delegate formatting to a tiny helper to stay dep-light
    format_iso_millis(now.as_millis() as i64)
}

fn format_iso_millis(ms: i64) -> String {
    // Minimal civil-time conversion (UTC). Good enough for recorded_at stamps.
    let secs = ms / 1000;
    // Use time crate-free approach via humantime-style: fall back to seconds since epoch tag.
    // For determinism in tests, callers may inject; here produce "1970-..+offset"? Instead use OffsetDateTime if available.
    // To avoid hand-rolling calendar math, depend on `time` (add to Cargo): see note.
    let dt = time::OffsetDateTime::from_unix_timestamp(secs).unwrap();
    dt.format(&time::format_description::well_known::Rfc3339).unwrap()
}

static WIKILINK_RE: OnceLock<regex::Regex> = OnceLock::new();
pub fn extract_wikilinks(s: &str) -> Vec<String> {
    let re = WIKILINK_RE.get_or_init(|| regex::Regex::new(r"\[\[([^\]\|]+?)(?:\|[^\]]+)?\]\]").unwrap());
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for cap in re.captures_iter(s) {
        let target = cap[1].trim().to_string();
        if seen.insert(target.clone()) { out.push(target); }
    }
    out
}

pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() { out.push(c.to_ascii_lowercase()); prev_dash = false; }
        else if !prev_dash && !out.is_empty() { out.push('-'); prev_dash = true; }
    }
    out.trim_end_matches('-').to_string()
}
```

Add deps to `crates/kgx-core/Cargo.toml`: `regex = "1"` and `time = { version = "0.3", features = ["formatting", "parsing"] }`.

- [ ] **Step 4: Implement json.rs**

Paste `JsonEnvelope<T>` from master plan §3.

- [ ] **Step 5: Verify**

Run: `cargo test -p kgx-core`
Expected: PASS (all core tests).

- [ ] **Step 6: Commit**

```bash
git add crates/kgx-core
git commit -m "feat(core): json envelope, ulid, wikilink + slug utils"
```

---

## Task 4: `kgx-core` llm + diff contracts

**Files:**
- Modify: `crates/kgx-core/src/llm.rs`, `crates/kgx-core/src/diff.rs`

**Interfaces:**
- Produces: `LlmProvider`, `Embedder`, `LlmRequest`, `LlmResponse`, `ProposedDiff`, `DiffKind`, `Severity`, `FileChange` (master §3). Pure trait/struct definitions — no tests needed beyond compile (no logic).

- [ ] **Step 1: Implement llm.rs and diff.rs**

Paste the `llm.rs` and `diff.rs` blocks from master plan §3 verbatim.

- [ ] **Step 2: Verify compile**

Run: `cargo build -p kgx-core`
Expected: builds.

- [ ] **Step 3: Commit**

```bash
git add crates/kgx-core/src/{llm,diff}.rs
git commit -m "feat(core): llm provider + dream diff contracts"
```

> **Wave 0 gate:** `kgx-core` complete. Wave 1 (`kgx-okf`, `kgx-vault`) may now run in parallel. Append both crates to `Cargo.toml` `[workspace.members]` before starting.

---

## Task 5: `kgx-vault` — note read/parse (Wave 1, agent A)

**Files:**
- Create: `crates/kgx-vault/Cargo.toml`, `crates/kgx-vault/src/lib.rs`, `crates/kgx-vault/src/parse.rs`
- Test: `crates/kgx-vault/src/parse.rs` in-module

**Interfaces:**
- Consumes: `kgx_core::{Note, Frontmatter, KgError, Result, util}`.
- Produces: `parse::parse_note(rel_path: &Path, raw: &str) -> Result<Note>` — splits `---` YAML frontmatter from Markdown body.

- [ ] **Step 1: Crate manifest**

```toml
# crates/kgx-vault/Cargo.toml
[package]
name = "kgx-vault"
edition.workspace = true
version.workspace = true
license.workspace = true
[dependencies]
kgx-core = { path = "../kgx-core" }
serde.workspace = true
serde_yaml.workspace = true
walkdir.workspace = true
[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 2: Write failing parse test**

```rust
// crates/kgx-vault/src/parse.rs
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    #[test]
    fn parses_frontmatter_and_body() {
        let raw = "---\ntype: fact\nid: 01J9X2ABC\ntitle: T\n---\nBody [[Postgres]] here.\n";
        let note = parse_note(Path::new("notes/facts/t.md"), raw).unwrap();
        assert_eq!(note.fm.id, "01J9X2ABC");
        assert!(note.body.contains("Body [[Postgres]]"));
        assert_eq!(note.rel_path, Path::new("notes/facts/t.md"));
    }
    #[test]
    fn missing_frontmatter_errors() {
        let err = parse_note(Path::new("x.md"), "no frontmatter").unwrap_err();
        assert!(matches!(err, kgx_core::KgError::Frontmatter { .. }));
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p kgx-vault parse`
Expected: FAIL — `parse_note` undefined.

- [ ] **Step 4: Implement parse.rs**

```rust
// crates/kgx-vault/src/parse.rs (above the test module)
use std::path::{Path, PathBuf};
use kgx_core::{Frontmatter, Note, KgError, Result};

pub fn parse_note(rel_path: &Path, raw: &str) -> Result<Note> {
    let rest = raw.strip_prefix("---\n").ok_or_else(|| KgError::Frontmatter {
        path: rel_path.display().to_string(), msg: "missing opening '---'".into() })?;
    let end = rest.find("\n---\n").or_else(|| rest.strip_suffix("\n---").map(|_| rest.len() - 4))
        .ok_or_else(|| KgError::Frontmatter {
            path: rel_path.display().to_string(), msg: "missing closing '---'".into() })?;
    let (yaml, body) = rest.split_at(end);
    let fm: Frontmatter = serde_yaml::from_str(yaml).map_err(|e| KgError::Frontmatter {
        path: rel_path.display().to_string(), msg: e.to_string() })?;
    let body = body.trim_start_matches("\n---\n").trim_start_matches("\n---").trim_start().to_string();
    Ok(Note { fm, body, rel_path: PathBuf::from(rel_path) })
}
```

- [ ] **Step 5: Verify pass**

Run: `cargo test -p kgx-vault parse`
Expected: PASS.

- [ ] **Step 6: Lib root**

```rust
// crates/kgx-vault/src/lib.rs
pub mod parse;
pub mod write;
pub mod scan;
pub use parse::parse_note;
```
Stub `write.rs` and `scan.rs` (filled in Tasks 6–7) with `// next task`.

- [ ] **Step 7: Commit**

```bash
git add crates/kgx-vault
git commit -m "feat(vault): note frontmatter+body parser"
```

---

## Task 6: `kgx-vault` — deterministic note write

**Files:**
- Modify: `crates/kgx-vault/src/write.rs`
- Test: in-module

**Interfaces:**
- Consumes: `kgx_core::Note`.
- Produces: `write::render_note(&Note) -> String` (deterministic), `write::write_note(vault_root: &Path, &Note) -> Result<()>`.

- [ ] **Step 1: Write failing determinism test**

```rust
// crates/kgx-vault/src/write.rs
#[cfg(test)]
mod tests {
    use super::*;
    use kgx_core::{Note, Frontmatter, NoteType, Status, Confidence, CreatedBy, CreatedVia};
    use std::path::PathBuf;
    fn note() -> Note {
        Note { rel_path: PathBuf::from("notes/facts/t.md"), body: "Body.".into(),
            fm: Frontmatter { r#type: NoteType::Fact, id: "01J9X2ABC".into(), title: "T".into(),
                status: Status::Active, valid_from: None, valid_to: None, recorded_at: None,
                supersedes: vec![], superseded_by: None, source: None, confidence: Confidence::High,
                sources_count: 0, tags: vec!["b".into(), "a".into()], links: vec![], entity_type: None,
                aliases: vec![], created_by: CreatedBy::Agent, created_via: CreatedVia::Cli,
                extra: Default::default() } }
    }
    #[test]
    fn render_is_deterministic_and_sorted() {
        let r1 = render_note(&note());
        let r2 = render_note(&note());
        assert_eq!(r1, r2);
        assert!(r1.starts_with("---\n"));
        // tags serialized in sorted order for stable diffs
        let tags_line = r1.lines().find(|l| l.starts_with("tags:")).unwrap();
        assert!(tags_line.find('a').unwrap() < tags_line.find('b').unwrap());
        assert!(r1.trim_end().ends_with("Body."));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p kgx-vault write`
Expected: FAIL.

- [ ] **Step 3: Implement write.rs**

```rust
// crates/kgx-vault/src/write.rs (above tests)
use std::path::Path;
use kgx_core::{Note, Result, KgError};

pub fn render_note(note: &Note) -> String {
    // Sort collections for stable diffs (Global Constraint: determinism).
    let mut fm = note.fm.clone();
    fm.tags.sort();
    fm.links.sort();
    fm.aliases.sort();
    fm.supersedes.sort();
    let yaml = serde_yaml::to_string(&fm).expect("frontmatter serialize");
    format!("---\n{yaml}---\n\n{}\n", note.body.trim_end())
}

pub fn write_note(vault_root: &Path, note: &Note) -> Result<()> {
    let full = vault_root.join(&note.rel_path);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).map_err(|e| KgError::Io { path: parent.display().to_string(), source: e })?;
    }
    std::fs::write(&full, render_note(note)).map_err(|e| KgError::Io { path: full.display().to_string(), source: e })
}
```

- [ ] **Step 4: Verify pass**

Run: `cargo test -p kgx-vault write`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-vault/src/write.rs
git commit -m "feat(vault): deterministic note renderer + writer"
```

---

## Task 7: `kgx-vault` — vault scan

**Files:**
- Modify: `crates/kgx-vault/src/scan.rs`
- Test: `crates/kgx-vault/tests/scan_integration.rs`

**Interfaces:**
- Consumes: `parse::parse_note`.
- Produces: `scan::scan_vault(vault_root: &Path) -> Result<Vec<Note>>` — walks `notes/**` and `raw/**`, parses each `.md`, returns ULID-sorted notes; skips `.kg/` and `.obsidian/`.

- [ ] **Step 1: Write failing integration test**

```rust
// crates/kgx-vault/tests/scan_integration.rs
use kgx_vault::scan::scan_vault;
use std::fs;
#[test]
fn scans_notes_and_raw_skipping_derived() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("notes/facts")).unwrap();
    fs::create_dir_all(root.join(".kg")).unwrap();
    fs::write(root.join("notes/facts/a.md"), "---\ntype: fact\nid: 01A\ntitle: A\n---\nx\n").unwrap();
    fs::write(root.join(".kg/ignore.md"), "---\ntype: fact\nid: 99\ntitle: NO\n---\n").unwrap();
    let notes = scan_vault(root).unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].fm.id, "01A");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p kgx-vault --test scan_integration`
Expected: FAIL.

- [ ] **Step 3: Implement scan.rs**

```rust
// crates/kgx-vault/src/scan.rs
use std::path::Path;
use kgx_core::{Note, Result, KgError};
use crate::parse::parse_note;

pub fn scan_vault(vault_root: &Path) -> Result<Vec<Note>> {
    let mut notes = Vec::new();
    for sub in ["notes", "raw"] {
        let base = vault_root.join(sub);
        if !base.exists() { continue; }
        for entry in walkdir::WalkDir::new(&base).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path();
            if !p.is_file() || p.extension().map(|e| e != "md").unwrap_or(true) { continue; }
            let raw = std::fs::read_to_string(p).map_err(|e| KgError::Io { path: p.display().to_string(), source: e })?;
            let rel = p.strip_prefix(vault_root).unwrap_or(p);
            notes.push(parse_note(rel, &raw)?);
        }
    }
    notes.sort_by(|a, b| a.fm.id.cmp(&b.fm.id));
    Ok(notes)
}
```

- [ ] **Step 4: Verify pass**

Run: `cargo test -p kgx-vault --test scan_integration`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-vault/src/scan.rs crates/kgx-vault/tests/scan_integration.rs
git commit -m "feat(vault): deterministic vault scanner"
```

---

## Task 8: `kgx-okf` — OKF validation (Wave 1, agent B)

**Files:**
- Create: `crates/kgx-okf/Cargo.toml`, `crates/kgx-okf/src/lib.rs`, `crates/kgx-okf/src/validate.rs`
- Test: in-module + `crates/kgx-okf/tests/validate_integration.rs`

**Interfaces:**
- Consumes: `kgx_core::{Note, Result}`, `kgx_vault::scan::scan_vault`.
- Produces: `validate::OkfReport { ok: bool, errors: Vec<OkfViolation> }`, `OkfViolation { path, code, msg }`, `validate::check_okf(vault_root: &Path) -> Result<OkfReport>`, and granular `check_frontmatter`, `check_links`, `check_bitemporal`.

- [ ] **Step 1: Crate manifest**

```toml
# crates/kgx-okf/Cargo.toml
[package]
name = "kgx-okf"
edition.workspace = true
version.workspace = true
license.workspace = true
[dependencies]
kgx-core = { path = "../kgx-core" }
kgx-vault = { path = "../kgx-vault" }
serde.workspace = true
serde_json.workspace = true
[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 2: Write failing tests**

```rust
// crates/kgx-okf/tests/validate_integration.rs
use kgx_okf::validate::check_okf;
use std::fs;
fn vault_with(content: &str, name: &str) -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    fs::create_dir_all(d.path().join("notes/facts")).unwrap();
    fs::write(d.path().join("index.md"), "# Index\n").unwrap();
    fs::write(d.path().join("log.md"), "# Log\n").unwrap();
    fs::write(d.path().join(format!("notes/facts/{name}.md")), content).unwrap();
    d
}
#[test]
fn valid_vault_passes() {
    let d = vault_with("---\ntype: fact\nid: 01A\ntitle: A\nvalid_from: 2026-01-01\nvalid_to: null\n---\nx\n", "a");
    let r = check_okf(d.path()).unwrap();
    assert!(r.ok, "expected ok, got {:?}", r.errors);
}
#[test]
fn bitemporal_violation_flagged() {
    // valid_to before valid_from is invalid
    let d = vault_with("---\ntype: fact\nid: 01A\ntitle: A\nvalid_from: 2026-06-01\nvalid_to: 2026-01-01\n---\nx\n", "a");
    let r = check_okf(d.path()).unwrap();
    assert!(!r.ok);
    assert!(r.errors.iter().any(|e| e.code == "bitemporal_order"));
}
#[test]
fn missing_reserved_file_flagged() {
    let d = tempfile::tempdir().unwrap();
    fs::create_dir_all(d.path().join("notes/facts")).unwrap();
    fs::write(d.path().join("notes/facts/a.md"), "---\ntype: fact\nid: 01A\ntitle: A\n---\nx\n").unwrap();
    let r = check_okf(d.path()).unwrap();
    assert!(r.errors.iter().any(|e| e.code == "missing_reserved" && e.path.contains("index.md")));
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p kgx-okf`
Expected: FAIL — `check_okf` undefined.

- [ ] **Step 4: Implement validate.rs**

```rust
// crates/kgx-okf/src/validate.rs
use std::path::Path;
use kgx_core::Result;
use kgx_vault::scan::scan_vault;

#[derive(Debug, serde::Serialize)]
pub struct OkfViolation { pub path: String, pub code: String, pub msg: String }
#[derive(Debug, serde::Serialize)]
pub struct OkfReport { pub ok: bool, pub errors: Vec<OkfViolation> }

pub fn check_okf(root: &Path) -> Result<OkfReport> {
    let mut errors = Vec::new();
    check_reserved(root, &mut errors);
    let notes = scan_vault(root)?;
    check_frontmatter(&notes, &mut errors);
    check_bitemporal(&notes, &mut errors);
    check_links(&notes, &mut errors);
    errors.sort_by(|a, b| (a.path.clone(), a.code.clone()).cmp(&(b.path.clone(), b.code.clone())));
    Ok(OkfReport { ok: errors.is_empty(), errors })
}

fn check_reserved(root: &Path, errors: &mut Vec<OkfViolation>) {
    for f in ["index.md", "log.md"] {
        if !root.join(f).exists() {
            errors.push(OkfViolation { path: f.into(), code: "missing_reserved".into(),
                msg: format!("OKF reserved file '{f}' missing") });
        }
    }
}

pub fn check_frontmatter(notes: &[kgx_core::Note], errors: &mut Vec<OkfViolation>) {
    for n in notes {
        if n.fm.id.trim().is_empty() {
            errors.push(OkfViolation { path: n.rel_path.display().to_string(),
                code: "missing_id".into(), msg: "frontmatter 'id' required".into() });
        }
        if n.fm.title.trim().is_empty() {
            errors.push(OkfViolation { path: n.rel_path.display().to_string(),
                code: "missing_title".into(), msg: "frontmatter 'title' required".into() });
        }
    }
}

pub fn check_bitemporal(notes: &[kgx_core::Note], errors: &mut Vec<OkfViolation>) {
    for n in notes {
        if let (Some(from), Some(to)) = (&n.fm.valid_from, &n.fm.valid_to) {
            if to != "null" && to.as_str() < from.as_str() {
                errors.push(OkfViolation { path: n.rel_path.display().to_string(),
                    code: "bitemporal_order".into(), msg: format!("valid_to {to} precedes valid_from {from}") });
            }
        }
    }
}

pub fn check_links(notes: &[kgx_core::Note], errors: &mut Vec<OkfViolation>) {
    use std::collections::BTreeSet;
    let titles: BTreeSet<&str> = notes.iter().map(|n| n.fm.title.as_str()).collect();
    let ids: BTreeSet<&str> = notes.iter().map(|n| n.fm.id.as_str()).collect();
    for n in notes {
        for link in kgx_core::util::extract_wikilinks(&n.body) {
            let target = link.trim_start_matches("raw/");
            if !titles.contains(target) && !ids.contains(target) && !link.starts_with("raw/") {
                errors.push(OkfViolation { path: n.rel_path.display().to_string(),
                    code: "phantom_link".into(), msg: format!("wikilink [[{link}]] resolves to nothing") });
            }
        }
    }
}
```

> Note: phantom-link checking is intentionally lenient toward `raw/` targets (provenance pointers). Tighter resolution lands in Phase 2's `kg link`.

- [ ] **Step 5: Lib root**

```rust
// crates/kgx-okf/src/lib.rs
pub mod validate;
pub use validate::{check_okf, OkfReport, OkfViolation};
```

- [ ] **Step 6: Verify pass**

Run: `cargo test -p kgx-okf`
Expected: PASS (3 tests).

- [ ] **Step 7: Commit**

```bash
git add crates/kgx-okf
git commit -m "feat(okf): reserved-file, frontmatter, bitemporal, link validation"
```

---

## Task 9: Shared fixture vault

**Files:**
- Create: `tests/fixtures/vault-min/` (full tree per master §4.4), `tests/fixtures/vault-min/COUNTS.json`

**Interfaces:**
- Produces: the canonical fixture every smoke test copies. `COUNTS.json` records expected counts.

- [ ] **Step 1: Author reserved files**

Create `tests/fixtures/vault-min/index.md`:
```markdown
# Knowledge Base Index
- [[Datastore MOC]]
```
Create `tests/fixtures/vault-min/log.md`:
```markdown
# Log
## [2026-01-15] init | vault-min fixture
```
Create `tests/fixtures/vault-min/CLAUDE.md` with a one-line schema pointer (full contract lands in Phase 5):
```markdown
# CLAUDE.md
See note types: fact, entity, decision, experience, moc, source, question.
```

- [ ] **Step 2: Author raw sources (2)**

`tests/fixtures/vault-min/raw/2026-01-15-arch-review.md`:
```markdown
---
type: source
id: 01RAW01ARCHREVIEW00000000
title: "Architecture Review 2026-01-15"
created_by: human
created_via: cli
---
We decided Postgres is the primary datastore. Billing Service depends on it.
```
`tests/fixtures/vault-min/raw/2026-03-01-migration.md`:
```markdown
---
type: source
id: 01RAW02MIGRATION000000000
title: "Datastore Migration Note 2026-03-01"
---
We are moving the primary datastore from Postgres to CockroachDB.
```

- [ ] **Step 3: Author 5 facts (incl. 1 orphan, 1 supersession pair, 1 contradiction)**

Create five files under `notes/facts/`. Key ones:

`f-postgres-primary.md` (will be superseded in T05):
```markdown
---
type: fact
id: 01FACT01POSTGRESPRIMARY00
title: "Postgres is the primary datastore"
status: active
valid_from: 2026-01-15
valid_to: null
source: "[[raw/2026-01-15-arch-review]]"
confidence: high
tags: [infra, datastore]
links: ["[[Postgres]]", "[[Billing Service]]"]
---
The primary datastore is [[Postgres]]. [[Billing Service]] depends on it.
```
`f-cockroach-primary.md` (contradicts the above — T07):
```markdown
---
type: fact
id: 01FACT02COCKROACHPRIMARY0
title: "CockroachDB is the primary datastore"
status: active
valid_from: 2026-03-01
valid_to: null
source: "[[raw/2026-03-01-migration]]"
confidence: medium
tags: [infra, datastore]
links: ["[[CockroachDB]]"]
---
The primary datastore is [[CockroachDB]] as of the migration.
```
`f-orphan.md` (intentional orphan — T04: no inbound/outbound links, not a MOC):
```markdown
---
type: fact
id: 01FACT05ORPHAN0000000000
title: "Standup happens at 9am"
status: active
source: "[[raw/2026-01-15-arch-review]]"
confidence: low
tags: [process]
---
Daily standup is at 9am.
```
Plus `f-billing-deps.md` and `f-backup-policy.md` (each with `source:` and at least one wikilink).

- [ ] **Step 4: Author 3 entities, 2 decisions, 1 moc, 1 question**

`notes/entities/postgres.md`, `notes/entities/cockroachdb.md`, `notes/entities/billing-service.md` (each `type: entity`, `entity_type: system`).
`notes/decisions/adr-001-datastore.md`, `notes/decisions/adr-002-migration.md` (`type: decision`).
`notes/moc/datastore-moc.md` (`type: moc`, `tags: [entrypoint]`, links to facts/entities).
`notes/questions/q-sync-strategy.md` (`type: question`).

Each entity title must match the wikilink text used in facts (e.g. entity titled `Postgres` so `[[Postgres]]` resolves).

- [ ] **Step 5: Author COUNTS.json**

```json
{
  "notes_total": 15,
  "facts": 5,
  "entities": 3,
  "decisions": 2,
  "moc": 1,
  "questions": 1,
  "sources": 2,
  "raw_sources": 2,
  "orphans": 1,
  "contradiction_pairs": 1,
  "supersession_candidates": 1
}
```

- [ ] **Step 6: Verify fixture validates**

Run: `cargo test -p kgx-okf -- --ignored` after adding an `#[ignore]` integration test `fixture_vault_is_okf_valid` that runs `check_okf` on `tests/fixtures/vault-min` and asserts `report.ok`.
Expected: PASS — fixture is OKF-clean (phantom links resolved by the entities authored in Step 4).

- [ ] **Step 7: Commit**

```bash
git add tests/fixtures/vault-min crates/kgx-okf/tests
git commit -m "test: canonical fixture vault + counts sidecar"
```

---

## Task 10: `kgx-cli` skeleton + `kg validate`

**Files:**
- Create: `crates/kgx-cli/Cargo.toml`, `crates/kgx-cli/src/main.rs`, `crates/kgx-cli/src/cli.rs`, `crates/kgx-cli/src/commands/validate.rs`, `crates/kgx-cli/src/output.rs`
- Test: `crates/kgx-cli/tests/cli_validate.rs`

**Interfaces:**
- Consumes: `kgx_okf::check_okf`, `kgx_core::JsonEnvelope`.
- Produces: binary `kg` with `kg validate [--okf] [--json]`; `output::emit(command, data, json, start)` helper used by all future commands.

- [ ] **Step 1: Crate manifest (binary named `kg`)**

```toml
# crates/kgx-cli/Cargo.toml
[package]
name = "kgx-cli"
edition.workspace = true
version.workspace = true
license.workspace = true
[[bin]]
name = "kg"
path = "src/main.rs"
[dependencies]
kgx-core = { path = "../kgx-core" }
kgx-okf = { path = "../kgx-okf" }
kgx-vault = { path = "../kgx-vault" }
clap.workspace = true
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true
[dev-dependencies]
assert_cmd.workspace = true
tempfile.workspace = true
```
Append `crates/kgx-cli` to `Cargo.toml` `[workspace.members]`.

- [ ] **Step 2: Write failing CLI test**

```rust
// crates/kgx-cli/tests/cli_validate.rs
use assert_cmd::Command;
use std::fs;
fn copy_fixture() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min");
    // shallow recursive copy
    for e in walkdir::WalkDir::new(&src) { let e = e.unwrap();
        let rel = e.path().strip_prefix(&src).unwrap();
        let dst = d.path().join(rel);
        if e.file_type().is_dir() { fs::create_dir_all(&dst).unwrap(); }
        else { fs::create_dir_all(dst.parent().unwrap()).unwrap(); fs::copy(e.path(), &dst).unwrap(); }
    }
    d
}
#[test]
fn validate_json_reports_ok_on_fixture() {
    let d = copy_fixture();
    let out = Command::cargo_bin("kg").unwrap()
        .args(["validate", "--okf", "--json"]).current_dir(d.path()).assert().success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["command"], "validate");
    assert_eq!(v["ok"], true);
}
```
Add `walkdir` to `[dev-dependencies]`.

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p kgx-cli --test cli_validate`
Expected: FAIL — binary/command missing.

- [ ] **Step 4: Implement output helper**

```rust
// crates/kgx-cli/src/output.rs
use std::time::Instant;
use kgx_core::json::JsonEnvelope;
pub fn emit<T: serde::Serialize>(command: &str, data: T, json: bool, start: Instant, human: impl FnOnce(&T)) {
    let elapsed = start.elapsed().as_millis() as u64;
    if json {
        let env = JsonEnvelope::success(command, &data, elapsed);
        println!("{}", serde_json::to_string_pretty(&env).expect("serialize envelope"));
    } else {
        human(&data);
    }
}
```

- [ ] **Step 5: Implement clap CLI + validate command**

```rust
// crates/kgx-cli/src/cli.rs
use clap::{Parser, Subcommand};
#[derive(Parser)]
#[command(name = "kg", version, about = "Local-first AI-managed knowledge graph")]
pub struct Cli {
    #[arg(long, global = true)] pub json: bool,
    #[command(subcommand)] pub command: Commands,
}
#[derive(Subcommand)]
pub enum Commands {
    /// Validate vault integrity and OKF conformance
    Validate {
        #[arg(long)] okf: bool,
        #[arg(long)] links: bool,
        #[arg(long)] frontmatter: bool,
        #[arg(long)] bitemporal: bool,
    },
    /// Scaffold a new OKF vault
    Init {
        #[arg(long, default_value = "pkm")] template: String,
        #[arg(long)] okf: bool,
        #[arg(long)] vault: Option<std::path::PathBuf>,
    },
}
```
```rust
// crates/kgx-cli/src/commands/validate.rs
use std::time::Instant;
use crate::output::emit;
pub fn run(json: bool, _okf: bool, _links: bool, _frontmatter: bool, _bitemporal: bool) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;
    let report = kgx_okf::check_okf(&root)?;
    let ok = report.ok;
    emit("validate", report, json, start, |r| {
        if r.ok { println!("✔ vault valid (OKF)"); }
        else { println!("✘ {} violation(s):", r.errors.len());
            for e in &r.errors { println!("  [{}] {} — {}", e.code, e.path, e.msg); } }
    });
    if !ok { std::process::exit(1); }
    Ok(())
}
```
```rust
// crates/kgx-cli/src/main.rs
mod cli; mod output; mod commands { pub mod validate; pub mod init; }
use clap::Parser;
use cli::{Cli, Commands};
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Validate { okf, links, frontmatter, bitemporal } =>
            commands::validate::run(cli.json, okf, links, frontmatter, bitemporal),
        Commands::Init { template, okf, vault } =>
            commands::init::run(cli.json, &template, okf, vault),
    }
}
```

- [ ] **Step 6: Stub init command so it compiles**

```rust
// crates/kgx-cli/src/commands/init.rs
pub fn run(_json: bool, _template: &str, _okf: bool, _vault: Option<std::path::PathBuf>) -> anyhow::Result<()> {
    anyhow::bail!("init not yet implemented") // filled in Task 11
}
```

- [ ] **Step 7: Verify pass**

Run: `cargo test -p kgx-cli --test cli_validate`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/kgx-cli
git commit -m "feat(cli): kg binary skeleton + validate command"
```

---

## Task 11: `kg init` — scaffold OKF vault

**Files:**
- Modify: `crates/kgx-cli/src/commands/init.rs`
- Create: `crates/kgx-cli/templates/` (tera templates per template kind: `research`, `code`, `pkm`, `team`)
- Test: `crates/kgx-cli/tests/cli_init.rs`

**Interfaces:**
- Consumes: `kgx_vault::write`, `kgx_okf::check_okf`.
- Produces: `kg init` creating the full OKF tree (§5 layout) + `CLAUDE.md` + `.gitignore`, then self-validating.

- [ ] **Step 1: Write failing test**

```rust
// crates/kgx-cli/tests/cli_init.rs
use assert_cmd::Command;
#[test]
fn init_creates_valid_okf_vault() {
    let d = tempfile::tempdir().unwrap();
    let target = d.path().join("brain");
    Command::cargo_bin("kg").unwrap()
        .args(["init", "--template", "research", "--okf", "--vault"])
        .arg(&target).assert().success();
    for p in ["index.md", "log.md", "CLAUDE.md", ".gitignore",
              "notes/facts", "notes/entities", "notes/decisions", "notes/moc",
              "notes/questions", "notes/sources", "notes/experiences", "notes/archived", "raw"] {
        assert!(target.join(p).exists(), "missing {p}");
    }
    // freshly-initialized vault must pass OKF validation
    Command::cargo_bin("kg").unwrap().args(["validate", "--okf"]).current_dir(&target).assert().success();
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p kgx-cli --test cli_init`
Expected: FAIL — init bails.

- [ ] **Step 3: Implement init.rs**

```rust
// crates/kgx-cli/src/commands/init.rs
use std::path::PathBuf;
use std::time::Instant;
use crate::output::emit;

const DIRS: &[&str] = &["raw/assets", "notes/facts", "notes/entities", "notes/decisions",
    "notes/experiences", "notes/moc", "notes/sources", "notes/questions", "notes/archived"];

pub fn run(json: bool, template: &str, _okf: bool, vault: Option<PathBuf>) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = vault.unwrap_or(std::env::current_dir()?);
    std::fs::create_dir_all(&root)?;
    for d in DIRS { std::fs::create_dir_all(root.join(d))?; }
    let today = kgx_core::util::now_iso();
    std::fs::write(root.join("index.md"), format!("# Knowledge Base Index\n\nokf_version: \"0.1\"\n\n- (add MOCs here)\n"))?;
    std::fs::write(root.join("log.md"), format!("# Log\n\n## [{}] init | template={}\n", &today[..10], template))?;
    std::fs::write(root.join("CLAUDE.md"), claude_md(template))?;
    std::fs::write(root.join(".gitignore"), ".kg/\n.obsidian/workspace*\n")?;
    let created: Vec<String> = DIRS.iter().map(|s| s.to_string()).collect();
    emit("init", serde_json::json!({"vault": root.display().to_string(), "template": template, "dirs": created}),
        json, start, |_| println!("✔ initialized vault at {} (template: {template})", root.display()));
    Ok(())
}

fn claude_md(template: &str) -> String {
    format!("# CLAUDE.md — KGX Agent Contract\n\nokf_version: \"0.1\"\ntemplate: {template}\n\n\
## Note types\nfact | entity | decision | experience | moc | source | question\n\n\
## Conventions\n- One fact per note (Zettelkasten).\n- Provenance: every fact has `source: [[raw/...]]`.\n\
- Supersede, never delete.\n\n(Full Ponytail ladders embedded in Phase 5.)\n")
}
```

> Templates (`research`/`code`/`pkm`/`team`) differ only in the seed `CLAUDE.md` guidance and a starter MOC; for Phase 0 the difference is the `template:` field. Richer per-template seeding is a Phase 6 polish task.

- [ ] **Step 4: Verify pass**

Run: `cargo test -p kgx-cli --test cli_init`
Expected: PASS.

- [ ] **Step 5: Smoke test T11 (partial — init half of round-trip)**

Create `tests/smoke/t11_okf.rs` asserting `kg init` → `kg validate --okf` succeeds (full ship/pull round-trip added in Phase 6). Wire it into a `tests/smoke/` test crate registered in the workspace.

- [ ] **Step 6: Commit**

```bash
git add crates/kgx-cli tests/smoke/t11_okf.rs
git commit -m "feat(cli): kg init scaffolds OKF vault (T11 partial)"
```

---

## Task 12: Smoke harness + T10 (rebuild determinism placeholder) + CI seed

**Files:**
- Create: `tests/smoke/Cargo.toml` (or register `tests/smoke/` under cli crate), `.github/workflows/ci.yml`
- Test: `tests/smoke/t10_rebuild.rs` (placeholder asserting validate is deterministic; real `.kg` rebuild lands in Phase 1)

**Interfaces:**
- Produces: the smoke test harness and the CI skeleton (lint + unit + integration + smoke jobs).

- [ ] **Step 1: CI workflow**

```yaml
# .github/workflows/ci.yml
name: ci
on: [push, pull_request]
jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.78
        with: { components: rustfmt, clippy }
      - run: cargo fmt --all --check
      - run: cargo clippy --all-targets --all-features -- -D warnings
  unit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.78
      - run: cargo test --workspace --lib
  integration:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.78
      - run: cargo test --workspace --test '*' -- --skip smoke
  smoke:
    runs-on: ubuntu-latest
    env: { KGX_LLM: mock }
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.78
      - run: cargo test --workspace --test 'smoke*' -- --test-threads=1
```

- [ ] **Step 2: Verify CI config parses locally**

Run: `cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: all green (Phase 0 scope).

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml tests/smoke
git commit -m "ci: lint+unit+integration+smoke gate; smoke harness"
```

---

## Self-Review (Phase 0)

- **Spec coverage:** vault layout (§5) ✔ via Task 11; OKF parser/validator (§22) ✔ Tasks 8–10; frontmatter + wikilink AST (§17 Phase 0) ✔ Tasks 2,3,5; `kg init`/`kg validate` ✔ Tasks 10–11; T11 partial + T10 placeholder ✔.
- **Type consistency:** uses master §3 names (`Note`, `Frontmatter`, `JsonEnvelope`, `KgError`) throughout; `check_okf`/`OkfReport` names stable into Phase 6 `kg validate` flags.
- **Placeholder scan:** no TBDs; every code step has full code.
- **Deferred (documented):** richer per-template seeding → Phase 6; full T10 `.kg` rebuild → Phase 1; full T11 ship/pull round-trip → Phase 6.
