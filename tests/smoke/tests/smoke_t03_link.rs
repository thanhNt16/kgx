/// T03: kg link integrity — phantom count is 0 for the fixture.
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
fn t03_link_phantoms_zero_for_fixture() {
    let d = copy_fixture();
    let out = Command::cargo_bin("kg")
        .unwrap()
        .args(["link", "--json"])
        .current_dir(d.path())
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    let phantoms = v["data"]["phantoms"].as_u64().unwrap();
    assert_eq!(
        phantoms, 0,
        "expected 0 phantom links in fixture, got {phantoms}"
    );
}
