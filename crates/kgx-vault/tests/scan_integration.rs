use kgx_vault::scan::scan_vault;
use std::fs;

#[test]
fn scans_notes_and_raw_skipping_derived() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("notes/facts")).unwrap();
    fs::create_dir_all(root.join(".kg")).unwrap();
    fs::write(
        root.join("notes/facts/a.md"),
        "---\ntype: fact\nid: 01A\ntitle: A\n---\nx\n",
    )
    .unwrap();
    fs::write(
        root.join(".kg/ignore.md"),
        "---\ntype: fact\nid: 99\ntitle: NO\n---\n",
    )
    .unwrap();
    let notes = scan_vault(root).unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].fm.id, "01A");
}
