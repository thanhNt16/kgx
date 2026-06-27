use kgx_okf::validate::check_okf;
use std::fs;

fn vault_with(content: &str, name: &str) -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    fs::create_dir_all(d.path().join("notes/facts")).unwrap();
    fs::write(d.path().join("index.md"), "# Index\n").unwrap();
    fs::write(d.path().join("log.md"), "# Log\n").unwrap();
    fs::write(d.path().join(format!("notes/facts/{name}.md")), content).unwrap();
    d
}

#[test]
fn valid_vault_passes() {
    let d = vault_with(
        "---\ntype: fact\nid: 01A\ntitle: A\nvalid_from: 2026-01-01\n---\nx\n",
        "a",
    );
    let r = check_okf(d.path()).unwrap();
    assert!(r.ok, "expected ok, got {:?}", r.errors);
}

#[test]
fn bitemporal_violation_flagged() {
    // valid_to before valid_from is invalid
    let d = vault_with(
        "---\ntype: fact\nid: 01A\ntitle: A\nvalid_from: 2026-06-01\nvalid_to: 2026-01-01\n---\nx\n",
        "a",
    );
    let r = check_okf(d.path()).unwrap();
    assert!(!r.ok);
    assert!(r.errors.iter().any(|e| e.code == "bitemporal_order"));
}

#[test]
fn missing_reserved_file_flagged() {
    let d = tempfile::tempdir().unwrap();
    fs::create_dir_all(d.path().join("notes/facts")).unwrap();
    fs::write(
        d.path().join("notes/facts/a.md"),
        "---\ntype: fact\nid: 01A\ntitle: A\n---\nx\n",
    )
    .unwrap();
    let r = check_okf(d.path()).unwrap();
    assert!(r
        .errors
        .iter()
        .any(|e| e.code == "missing_reserved" && e.path.contains("index.md")));
}
