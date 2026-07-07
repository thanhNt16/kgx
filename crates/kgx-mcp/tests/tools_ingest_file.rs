use kgx_mcp::tools::dispatch;
use serde_json::json;

#[tokio::test]
async fn ingest_file_content_writes_raw() {
    let d = tempfile::tempdir().unwrap();
    let brain = d.path().join(".brain");
    std::fs::create_dir_all(brain.join("raw")).unwrap();

    let res = dispatch(
        &brain,
        "ingest_file",
        &json!({ "content": "# Title\nbody text" }),
    )
    .await
    .unwrap();
    assert_eq!(res["status"], "ok");
    let raw = res["raw"].as_str().unwrap();
    assert!(raw.starts_with("raw/"));
    assert!(brain.join(raw).exists());
}

#[tokio::test]
async fn ingest_file_directory_walks_text_files() {
    let d = tempfile::tempdir().unwrap();
    let brain = d.path().join(".brain");
    std::fs::create_dir_all(brain.join("raw")).unwrap();

    // Stage a source folder outside the vault.
    let src = d.path().join("src");
    std::fs::create_dir_all(src.join("sub")).unwrap();
    std::fs::write(src.join("a.md"), "# A\nalpha").unwrap();
    std::fs::write(src.join("b.txt"), "beta").unwrap();
    std::fs::write(src.join("sub/c.md"), "# C\ngamma").unwrap();
    std::fs::write(src.join("skip.json"), "{}").unwrap(); // filtered out by ext

    let res = dispatch(&brain, "ingest_file", &json!({ "path": src }))
        .await
        .unwrap();
    assert_eq!(res["status"], "ok");
    assert_eq!(res["count"].as_u64().unwrap(), 3, "3 text files ingested");
    let ingested = res["ingested"].as_array().unwrap();
    assert_eq!(ingested.len(), 3);

    // Each reported raw note exists on disk.
    for entry in ingested {
        let raw = entry["raw"].as_str().unwrap();
        assert!(brain.join(raw).exists(), "missing {raw}");
    }
}

#[tokio::test]
async fn ingest_file_idempotent_on_same_day_same_title() {
    let d = tempfile::tempdir().unwrap();
    let brain = d.path().join(".brain");
    std::fs::create_dir_all(brain.join("raw")).unwrap();

    let args = &json!({ "content": "# Same Title\nsame body" });
    let first = dispatch(&brain, "ingest_file", args).await.unwrap();
    assert_eq!(first["status"], "ok");
    let second = dispatch(&brain, "ingest_file", args).await.unwrap();
    assert_eq!(second["status"], "skipped");
}
