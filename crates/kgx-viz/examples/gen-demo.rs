use kgx_graph::{build::build_full, embed::MockEmbedder, Brain};
use kgx_vault::scan::scan_vault;
use kgx_viz::{html, model::from_brain};

fn main() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/vault-min");
    let notes = scan_vault(&fixture).unwrap();
    let mut brain = Brain::open_in_memory().unwrap();
    build_full(&mut brain, &notes, &MockEmbedder::new()).unwrap();
    let m = from_brain(&brain, None).unwrap();
    let h = html::render(&m);

    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/kgx-graph-demo.html");
    std::fs::write(&out, h).unwrap();
    println!("wrote {} bytes to {}", out.display(), std::fs::metadata(&out).unwrap().len());
    println!("nodes: {}, edges: {}", m.nodes.len(), m.edges.len());
}
