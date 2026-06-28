/// T10: .kg rebuild is deterministic — same node count after deleting and re-indexing.
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

fn node_count(db: &Path) -> i64 {
    let c = rusqlite::Connection::open(db).unwrap();
    c.query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn t10_rebuild_is_deterministic() {
    let d = copy_fixture();

    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full"])
        .current_dir(d.path())
        .assert()
        .success();
    let n1 = node_count(&d.path().join(".kg/brain.sqlite"));

    std::fs::remove_dir_all(d.path().join(".kg")).unwrap();

    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full"])
        .current_dir(d.path())
        .assert()
        .success();
    let n2 = node_count(&d.path().join(".kg/brain.sqlite"));

    assert_eq!(n1, n2, "node count must be identical across rebuilds");
    assert_eq!(n1, 17, "fixture should index exactly 17 notes");
}
