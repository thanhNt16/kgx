# KGX Phase 0 — Real Bugs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the four verified correctness bugs blocking honest benchmarks: (1) `--incremental` re-embeds every note, (2) `kg index --communities` multiplies MOC notes on each run, (3) `ingest_file` advertises SHA-256 but uses SipHash, (4) `query_memory.project` filter is silently ignored — then add the missing end-to-end test for real semantic embeddings.

**Architecture:** Surgical edits to five files, plus three new tests and one upgraded test. No schema changes, no new crates, no new dependencies except `sha2` (already in the transitive dep graph via other Rust tooling). Every fix is gated by a failing test that reproduces the bug as observed on 2026-07-04.

**Tech Stack:** Rust 2021, `rusqlite`, `sha2`, `assert_cmd` (integration tests), `KGX_LLM=mock` for hermetic test runs, optional `--features kgx-cli/semantic` + `KGX_EMBED=fastembed` for the new semantic e2e test.

---

## Why this plan exists (audit context)

The original `2026-07-02-kgx-unified-rewrite.md` "ground truth" section listed many bugs. **Re-verification on 2026-07-04 against the actual code showed most were already fixed**:

| Alleged bug | Status on 2026-07-04 | Evidence |
|---|---|---|
| PageRank dangling-node mass leak | **Already fixed** | `pagerank.rs:43-60` redistributes `dangling_mass / n`; verified `sum(score) = 1.000000` on `vault-min` |
| `build_full` doesn't clear derived tables | **Already fixed** | `build.rs:156-161` DELETEs notes, edges, fts, pagerank, communities, summaries, notes_vec |
| `meta` table never written | **Already fixed** | `build.rs:112-145` `write_meta()` writes 4 keys |
| Communities = connected-components | **Already fixed** | `leiden.rs` is real modularity-gain local-moving; verified 4 distinct communities on fixture |
| KNN brute-force O(N) | **Already fixed** | `knn.rs:46-48` delegates to vec0 `knn_search` when `notes_vec` exists |
| Mock embeddings only | **Partly fixed** | `FastEmbedEmbedder` exists behind `--features semantic`; works, but ships disabled and untested e2e |

**This plan therefore targets only the four bugs that are still real, plus the one missing test.** Each was reproduced empirically before a line of the plan was written.

---

## File Structure

**Modify (5 files, surgical):**
- `crates/kgx-cli/src/commands/index.rs` — fix `find_changed_ids` (Task 1) + deterministic MOC IDs (Task 3)
- `crates/kgx-mcp/src/tools/ingest_file.rs` — replace SipHash with real SHA-256 (Task 4)
- `crates/kgx-mcp/src/tools/query.rs` — honor `project` filter (Task 5)
- `crates/kgx-mcp/Cargo.toml` — add `sha2` dependency (Task 4)
- `tests/smoke/tests/smoke_t10_rebuild.rs` — upgrade assertion to byte-hash (Task 2)

**Create (3 test files):**
- `tests/smoke/tests/smoke_t19_incremental.rs` — incremental touches only changed rows (Task 1)
- `tests/smoke/tests/smoke_t25_moc_idempotent.rs` — `--communities` is idempotent across runs (Task 3)
- `tests/smoke/tests/smoke_t26_ingest_sha256.rs` — `ingest_file` hash is real SHA-256 (Task 4)

**Create (1 test file, optional feature gate):**
- `crates/kgx-graph/tests/semantic_e2e.rs` — real fastembed returns semantically-relevant results (Task 6)

**Modify (1 doc):**
- `tests/smoke/tests/smoke_t04_orphan.rs` — remove the stale `// TODO T09` comment marker now that T19/T27 cover benchmarking (Task 7)

---

## Task 1: Fix `find_changed_ids` — incremental re-embeds everything

**Bug:** `crates/kgx-cli/src/commands/index.rs:86-114` computes the changed set as `added ∪ removed ∪ intersection`, where `intersection` is *every existing note still present*. So every `--incremental` run flags all current notes as changed and re-embeds them. Verified: touched nothing, ran `kg index --incremental`, observed "indexed 21 nodes" (re-embedded all).

**Root cause:** Set arithmetic is wrong. `intersection` should not be in the changed set at all — presence in both sides means *unchanged unless content differs*.

**Fix:** Compare a content hash per note, not set membership. A note is "changed" iff it's new, removed, or its `raw_text` hash differs from what's stored.

**Files:**
- Modify: `crates/kgx-cli/src/commands/index.rs:86-114` (rewrite `find_changed_ids`)
- Test: `tests/smoke/tests/smoke_t19_incremental.rs` (new)

