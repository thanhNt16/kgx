use std::process::Command;

fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn bench_metrics_accept_answer_pattern_hits() {
    let root = repo_root();
    let script = r#"
import importlib.util
import os
from pathlib import Path

root = Path(os.environ["KGX_REPO_ROOT"])
spec = importlib.util.spec_from_file_location("kgx_bench", root / "bench" / "bench.py")
bench = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench)

metrics = bench.metrics_for_query(
    [("note-a", 1.0, ""), ("note-b", 0.5, "")],
    {"missing-id"},
    expected_patterns=["pagerduty", "billing pipeline"],
    note_texts={"note-a": "Wire PagerDuty for billing pipeline failures."},
    k=5,
)
assert metrics["recall"] == 1.0, metrics
assert metrics["mrr"] == 1.0, metrics
assert str(bench.OUT_JSON).endswith("bench/results.json"), bench.OUT_JSON
"#;
    let out = Command::new("python3")
        .arg("-c")
        .arg(script)
        .env("KGX_REPO_ROOT", root)
        .output()
        .expect("python3 should run benchmark harness test");
    assert!(
        out.status.success(),
        "python test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn generated_flink_entity_names_owner() {
    let root = repo_root();
    let corpus = tempfile::tempdir().unwrap();
    let out = Command::new("python3")
        .arg(root.join("bench/gen_corpus.py"))
        .arg(corpus.path())
        .output()
        .expect("python3 should run corpus generator");
    assert!(
        out.status.success(),
        "corpus generation failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let flink = std::fs::read_to_string(corpus.path().join("notes/entities/flink.md")).unwrap();
    assert!(
        flink.to_lowercase().contains("cara"),
        "Flink entity should contain its owner so owner lookups have answer-bearing evidence:\n{flink}"
    );
}
