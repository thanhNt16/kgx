use kgx_graph::{build::build_full, embed::MockEmbedder, Brain};
use kgx_vault::scan::scan_vault;

fn fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min")
}

#[test]
fn leiden_deterministic_with_seed() {
    let notes = scan_vault(&fixture()).unwrap();
    let mut b1 = Brain::open_in_memory().unwrap();
    build_full(&mut b1, &notes, &MockEmbedder::new()).unwrap();
    let s1 = kgx_graph::leiden::detect(&mut b1, 42).unwrap();
    let mut b2 = Brain::open_in_memory().unwrap();
    build_full(&mut b2, &notes, &MockEmbedder::new()).unwrap();
    let s2 = kgx_graph::leiden::detect(&mut b2, 42).unwrap();
    assert_eq!(s1.count, s2.count);
    assert_eq!(s1.assignments, s2.assignments);
    assert!(s1.count >= 1);
    let cnt: i64 = b1
        .conn()
        .query_row("SELECT count(*) FROM communities", [], |r| r.get(0))
        .unwrap();
    // Leiden excludes `type: moc` notes (derived artifacts that would
    // otherwise form singleton communities and create a feedback loop with
    // --communities MOC generation). The fixture has 1 MOC (datastore-moc),
    // so communities covers notes.len() - moc_count rows.
    let non_moc_count = notes
        .iter()
        .filter(|n| !matches!(n.fm.r#type, kgx_core::NoteType::Moc))
        .count();
    assert_eq!(
        cnt as usize, non_moc_count,
        "communities table must have one row per non-MOC note"
    );
}

#[test]
fn leiden_different_seed_may_differ() {
    let notes = scan_vault(&fixture()).unwrap();
    let mut b = Brain::open_in_memory().unwrap();
    build_full(&mut b, &notes, &MockEmbedder::new()).unwrap();
    let s1 = kgx_graph::leiden::detect(&mut b, 42).unwrap();
    let s2 = kgx_graph::leiden::detect(&mut b, 99).unwrap();
    // Different seeds may produce different partitions (or same by coincidence)
    // At minimum, both should produce valid partitions
    assert!(s1.count >= 1);
    assert!(s2.count >= 1);
}
