use kgx_dream::{
    context::DreamContext,
    run::{dream, DreamOptions},
    PassId,
};
use kgx_graph::{build::build_full, embed::MockEmbedder, Brain};
use kgx_llm::mock::MockProvider;
use kgx_vault::scan::scan_vault;

fn fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min")
}

#[tokio::test]
async fn dream_respects_max_iterations() {
    let notes = scan_vault(&fixture()).unwrap();
    let mut b = Brain::open_in_memory().unwrap();
    build_full(&mut b, &notes, &MockEmbedder::new()).unwrap();
    let p = MockProvider::new();
    let e = MockEmbedder::new();
    let ctx = DreamContext {
        notes: &notes,
        brain: &b,
        provider: &p,
        embedder: &e,
    };
    let r = dream(
        &ctx,
        DreamOptions {
            passes: PassId::all().to_vec(),
            max_iterations: 3,
        },
    )
    .await
    .unwrap();
    assert!(
        r.iterations <= 3,
        "exceeded max_iterations: {}",
        r.iterations
    );
    assert!(!r.diffs.is_empty(), "expected at least some diffs");
}

#[tokio::test]
async fn dream_converges_on_empty_input() {
    // With max_iterations=1 and no passes, should complete with done_signal
    let notes = scan_vault(&fixture()).unwrap();
    let mut b = Brain::open_in_memory().unwrap();
    build_full(&mut b, &notes, &MockEmbedder::new()).unwrap();
    let p = MockProvider::new();
    let e = MockEmbedder::new();
    let ctx = DreamContext {
        notes: &notes,
        brain: &b,
        provider: &p,
        embedder: &e,
    };
    let r = dream(
        &ctx,
        DreamOptions {
            passes: vec![PassId::Community], // community returns empty since no data
            max_iterations: 1,
        },
    )
    .await
    .unwrap();
    assert!(r.iterations <= 1);
    // Community pass returns empty on empty table → done_signal should be true
    assert!(
        r.done_signal,
        "community pass on empty table should signal done"
    );
}
