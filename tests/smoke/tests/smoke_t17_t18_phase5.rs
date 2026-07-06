/// T17/T18: RTK fallback/integration and Ponytail audit smoke coverage.
use assert_cmd::Command;
use kgx_rtk::run_with_rtk;

mod common;

#[test]
fn t17_rtk_wrapper_uses_rtk_or_raw_fallback() {
    let mut cmd = std::process::Command::new("sh");
    cmd.arg("-c")
        .arg("printf 'alpha beta gamma\\n%.0s' 1 2 3 4 5");
    let out = run_with_rtk(&mut cmd).unwrap();

    assert!(out.raw_bytes > 0);
    assert!(out.compressed_bytes > 0);
    let has_rtk = std::process::Command::new("sh")
        .arg("-c")
        .arg("command -v rtk >/dev/null 2>&1")
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if std::env::var("KGX_RTK").as_deref() == Ok("off") || !has_rtk {
        assert_eq!(out.raw_bytes, out.compressed_bytes);
        assert!(out.stdout.contains("alpha beta gamma"));
    } else {
        assert!(out.compressed_bytes <= out.raw_bytes);
    }
}

#[test]
fn t18_ponytail_audit_reports_over_broad_review_flags() {
    let d = common::copy_fixture();
    std::fs::create_dir_all(d.path().join(".brain/.kg")).unwrap();
    let files: Vec<serde_json::Value> = (0..4)
        .map(|i| {
            serde_json::json!({
                "rel_path": format!("notes/facts/audit-{i}.md"),
                "before": null,
                "after": "---\ntype: fact\nid: X\ntitle: Audit\n---\n\nAudit.\n"
            })
        })
        .collect();
    std::fs::write(
        d.path().join(".brain/.kg/staged_diffs.json"),
        serde_json::to_string_pretty(&serde_json::json!([{
            "id": "audit-diff",
            "pass": "dedup",
            "kind": "add_link",
            "rationale": "over broad test rationale",
            "severity": "soft",
            "files": files
        }]))
        .unwrap(),
    )
    .unwrap();

    let out = Command::cargo_bin("kg")
        .unwrap()
        .args(["review", "--approve", "all", "--ponytail-audit", "--json"])
        .current_dir(d.path())
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    let flags = v["data"]["audit_flags"].as_array().unwrap();
    assert!(flags
        .iter()
        .any(|f| f.as_str().unwrap().contains("over_broad")));
}
