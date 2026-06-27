use assert_cmd::Command;
use std::fs;

fn copy_fixture() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/vault-min");
    // shallow recursive copy
    for e in walkdir::WalkDir::new(&src) {
        let e = e.unwrap();
        let rel = e.path().strip_prefix(&src).unwrap();
        let dst = d.path().join(rel);
        if e.file_type().is_dir() {
            fs::create_dir_all(&dst).unwrap();
        } else {
            fs::create_dir_all(dst.parent().unwrap()).unwrap();
            fs::copy(e.path(), &dst).unwrap();
        }
    }
    d
}

#[test]
fn validate_json_reports_ok_on_fixture() {
    let d = copy_fixture();
    let out = Command::cargo_bin("kg")
        .unwrap()
        .args(["validate", "--okf", "--json"])
        .current_dir(d.path())
        .assert()
        .success();
    let v: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["command"], "validate");
    assert_eq!(v["ok"], true);
}
