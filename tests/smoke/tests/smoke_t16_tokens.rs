/// T16: kg index writes a token accounting record to .kg/metrics.log.
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
fn t16_index_writes_token_record() {
    let d = copy_fixture();

    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full"])
        .current_dir(d.path())
        .assert()
        .success();

    let log = std::fs::read_to_string(d.path().join(".kg/metrics.log")).unwrap();
    assert!(
        log.lines().any(|l| {
            l.contains("\"operation\":\"embed\"") && l.contains("\"command\":\"index\"")
        }),
        "metrics.log must contain an embed/index token record; got:\n{log}"
    );
}
