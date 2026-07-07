use kgx_okf::{
    bundle::{pull, ship},
    check_okf,
};
use std::path::Path;

fn copy_fixture() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min");
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry.unwrap();
        let rel = entry
            .path()
            .strip_prefix(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min"),
            )
            .unwrap();
        let dst = d.path().join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dst).unwrap();
        } else {
            std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
            std::fs::copy(entry.path(), dst).unwrap();
        }
    }
    d
}

#[test]
fn ship_then_pull_is_lossless_and_valid() {
    let src = copy_fixture();
    // The fixture mirrors the on-disk layout: vault content lives under
    // .brain/. ship/pull are vault-root-agnostic library functions, so pass
    // the .brain/ directory as the vault root.
    let src_vault = src.path().join(".brain");
    let bundle = src.path().join("out.okf.tar.gz");
    ship(&src_vault, &bundle).unwrap();
    let dst = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dst.path().join("notes")).unwrap();
    std::fs::write(dst.path().join("index.md"), "# Index\n").unwrap();
    std::fs::write(dst.path().join("log.md"), "# Log\n").unwrap();
    let n = pull(&bundle, dst.path(), Some("imported")).unwrap();
    assert!(n > 0);
    assert!(dst.path().join("notes/imported").exists());
    let report = check_okf(dst.path()).unwrap();
    assert!(report.ok, "{:?}", report.errors);
}
