use kgx_graph::{build::build_full, embed::MockEmbedder, Brain};
use kgx_mcp::tools::dispatch;
use serde_json::json;

mod common;

#[tokio::test]
async fn upsert_note_writes_valid_mcp_frontmatter() {
    let d = tempfile::tempdir().unwrap();

    let res = dispatch(
        d.path(),
        "upsert_note",
        &json!({
            "type": "fact",
            "title": "MCP created fact",
            "body": "The MCP tool can write notes."
        }),
    )
    .await
    .unwrap();

    let rel = res["path"].as_str().expect("path in response");
    let written = std::fs::read_to_string(d.path().join(rel)).unwrap();
    assert!(written.contains("type: fact"));
    assert!(written.contains("title: MCP created fact"));
    assert!(written.contains("created_by: agent"));
    assert!(written.contains("created_via: mcp"));
    assert!(written.contains("The MCP tool can write notes."));
}

#[tokio::test]
async fn dream_step_returns_diffs_without_staging_or_applying() {
    let d = common::copy_fixture();
    unsafe {
        std::env::set_var("KGX_LLM", "mock");
    }
    let notes = kgx_vault::scan::scan_vault(d.path()).unwrap();
    let mut brain = Brain::open(&d.path().join(".kg/brain.sqlite")).unwrap();
    build_full(&mut brain, &notes, &MockEmbedder::new()).unwrap();

    let note_path = d.path().join("notes/facts/f-orphan.md");
    let before = std::fs::read_to_string(&note_path).unwrap();
    let res = dispatch(
        d.path(),
        "dream_step",
        &json!({"only": "orphan_repair", "max_iterations": 1}),
    )
    .await
    .unwrap();

    assert!(res["iterations"].as_u64().unwrap() <= 1);
    assert!(!res["diffs"].as_array().unwrap().is_empty());
    assert!(!d.path().join(".kg/staged_diffs.json").exists());
    assert_eq!(before, std::fs::read_to_string(note_path).unwrap());
}
