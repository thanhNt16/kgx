use std::path::Path;

pub fn copy_fixture() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min");
    copy_dir(&src, d.path());
    d
}

fn copy_dir(src: &Path, dst_root: &Path) {
    for e in walkdir::WalkDir::new(src) {
        let e = e.unwrap();
        let rel = e.path().strip_prefix(src).unwrap();
        let dst = dst_root.join(rel);
        if e.file_type().is_dir() {
            std::fs::create_dir_all(&dst).unwrap();
        } else {
            std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
            std::fs::copy(e.path(), &dst).unwrap();
        }
    }
}

#[allow(dead_code)]
pub fn run_index(root: &Path) {
    assert_cmd::Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full"])
        .current_dir(root)
        .assert()
        .success();
}
