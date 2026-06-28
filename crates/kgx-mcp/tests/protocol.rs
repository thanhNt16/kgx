use kgx_mcp::protocol::handle_message;
mod common;

#[tokio::test]
async fn initialize_and_tools_list() {
    let d = common::copy_fixture();
    let init = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}});
    let resp = handle_message(d.path(), init).await.unwrap();
    assert_eq!(resp["result"]["serverInfo"]["name"], "kgx");
    let list = serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}});
    let resp = handle_message(d.path(), list).await.unwrap();
    let tools = resp["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    for expected in [
        "search_notes",
        "get_note",
        "upsert_note",
        "ask_question",
        "capture_raw",
        "dream_step",
    ] {
        assert!(names.contains(&expected), "missing tool {expected}");
    }
}
