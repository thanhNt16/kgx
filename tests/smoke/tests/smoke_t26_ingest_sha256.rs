/// T26: ingest_file's hash is real SHA-256 (64 hex chars), not SipHash (16 hex chars).
/// Calls the kgx-mcp library function directly (the kg binary doesn't expose
/// ingest_file as a CLI subcommand; it's only reachable via the MCP server).
use serde_json::json;

#[test]
fn t26_ingest_file_hash_is_sha256() {
    let tmp = tempfile::tempdir().unwrap();
    let content = "hello world\nthis is a test source";
    let args = json!({ "content": content });
    let result = kgx_mcp::tools::ingest_file::run(tmp.path(), &args).unwrap();

    let hash = result["hash"]
        .as_str()
        .expect("hash field must be present on first call");
    assert_eq!(
        hash.len(),
        64,
        "SHA-256 hex digest is 64 chars; got {} ({}). SipHash (the old impl) would be 16.",
        hash,
        hash.len()
    );

    // Verify the digest is lowercase hex (SHA-256's canonical encoding).
    assert!(
        hash.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "SHA-256 hex must be lowercase; got {hash}"
    );

    // Determinism: hashing the same content twice yields the same digest.
    // (We call the helper indirectly by re-running on a fresh tempdir so
    // the path-exists skip branch doesn't interfere.)
    let tmp2 = tempfile::tempdir().unwrap();
    let result2 = kgx_mcp::tools::ingest_file::run(tmp2.path(), &args).unwrap();
    let hash2 = result2["hash"]
        .as_str()
        .expect("second call must also return hash");
    assert_eq!(hash, hash2, "same content must hash identically");

    // Different content must produce a different digest.
    let tmp3 = tempfile::tempdir().unwrap();
    let args3 = json!({ "content": "completely different content here" });
    let result3 = kgx_mcp::tools::ingest_file::run(tmp3.path(), &args3).unwrap();
    let hash3 = result3["hash"].as_str().unwrap();
    assert_ne!(hash, hash3, "different content must hash differently");
    assert_eq!(hash3.len(), 64, "all SHA-256 digests are 64 chars");
}
