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
    dir.join(".brain/.kg/brain.sqlite")
}

/// Embedding-write count proxy: the `embed` token record's input_tokens
/// is approximated as sum(body.len()/4). If only 1 note changed, the
/// incremental record's input_tokens must be much smaller than the full
/// index record's.
fn embed_input_tokens(dir: &Path) -> u64 {
    let metrics = std::fs::read_to_string(dir.join(".brain/.kg/metrics.log")).unwrap_or_default();
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
    std::fs::write(d.path().join(".brain/.kg/metrics.log"), "").unwrap();

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
    let note_path = d.path().join(".brain/notes/facts/f-postgres-primary.md");
    let original = std::fs::read_to_string(&note_path).unwrap();
    let edited = original.replace("primary datastore", "primary datastore (updated)");
    std::fs::write(&note_path, edited).unwrap();

    std::fs::write(d.path().join(".brain/.kg/metrics.log"), "").unwrap();

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
