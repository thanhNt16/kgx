/// T01: raw/ files are never mutated after capture/index.
/// (Extraction is now harness-driven — there's no `kg extract` to test here.
/// The immutability guarantee is verified against `kg index`, which reads
/// raw sources but must never mutate them.)
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
fn t01_raw_hash_unchanged_after_index() {
    let d = copy_fixture();
    let raw = d.path().join(".brain/raw/2026-01-15-arch-review.md");
    let before = std::fs::read(&raw).unwrap();
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full"])
        .current_dir(d.path())
        .assert()
        .success();
    assert_eq!(before, std::fs::read(&raw).unwrap(), "raw file mutated");
}