- [ ] **Step 1: Write the failing test**

Create `tests/smoke/tests/smoke_t19_incremental.rs`:

```rust
/// T19: --incremental touches only notes whose content actually changed.
/// Reproduces the bug: today --incremental re-embeds every existing note.
use assert_cmd::Command;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

fn copy_fixture() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min");
    for e in walkdir::WalkDir::new(&src) {
        let e = e.unwrap();
        let rel = e.path().strip_prefix(&src).unwrap();
        let dst = d.path().join(rel);
        if e.file_type().is_dir() {
            std::fs::create_dir_all(&dst).unwrap();
        } else {
            std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
            std::fs::copy(e.path(), &dst).unwrap();
        }
    }
    d
}

fn brain_path(dir: &Path) -> PathBuf {
    dir.join(".kg/brain.sqlite")
}

/// Embedding-write count proxy: the `embed` token record's input_tokens
/// is approximated as sum(body.len()/4). If only 1 note changed, the
/// incremental record's input_tokens must be much smaller than the full
/// index record's.
fn embed_input_tokens(dir: &Path) -> u64 {
    let metrics = std::fs::read_to_string(dir.join(".kg/metrics.log")).unwrap_or_default();
    let mut last_embed = 0u64;
    for line in metrics.lines() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            if v["command"] == "index" && v["operation"] == "embed" {
                last_embed = v["input_tokens"].as_u64().unwrap_or(0);
            }
        }
    }
    last_embed
}

#[test]
fn t19_incremental_only_touches_changed_notes() {
    let d = copy_fixture();

    // Baseline: full index
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full"])
        .current_dir(d.path())
        .assert()
        .success();
    let full_tokens = embed_input_tokens(d.path());
    assert!(full_tokens > 0, "full index must record embed tokens");

    // Clear metrics log so the next reading isolates the incremental run
    std::fs::write(d.path().join(".kg/metrics.log"), "").unwrap();

    // Touch NOTHING. Re-index incremental. Should embed ~0 notes.
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--incremental"])
        .current_dir(d.path())
        .assert()
        .success();

    let incr_tokens = embed_input_tokens(d.path());
    assert!(
        incr_tokens < full_tokens / 4,
        "incremental with no changes must embed far less than full. \
         got incr={incr_tokens}, full={full_tokens}"
    );
}

#[test]
fn t19_incremental_picks_up_edited_note() {
    let d = copy_fixture();
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full"])
        .current_dir(d.path())
        .assert()
        .success();

    // Edit exactly one note's body
    let note_path = d.path().join("notes/facts/f-postgres-primary.md");
    let original = std::fs::read_to_string(&note_path).unwrap();
    let edited = original.replace("primary datastore", "primary datastore (updated)");
    std::fs::write(&note_path, edited).unwrap();

    std::fs::write(d.path().join(".kg/metrics.log"), "").unwrap();

    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--incremental"])
        .current_dir(d.path())
        .assert()
        .success();

    // At least one note was re-embedded (token count > 0), but still
    // far less than re-embedding all 17.
    let incr_tokens = embed_input_tokens(d.path());
    assert!(incr_tokens > 0, "editing a note must trigger re-embed");
    assert!(
        incr_tokens < 200,
        "single-note re-embed should be tiny; got {incr_tokens}"
    );

    // And the brain reflects the edit
    let conn = Connection::open(brain_path(d.path())).unwrap();
    let body: String = conn
        .query_row(
            "SELECT raw_text FROM notes WHERE id LIKE '01FACT01%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(body.contains("(updated)"), "edited body must be in brain");
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
KGX_LLM=mock cargo test --package smoke --test smoke_t19_incremental -- --test-threads=1
```

Expected: FAIL on `t19_incremental_only_touches_changed_notes` with "incremental with no changes must embed far less than full. got incr=NNNN, full=NNNN" (incr ≈ full today).

- [ ] **Step 3: Rewrite `find_changed_ids`**

Replace `crates/kgx-cli/src/commands/index.rs:86-114` (the entire `find_changed_ids` function) with:

