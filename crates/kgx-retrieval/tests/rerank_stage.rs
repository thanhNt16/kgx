use kgx_graph::{build::build_full, rerank::MockReranker, Brain};
use kgx_retrieval::{search, Mode, Retrievers, SearchOpts};
use kgx_vault::scan::scan_vault;

fn fixture_brain(dir: &std::path::Path) -> Brain {
    let notes_dir = dir.join("notes");
    std::fs::create_dir_all(&notes_dir).unwrap();
    for (slug, title, body) in [
        ("a", "flink checkpoint tuning", "flink checkpoint interval set to 60s. See [[flink deployment]]"),
        ("b", "flink deployment", "flink runs on kubernetes"),
        ("c", "s3 lifecycle", "tier to glacier after 90 days"),
    ] {
        std::fs::write(
            notes_dir.join(format!("{slug}.md")),
            format!(
                "---\ntype: fact\nid: 01TESTRERANK{:0>14}\ntitle: {title}\nstatus: active\ntags: [t]\nlinks: []\n---\n{body}\n",
                slug.to_uppercase()
            ),
        )
        .unwrap();
    }
    let notes = scan_vault(dir).unwrap();
    let mut brain = Brain::open(&dir.join("brain.sqlite")).unwrap();
    let embedder = kgx_graph::embed::MockEmbedder::new();
    build_full(&mut brain, &notes, &embedder).unwrap();
    brain
}

#[test]
fn rerank_signal_present_and_best_overlap_ranks_first() {
    let tmp = tempfile::tempdir().unwrap();
    let brain = fixture_brain(tmp.path());
    let embedder = kgx_graph::embed::MockEmbedder::new();
    let reranker = MockReranker;
    let r = Retrievers::new(&embedder).with_reranker(Some(&reranker));
    let hits = search(
        &brain,
        &r,
        "flink checkpoint interval",
        SearchOpts {
            mode: Mode::Keyword,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!hits.is_empty());
    assert!(
        hits[0].signals.contains(&"rerank".to_string()),
        "top hit should carry the rerank signal: {:?}",
        hits[0].signals
    );
    assert!(hits[0].id.ends_with('A'), "hits: {hits:?}");
}

#[test]
fn no_reranker_means_no_rerank_signal() {
    let tmp = tempfile::tempdir().unwrap();
    let brain = fixture_brain(tmp.path());
    let embedder = kgx_graph::embed::MockEmbedder::new();
    let r = Retrievers::new(&embedder);
    let hits = search(
        &brain,
        &r,
        "flink checkpoint interval",
        SearchOpts {
            mode: Mode::Keyword,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(hits.iter().all(|h| !h.signals.contains(&"rerank".to_string())));
}
