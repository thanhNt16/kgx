/// T04: Orphan detection — exactly 1 orphan with id 01FACT05ORPHAN0000000000.
// TODO T09: criterion benchmark for hybrid vs semantic recall — see tests/fixtures/qa.json
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
fn t04_exactly_one_orphan() {
    let d = copy_fixture();
    let out = Command::cargo_bin("kg")
        .unwrap()
        .args(["link", "--orphans", "--json"])
        .current_dir(d.path())
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    let orph = v["data"]["orphans"].as_array().unwrap();
    assert_eq!(orph.len(), 1, "expected exactly 1 orphan, got: {:?}", orph);
    assert_eq!(
        orph[0].as_str().unwrap(),
        "01FACT05ORPHAN0000000000",
        "wrong orphan id"
    );
}