```rust
fn find_changed_ids(brain: &Brain, notes: &[Note]) -> anyhow::Result<Vec<String>> {
    use std::collections::BTreeMap;
    // Pull the stored content fingerprint per note id. We hash the
    // concatenation of title + "\n" + body, matching what gets embedded
    // (build.rs uses format!("{}\n{}", title, body)).
    let stored: BTreeMap<String, u64> = {
        let mut stmt = brain
            .conn()
            .prepare("SELECT id, raw_text FROM notes")
            .map_err(|e| kgx_core::KgError::Brain(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| {
                let id: String = r.get(0)?;
                let body: String = r.get::<_, String>(1).unwrap_or_default();
                // We can't recover the original title from raw_text alone
                // (raw_text is just the body), but body-only hashing is
                // sufficient to detect edits — title edits without a body
                // change are vanishingly rare and the next --full catches them.
                Ok((id, hash_str(&body)))
            })
            .map_err(|e| kgx_core::KgError::Brain(e.to_string()))?;
        let mut m = BTreeMap::new();
        for r in rows {
            let (id, h) = r.map_err(|e| kgx_core::KgError::Brain(e.to_string()))?;
            m.insert(id, h);
        }
        m
    };

    let mut changed = Vec::new();
    for n in notes {
        let cur_hash = hash_str(&n.body);
        match stored.get(n.fm.id.as_str()) {
            None => changed.push(n.fm.id.clone()),      // new note
            Some(prev) if *prev != cur_hash => changed.push(n.fm.id.clone()), // edited
            Some(_) => { /* unchanged */ }
        }
    }
    // Removed notes (in brain, not in vault) don't need re-embedding —
    // they'll be pruned by build_incremental's full edge recompute, and
    // the next --full cleans them fully. Don't add them to changed (nothing
    // to embed).
    changed.sort();
    changed.dedup();
    Ok(changed)
}

fn hash_str(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}
```

- [ ] **Step 4: Run the test to verify it passes**

```bash
KGX_LLM=mock cargo test --package smoke --test smoke_t19_incremental -- --test-threads=1
```

Expected: PASS (both `t19_incremental_only_touches_changed_notes` and `t19_incremental_picks_up_edited_note`).

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-cli/src/commands/index.rs tests/smoke/tests/smoke_t19_incremental.rs
git commit -m "fix(index): --incremental only re-embeds changed notes (content-hash diff)

find_changed_ids now compares a body hash per note against the stored
raw_text hash, instead of unioning added+removed+intersection (which
flagged every existing note as changed). T19 covers both the no-op and
single-edit cases."
```

---

## Task 2: Upgrade T10 to byte-hash equality (catches silent score drift)

**Bug:** Today's `smoke_t10_rebuild.rs:52-53` only asserts `node_count` equality. A bug that silently shifts PageRank scores (e.g. a non-deterministic ordering) would pass T10. The PRD calls for byte-hash equality across rebuilds.

**Files:**
- Modify: `tests/smoke/tests/smoke_t10_rebuild.rs:1-54`

- [ ] **Step 1: Add a brain-hash helper and tighten the assertion**

Replace the entire contents of `tests/smoke/tests/smoke_t10_rebuild.rs` with:

```rust
/// T10: .kg rebuild is deterministic — identical brain bytes across rebuilds.
/// Upgraded from count-only to byte-hash equality across the notes table,
/// pagerank, communities, and notes_vec contents.
use assert_cmd::Command;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

fn copy_fixture() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min");
    for e in walkdir::WalkDir::new(&src) {
        let e = e.unwrap();
        let rel = e.path().strip_prefix(&src).unwrap();
        let dst = d.path().join(rel);
        if e.file_type().is_dir() {
            std::fs::create_dir_all(&dst).unwrap();
        } else {
            std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
            std::fs::copy(e.path(), &dst).unwrap();
        }
    }
    d
}

fn brain_path(dir: &Path) -> PathBuf {
    dir.join(".kg/brain.sqlite")
}

/// Canonical content hash of the brain's derived tables.
/// Ordering is forced deterministic by ORDER BY on primary keys.
fn brain_fingerprint(p: &Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let conn = Connection::open(p).unwrap();
    let mut out = String::new();

    let mut concat = |sql: &str, out: &mut String| {
        let mut stmt = conn.prepare(sql).unwrap();
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            for i in 0..row.column_count() {
                let v: String = row
                    .get_ref(i)
                    .map(|r| r.as_str().map(|s| s.to_string()).unwrap_or_else(|_| format!("{:?}", r)));
                out.push_str(&v);
                out.push('|');
            }
            out.push('\n');
        }
    };
    concat("SELECT id, type, status, tags, raw_text FROM notes ORDER BY id", &mut out);
    concat("SELECT src_id, dst_id, rel_type FROM edges ORDER BY src_id, dst_id, rel_type", &mut out);
    concat("SELECT id, score FROM pagerank ORDER BY id", &mut out);
    concat("SELECT id, community_id FROM communities ORDER BY id", &mut out);

    let mut h = DefaultHasher::new();
    out.hash(&mut h);
    format!("{:016x}", h.finish())
}

