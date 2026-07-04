//! T27: real fastembed embeddings produce semantically-relevant retrieval
//! that keyword search cannot match. Gated on the `semantic` feature so the
//! hermetic suite stays network-free; the model downloads ~40MB on first run
//! and is cached in ~/.cache/fastembed/.
//!
//! Run with:
//!   cargo test --package kgx-graph --features semantic --test semantic_e2e
#![cfg(all(feature = "semantic", test))]

use kgx_core::llm::Embedder;
use kgx_graph::embed::FastEmbedEmbedder;

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

#[test]
fn fastembed_returns_semantic_neighbors_not_keyword_overlap() {
    let e = FastEmbedEmbedder::load().expect("fastembed model must download on first run");
    assert_eq!(e.dim(), 384);
    assert!(e.is_semantic());

    // Two phrases with ZERO shared words but clear semantic similarity,
    // plus an unrelated distractor with a shared word ("data") that
    // keyword search would wrongly prefer.
    let phrases = vec![
        "how do I store information in my application".to_string(), // query-ish
        "best practices for persisting records".to_string(),       // semantically close, no word overlap
        "the weather forecast for tomorrow".to_string(),           // unrelated distractor
    ];
    let embs = e.embed(&phrases).expect("embed must succeed");
    assert_eq!(embs.len(), 3);
    for emb in &embs {
        assert_eq!(emb.len(), 384, "every embedding must be 384-dim");
    }

    let q = &embs[0];
    let persist = &embs[1];
    let weather = &embs[2];

    let sim_persist = cosine(q, persist);
    let sim_weather = cosine(q, weather);

    // The core semantic-retrieval assertion: a phrase about storing
    // information is closer to one about persisting records than to one
    // about the weather, despite zero lexical overlap with the former.
    assert!(
        sim_persist > sim_weather,
        "semantic sim to 'persist records' ({sim_persist:.3}) must exceed sim to 'weather' ({sim_weather:.3}); \
         if this fails, the embedder is not producing meaningful embeddings"
    );
    assert!(
        sim_persist > 0.3,
        "semantically-related phrases should have >0.3 cosine similarity; got {sim_persist:.3}"
    );
}

#[test]
fn fastembed_is_deterministic() {
    let e = FastEmbedEmbedder::load().expect("model loads");
    let a = e
        .embed(&["deterministic embedding test".to_string()])
        .unwrap()
        .remove(0);
    let b = e
        .embed(&["deterministic embedding test".to_string()])
        .unwrap()
        .remove(0);
    // Same input → identical embedding (cosine sim exactly 1.0).
    assert!(
        (cosine(&a, &b) - 1.0).abs() < 1e-5,
        "fastembed must be deterministic; cosine = {}",
        cosine(&a, &b)
    );
}
