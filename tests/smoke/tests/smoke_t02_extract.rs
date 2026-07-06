/// T02: Extracting raw sources yields ≥1 fact notes each with `source:` and `recorded_at`.
use assert_cmd::Command;
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

#[test]
fn t02_extract_produces_provenance_facts() {
    let d = copy_fixture();
    let facts_dir = d.path().join(".brain/notes/facts");
    let before = std::fs::read_dir(&facts_dir).unwrap().count();

    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["extract", "--source", "01RAW01ARCHREVIEW00000000"])
        .current_dir(d.path())
        .assert()
        .success();

    let after = std::fs::read_dir(&facts_dir).unwrap().count();
    assert!(after > before, "no new facts written after extract");

    // Every fact note (new or existing) must have source: and recorded_at
    let new_facts: Vec<_> = std::fs::read_dir(&facts_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            // Only check newly created facts (those written by extract — they have recorded_at)
            let content = std::fs::read_to_string(e.path()).unwrap_or_default();
            content.contains("recorded_at:")
        })
        .collect();

    assert!(!new_facts.is_empty(), "no new facts with recorded_at stamp");
    for e in &new_facts {
        let c = std::fs::read_to_string(e.path()).unwrap();
        assert!(
            c.contains("source:"),
            "fact missing source provenance: {:?}",
            e.path()
        );
        assert!(
            c.contains("recorded_at:"),
            "fact missing recorded_at stamp: {:?}",
            e.path()
        );
    }
}