#[test]
fn t10_rebuild_is_deterministic() {
    let d = copy_fixture();

    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full", "--communities", "--pagerank"])
        .current_dir(d.path())
        .assert()
        .success();
    let fp1 = brain_fingerprint(&brain_path(d.path()));

    std::fs::remove_dir_all(d.path().join(".kg")).unwrap();

    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full", "--communities", "--pagerank"])
        .current_dir(d.path())
        .assert()
        .success();
    let fp2 = brain_fingerprint(&brain_path(d.path()));

    assert_eq!(fp1, fp2, "brain fingerprint must match across rebuilds");
}
```

Note: we do **not** assert `notes_vec` byte-equality because vec0 internal storage order is not guaranteed deterministic by sqlite-vec; we assert the four canonical tables whose contents are fully under our control.

- [ ] **Step 2: Run the test to verify it passes**

```bash
KGX_LLM=mock cargo test --package smoke --test smoke_t10_rebuild -- --test-threads=1
```

Expected: PASS. (If it FAILS, that itself is a real determinism bug to file — Leiden uses `seed=42` and PageRank is order-stable, so it should pass. If it doesn't, do not proceed to Task 3; investigate the non-determinism first.)

- [ ] **Step 3: Commit**

```bash
git add tests/smoke/tests/smoke_t10_rebuild.rs
git commit -m "test(t10): upgrade to byte-hash brain fingerprint across rebuilds

Was count-only, which would pass even if PageRank or Leiden produced
different scores across runs. Now hashes notes/edges/pagerank/communities
content so any non-determinism fails the gate."
```

---

## Task 3: Make `kg index --communities` idempotent (MOC feedback loop)

**Bug:** `crates/kgx-cli/src/commands/index.rs:42-56` writes one MOC note per community, generating a **fresh ULID** for each on every run. So each `kg index --communities` creates new MOC notes, the old ones become orphans, and node count grows unboundedly. Verified: ran `kg index --full --communities` on `vault-min` (17 notes) → node count became 21 (4 community MOCs added); next run would add 4 more.

**Fix:** Derive the MOC note id deterministically from the community id, so re-writing the same community overwrites the same note instead of creating a new one.

**Files:**
- Modify: `crates/kgx-cli/src/commands/index.rs:42-56` (the MOC write block)
- Test: `tests/smoke/tests/smoke_t25_moc_idempotent.rs` (new)

- [ ] **Step 1: Write the failing test**

Create `tests/smoke/tests/smoke_t25_moc_idempotent.rs`:

```rust
/// T25: kg index --communities is idempotent across runs.
/// Reproduces the MOC feedback loop: today each run generates fresh ULIDs
/// for community MOCs, so node count grows by (num_communities) each run.
use assert_cmd::Command;
use rusqlite::Connection;
use std::path::Path;

fn copy_fixture() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min");
    for e in walkdir::WalkDir::new(&src) {
        let e = e.unwrap();
        let rel = e.path().strip_prefix(&src).unwrap();
        let dst = d.path().join(rel);
        if e.file_type().is_dir() {
            std::fs::create_dir_all(&dst).unwrap();
        } else {
            std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
            std::fs::copy(e.path(), &dst).unwrap();
        }
    }
    d
}

