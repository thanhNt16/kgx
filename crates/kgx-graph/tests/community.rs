use kgx_graph::{build::build_full, community::detect, embed::MockEmbedder, Brain};
use kgx_vault::scan::scan_vault;

fn fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min/.brain")
}

#[test]
fn detect_produces_connected_communities_deterministically() {
    let notes = scan_vault(&fixture()).unwrap();
    let mut b1 = Brain::open_in_memory().unwrap();
    build_full(&mut b1, &notes, &MockEmbedder::new()).unwrap();
    let s1 = detect(&mut b1, 42).unwrap();
    let mut b2 = Brain::open_in_memory().unwrap();
    build_full(&mut b2, &notes, &MockEmbedder::new()).unwrap();
    let s2 = detect(&mut b2, 42).unwrap();
    assert_eq!(s1.count, s2.count);
    assert_eq!(s1.assignments, s2.assignments);
    assert!(s1.count >= 1);
    let cnt: i64 = b1
        .conn()
        .query_row("SELECT count(*) FROM communities", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cnt as usize, notes.len());
}
