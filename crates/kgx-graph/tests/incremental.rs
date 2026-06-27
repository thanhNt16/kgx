use kgx_graph::{
    build::{build_full, build_incremental},
    embed::MockEmbedder,
    pagerank, Brain,
};
use kgx_vault::scan::scan_vault;

fn fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min")
}

#[test]
fn incremental_matches_full_for_single_change() {
    let notes = scan_vault(&fixture()).unwrap();
    let mut full = Brain::open_in_memory().unwrap();
    build_full(&mut full, &notes, &MockEmbedder::new()).unwrap();
    let mut inc = Brain::open_in_memory().unwrap();
    build_full(&mut inc, &notes, &MockEmbedder::new()).unwrap();
    let changed = vec![notes[0].fm.id.clone()];
    build_incremental(&mut inc, &notes, &changed, &MockEmbedder::new()).unwrap();
    let n_full: i64 = full
        .conn()
        .query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
        .unwrap();
    let n_inc: i64 = inc
        .conn()
        .query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n_full, n_inc);
}

#[test]
fn pagerank_writes_scores() {
    let notes = scan_vault(&fixture()).unwrap();
    let mut b = Brain::open_in_memory().unwrap();
    build_full(&mut b, &notes, &MockEmbedder::new()).unwrap();
    pagerank::compute(&mut b, 0.85, 20).unwrap();
    let cnt: i64 = b
        .conn()
        .query_row("SELECT count(*) FROM pagerank WHERE score > 0", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(cnt > 0);
}