fn moc_count(db: &std::path::Path) -> i64 {
    let c = Connection::open(db).unwrap();
    // MOCs are written as notes with path under notes/moc/ — count those,
    // since the type column may be a serialized enum string.
    c.query_row(
        "SELECT count(*) FROM notes WHERE path LIKE 'notes/moc/%'",
        [],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn t25_communities_moc_idempotent() {
    let d = copy_fixture();

    // First run: creates N community MOCs
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full", "--communities"])
        .current_dir(d.path())
        .assert()
        .success();
    let n1 = moc_count(&d.path().join(".kg/brain.sqlite"));

    // Delete .kg and redo: MOC notes also live on disk under notes/moc/,
    // so a fresh brain rebuild must see exactly the SAME count of MOCs
    // (not 2x).
    std::fs::remove_dir_all(d.path().join(".kg")).unwrap();
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full", "--communities"])
        .current_dir(d.path())
        .assert()
        .success();
    let n2 = moc_count(&d.path().join(".kg/brain.sqlite"));

    assert!(n1 > 0, "first --communities run must produce MOCs");
    assert_eq!(n1, n2, "MOC count must be stable across runs (got {n1} then {n2})");
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
KGX_LLM=mock cargo test --package smoke --test smoke_t25_moc_idempotent -- --test-threads=1
```

Expected: FAIL on the second-run assertion (today, MOC files are written to `notes/moc/` on disk with fresh IDs each run, and old files accumulate — `n2 > n1`).

- [ ] **Step 3: Make MOC note IDs deterministic**

In `crates/kgx-cli/src/commands/index.rs`, replace the MOC write block (lines 42-56, the `for summary in summaries { ... }` loop) with a version that derives a stable id from `community_id` and overwrites in place:

```rust
        let moc_dir = root.join("notes/moc");
        std::fs::create_dir_all(&moc_dir)?;
        for summary in summaries {
            let body = format!("{}\n\nMembers: {}", summary.summary, summary.member_count);
            let path = moc_dir.join(format!("community-{}.md", summary.community_id));
            // Deterministic id: MOC notes are derived artifacts, so a stable
            // id keyed on community_id makes --communities idempotent across
            // runs. ULID format is 26 chars of Crockford base32; we pad the
            // community id into the tail so it sorts after real notes
            // (which start with '01...'). 'MOC' prefix in the data segment
            // keeps it human-recognizable.
            let moc_id = format!("01MOC{:019}{}", summary.community_id, summary.community_id)
                .chars()
                .take(26)
                .collect::<String>();
            std::fs::write(
                path,
                format!(
                    "---\ntype: moc\nid: {}\ntitle: \"{}\"\ntags: [entrypoint, community]\ncreated_by: agent\ncreated_via: cli\n---\n{}\n",
                    moc_id,
                    summary.title.replace('"', "\\\""),
                    body
                ),
            )?;
        }
```

- [ ] **Step 4: Run the test to verify it passes**

```bash
KGX_LLM=mock cargo test --package smoke --test smoke_t25_moc_idempotent -- --test-threads=1
```

Expected: PASS. If the MOC id format collides with the ULID validation elsewhere, check `kgx-core/src/util.rs` for `new_ulid`/parsing — MOC ids only need to be unique strings, not valid ULIDs, so a 26-char alphanumeric string is fine.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-cli/src/commands/index.rs tests/smoke/tests/smoke_t25_moc_idempotent.rs
git commit -m "fix(index): deterministic community MOC ids (--communities idempotent)

Was generating fresh ULIDs per MOC per run, causing unbounded node growth
(17 -> 21 -> 25 ...). Now derives a stable id from community_id so re-run
overwrites the same note. T25 covers idempotency across two --full runs."
```

---

## Task 4: Replace SipHash with real SHA-256 in `ingest_file`

**Bug:** `crates/kgx-mcp/src/tools/ingest_file.rs:49-55` advertises "idempotent by sha256" (in the tool description and the `hash` field) but uses `std::collections::hash_map::DefaultHasher`, which is SipHash 13 — a 64-bit non-cryptographic hash. SipHash is fine for hashmap keys but (a) the docstring lies, and (b) two different contents could in principle collide. The MCP tool also writes `hash: {hash}` into the frontmatter, so the on-disk record is misleading.

**Fix:** Use `sha2::Sha256`. The hash becomes a real 64-char hex string. Update the frontmatter write and the description stays truthful.

**Files:**
- Modify: `crates/kgx-mcp/Cargo.toml` (add `sha2`)
- Modify: `crates/kgx-mcp/src/tools/ingest_file.rs:1-55`
- Test: `tests/smoke/tests/smoke_t26_ingest_sha256.rs` (new — but tests the lib directly since the binary doesn't expose `ingest_file`; use the existing `tools_dispatch.rs` pattern)

- [ ] **Step 1: Add the `sha2` dependency**

In `crates/kgx-mcp/Cargo.toml`, under `[dependencies]`, add:

```toml
sha2 = "0.10"
```

Run `cargo metadata` check:

```bash
cargo check -p kgx-mcp
```

Expected: compiles (sha2 may already be in the lockfile transitively).

- [ ] **Step 2: Write the failing test**

Create `tests/smoke/tests/smoke_t26_ingest_sha256.rs`:

```rust
/// T26: ingest_file's hash is real SHA-256 (64 hex chars), not SipHash (16 hex chars).
//! This is a doc-test of the kgx-mcp crate via its public tool dispatch.
//! Run with: cargo test --package smoke --test smoke_t26_ingest_sha256
use serde_json::json;

#[test]
fn t26_ingest_file_hash_is_sha256() {
    // We can't easily spin the kg binary's MCP server from a smoke test,
    // so call the library function directly. kgx-mcp exposes
    // kgx_mcp::tools::dispatch (or we call the ingest_file module directly).
    // Use the public dispatch path the same way protocol.rs does.
    let tmp = tempfile::tempdir().unwrap();
    let args = json!({
        "content": "hello world\nthis is a test source",
    });
    let result = kgx_mcp::tools::ingest_file::run(tmp.path(), &args).unwrap();
    let hash = result["hash"].as_str().expect("hash field present");
    assert_eq!(
        hash.len(),
        64,
        "SHA-256 hex digest is 64 chars; got {} ({}). SipHash would be 16.",
        hash,
        hash.len()
    );
    // Known SHA-256 of "hello world\nthis is a test source"
    // (computed independently): verify determinism by re-ingesting.
    let result2 = kgx_mcp::tools::ingest_file::run(tmp.path(), &args).unwrap();
    // Second call hits the path-exists skip branch — but the hash field
    // should still be present and equal.
    assert_eq!(result2["hash"].as_str(), Some(hash));
}
```

Note: this test requires `kgx-mcp` to be a `dev-dependency` of the `smoke` test crate. If it isn't yet, add to `tests/smoke/Cargo.toml`:

```toml
[dev-dependencies]
kgx-mcp = { path = "../../crates/kgx-mcp" }
```

And expose the module by ensuring `crates/kgx-mcp/src/lib.rs` re-exports `pub mod tools;` (it already declares `mod tools;` — change to `pub mod tools;` for the test, and `pub mod tools::ingest_file;` accordingly). Check `lib.rs` first and only change visibility if needed.

- [ ] **Step 3: Run the test to verify it fails**

```bash
cargo test --package smoke --test smoke_t26_ingest_sha256 -- --test-threads=1
```

Expected: FAIL with "SHA-256 hex digest is 64 chars; got <16-char SipHash> (16)".

- [ ] **Step 4: Replace the hash function**

In `crates/kgx-mcp/src/tools/ingest_file.rs`, replace the `sha256` function (lines 49-55) with a real one:

```rust
fn sha256(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    format!("{:x}", hasher.finalize())
}
```

Also update the file-top comment (line 1) — it's already accurate ("idempotent by sha256") so no change needed there. The frontmatter write on line 36 already references `{hash}` and stays correct.

- [ ] **Step 5: Run the test to verify it passes**

```bash
cargo test --package smoke --test smoke_t26_ingest_sha256 -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/kgx-mcp/Cargo.toml crates/kgx-mcp/src/tools/ingest_file.rs \
        crates/kgx-mcp/src/lib.rs tests/smoke/Cargo.toml \
        tests/smoke/tests/smoke_t26_ingest_sha256.rs
git commit -m "fix(mcp): ingest_file uses real SHA-256, not SipHash

The tool description and frontmatter both said 'sha256' but the impl used
std DefaultHasher (SipHash 13, 64-bit). Now uses sha2::Sha256 (64 hex chars).
T26 verifies the digest length and determinism."
```

---

## Task 5: Honor `query_memory.project` filter (or remove it honestly)

**Bug:** `crates/kgx-mcp/src/tools/mod.rs:18` declares `project` in the `query_memory` input schema, but `crates/kgx-mcp/src/tools/query.rs:5-41` never reads `args["project"]`. The filter is silently ignored.

**Decision:** Per-project brains (PRD Phase 4) don't exist yet — there's only one brain today. So we cannot *meaningfully* honor `project`. The honest fix is to **remove the field from the schema** rather than pretend to support it. Phase 4 reintroduces it with a real impl.

**Files:**
- Modify: `crates/kgx-mcp/src/tools/mod.rs:18` (drop `project` from the schema)
- Test: `tests/smoke/tests/t_skills_valid.rs` should still pass (it asserts the tool *names*, not input fields — verify)

- [ ] **Step 1: Read the current schema line**

```bash
grep -n '"project"' crates/kgx-mcp/src/tools/mod.rs
```

- [ ] **Step 2: Remove `project` from the `query_memory` schema**

In `crates/kgx-mcp/src/tools/mod.rs`, find the `query_memory` schema entry (line ~18). Remove the `,"project":{"type":"string"}` fragment. The resulting properties object should read:

```json
"properties":{"note_type":{"type":"string"},"tag":{"type":"string"},"status":{"type":"string"},"limit":{"type":"integer"}}
```

- [ ] **Step 3: Verify no test references `project`**

```bash
grep -rn '"project"' tests/ crates/kgx-mcp/
```

Expected: no matches (or only matches in other tools that legitimately have a project concept — `ingest_*` don't). If `tests/smoke/tests/t_skills_valid.rs` references `project`, update it; otherwise no change.

- [ ] **Step 4: Run the full MCP test suite**

```bash
cargo test --package kgx-mcp
cargo test --package smoke --test t_skills_valid
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/kgx-mcp/src/tools/mod.rs
git commit -m "fix(mcp): drop advertized-but-ignored query_memory.project filter

Per-project brains (PRD Phase 4) don't exist yet, so the filter was a
lie. Removing it from the schema; Phase 4 will reintroduce it with a
real implementation."
```

---

## Task 6: Add real-embedding end-to-end test (closes the "semantic works" gap)

**Gap:** `FastEmbedEmbedder` exists behind `--features semantic` and the unit tests confirm it loads, but there's no test proving that *retrieval* with real embeddings returns semantically-relevant (non-keyword-overlap) results. The MockProvider's `is_semantic() == false` means today's hybrid search never even enters the vector branch (`hybrid.rs:134`).

**Fix:** Add an integration test, gated behind `--features semantic`, that builds a tiny vault with two notes that share no keywords but are semantically related, and asserts `kg search --mode semantic` returns the related note.

**Files:**
- Create: `crates/kgx-graph/tests/semantic_e2e.rs`

- [ ] **Step 1: Write the test (gated on the semantic feature)**

Create `crates/kgx-graph/tests/semantic_e2e.rs`:

```rust
//! T27: real fastembed embeddings produce semantically-relevant retrieval
//! that keyword search cannot match. Gated on the `semantic` feature.
#![cfg(feature = "semantic")]
#![cfg(test)]

use kgx_core::llm::Embedder;
use kgx_graph::embed::FastEmbedEmbedder;

#[test]
fn fastembed_returns_semantic_neighbors_not_keyword_overlap() {
    let e = FastEmbedEmbedder::load().expect("fastembed model must download on first run");
    assert_eq!(e.dim(), 384);
    assert!(e.is_semantic());

    // Two phrases with ZERO shared words but clear semantic similarity.
    let phrases = vec![
        "How do I store data in my application?".to_string(),      // query-ish
        "best practices for persisting information".to_string(),   // semantically close, no word overlap
        "the weather forecast for tomorrow".to_string(),           // unrelated distractor
    ];
    let embs = e.embed(&phrases).unwrap();
    let q = &embs[0];
    let persist = &embs[1];
    let weather = &embs[2];

    let cos = |a: &[f32], b: &[f32]| {
        let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
    };

    let sim_persist = cos(q, persist);
    let sim_weather = cos(q, weather);
    assert!(
        sim_persist > sim_weather,
        "semantic sim to 'persist' ({sim_persist:.3}) must exceed sim to 'weather' ({sim_weather:.3})"
    );
    assert!(
        sim_persist > 0.5,
        "semantically-related phrases should be >0.5 cosine; got {sim_persist:.3}"
    );
}
```

- [ ] **Step 2: Run the test with the semantic feature**

```bash
cargo test --package kgx-graph --features semantic --test semantic_e2e -- --test-threads=1
```

Expected: PASS (model downloads ~40MB on first run, cached in `~/.cache/fastembed/`). This test will be **skipped in CI by default** because the default features don't include `semantic`. That's intentional — the test is opt-in to keep the hermetic suite network-free. Document this in `README.md` test section.

- [ ] **Step 3: Document the opt-in**

Append to the `## Test Results` section of `README.md`:

```markdown
### Real-embedding test (opt-in)

The hermetic suite uses mock embeddings. To verify real fastembed
(all-MiniLM-L6-v2) retrieval end-to-end:

\`\`\`bash
cargo test --package kgx-graph --features semantic --test semantic_e2e
\`\`\`

Downloads ~40MB on first run, then cached. Not run in CI by default.
```

- [ ] **Step 4: Commit**

```bash
git add crates/kgx-graph/tests/semantic_e2e.rs README.md
git commit -m "test(graph): real fastembed semantic retrieval e2e (T27, opt-in)

Asserts that two phrases with zero word overlap ('store data' vs
'persist information') embed closer than an unrelated distractor. Gated
behind --features semantic so the hermetic suite stays network-free."
```

---

## Task 7: Remove the stale T09 TODO marker

**Cleanup:** `tests/smoke/tests/smoke_t04_orphan.rs:2` carries `// TODO T09: criterion benchmark for hybrid vs semantic recall`. With T19 (incremental) and T27 (semantic e2e) now landing, and the kill-criterion `kgx-bench` slated for Phase 7, the marker is stale and the bench is tracked elsewhere.

**Files:**
- Modify: `tests/smoke/tests/smoke_t04_orphan.rs:2`

- [ ] **Step 1: Delete the comment line**

In `tests/smoke/tests/smoke_t04_orphan.rs`, delete line 2:

```rust
// TODO T09: criterion benchmark for hybrid vs semantic recall — see tests/fixtures/qa.json
```

Leave the rest of the file (including the `/// T04:` doc comment) unchanged.

- [ ] **Step 2: Verify the workspace still has zero TODO/FIXME in production**

```bash
grep -rn 'TODO\|FIXME\|unimplemented!\|todo!\|panic!' crates/*/src/ || echo "clean"
```

Expected: `clean`.

- [ ] **Step 3: Commit**

```bash
git add tests/smoke/tests/smoke_t04_orphan.rs
git commit -m "chore(smoke): remove stale T09 TODO (covered by T19/T27 + Phase 7 kgx-bench)"
```

---

## Final gate: full workspace test run

- [ ] **Step 1: Run the entire hermetic suite**

```bash
KGX_LLM=mock cargo test --workspace -- --test-threads=1 2>&1 | tail -20
```

Expected: all suites pass. Count should now be **21 passed** (was 18, +T19a +T19b +T25 +T26; T10 upgraded in place; T27 is opt-in and skipped).

- [ ] **Step 2: Run the opt-in semantic test separately**

```bash
cargo test --package kgx-graph --features semantic --test semantic_e2e
```

Expected: PASS (network required for first-run model download).

- [ ] **Step 3: Verify production code is still marker-clean**

```bash
grep -rn 'TODO\|FIXME\|unimplemented!\|todo!\|panic!' crates/*/src/ && echo "REGRESSION" || echo "clean"
```

Expected: `clean`.

---

## Self-Review

**1. Spec coverage** — This plan implements the "Phase 0 — Foundation" goal from `docs/superpowers/specs/2026-07-04-kgx-unified-prd.md` §11 (acceptance gate: T10-upgrade byte-hash; T19 incremental). It also closes four of the items in PRD §5.2 "honest gaps" that survived re-verification:

- T19 → `find_changed_ids` correctness (was: every note re-embedded)
- T25 → MOC feedback loop (was: unbounded node growth)
- T26 → `ingest_file` hash honesty (was: SipHash advertised as SHA-256)
- T27 → real-embedding e2e (was: no test that semantic retrieval works)
- Task 5 → `query_memory.project` (was: silently ignored; honestly removed)
- T10-upgrade → determinism gate (was: count-only)

The PRD §5.2 items that I verified are *already fixed* (PageRank, build_full clears tables, meta written, real Leiden, vec0 ANN) are deliberately **not** re-touched — the audit table at the top of this plan documents why.

**2. Placeholder scan** — Every step contains the actual code or exact command. No "TBD", "implement later", "add error handling", or "similar to Task N". The one place a step says "check X first" (Task 4 Step 2, visibility of `tools::ingest_file`) is paired with the exact conditional instruction ("change `mod tools` to `pub mod tools`").

**3. Type consistency** — `find_changed_ids(brain: &Brain, notes: &[Note]) -> anyhow::Result<Vec<String>>` signature is unchanged (Task 1 calls it from `index.rs:24`). `hash_str(s: &str) -> u64` is a private helper. `sha256(s: &str) -> String` keeps the same signature (Task 4 only swaps the body). MOC id format `01MOC…` is a 26-char string consistent with the existing ULID-string usage in frontmatter `id:` fields. T19's `brain_path`/`embed_input_tokens` helpers and T25's `moc_count` helper are local to their test files and don't leak.

**4. Sequencing** — Tasks are independent and can be executed in any order, but the recommended order (1→2→3→4→5→6→7) keeps each commit self-contained and shippable. The final gate runs everything together.

**5. Risk notes** — Task 3 (MOC id format) is the only step with a real chance of unforeseen friction: if `kgx-core` validates note id format somewhere, the `01MOC…` id may be rejected. The plan calls this out in Task 3 Step 4 and tells the engineer to check `util.rs` first. Task 4's visibility change to `kgx-mcp/src/lib.rs` is the only structural modification; it's a one-keyword change (`mod` → `pub mod`) and reversible.
