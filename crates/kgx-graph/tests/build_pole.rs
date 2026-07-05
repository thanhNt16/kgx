use kgx_core::{Note, RelType};
use kgx_graph::build::derive_edges;

fn note(id: &str, title: &str) -> Note {
    let yaml = format!("type: fact\nid: {id}\ntitle: {title}\n");
    let fm: kgx_core::Frontmatter = serde_yaml::from_str(&yaml).unwrap();
    Note {
        fm,
        body: String::new(),
        rel_path: std::path::PathBuf::from(format!("notes/facts/{title}.md")),
    }
}

#[test]
fn relations_frontmatter_becomes_typed_edges() {
    let mut fact = note("F1", "billing-migrates");
    let alice = note("E1", "Alice");
    let rels: serde_yaml::Value =
        serde_yaml::from_str("- target: Alice\n  rel: decided\n").unwrap();
    fact.fm.extra.insert("relations".into(), rels);

    let edges = derive_edges(&[fact, alice]);
    assert!(
        edges
            .iter()
            .any(|e| e.src_id == "F1" && e.dst_id == "E1" && e.rel_type == RelType::Decided),
        "expected F1 -decided-> E1, got {edges:?}"
    );
}

#[test]
fn unknown_rel_and_unresolvable_target_are_ignored() {
    let mut fact = note("F1", "t");
    let rels: serde_yaml::Value =
        serde_yaml::from_str("- target: Ghost\n  rel: decided\n- target: t\n  rel: not_a_rel\n")
            .unwrap();
    fact.fm.extra.insert("relations".into(), rels);
    let edges = derive_edges(&[fact]);
    assert!(edges
        .iter()
        .all(|e| e.rel_type == RelType::LinksTo || e.rel_type != RelType::Decided));
}
