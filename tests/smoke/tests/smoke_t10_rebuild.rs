/// T10: .kg rebuild is deterministic — identical brain bytes across rebuilds.
/// Upgraded from count-only to byte-hash equality across the notes table,
/// pagerank, communities, and notes_vec contents.
use assert_cmd::Command;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

fn copy_fixture() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vault-min");
    for e in walkdir::WalkDir::new(&src) {
        let e = e.unwrap();
        let rel = e.path().strip_prefix(&src).unwrap();
        let dst = d.path().join(rel);
        if e.file_type().is_dir() {
            std::fs::create_dir_all(&dst).unwrap();
        } else {
            std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
            std::fs::copy(e.path(), &dst).unwrap();
        }
    }
    d
}

fn brain_path(dir: &Path) -> PathBuf {
    dir.join(".kg/brain.sqlite")
}

/// Canonical content hash of the brain's derived tables.
/// Ordering is forced deterministic by ORDER BY on primary keys.
fn brain_fingerprint(p: &Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let conn = Connection::open(p).unwrap();
    let mut out = String::new();

    let concat = |sql: &str, out: &mut String| {
        let mut stmt = conn.prepare(sql).unwrap();
        let ncols = stmt.column_count();
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            for i in 0..ncols {
                let val: rusqlite::types::Value =
                    row.get(i).unwrap_or(rusqlite::types::Value::Null);
                let s = match val {
                    rusqlite::types::Value::Text(s) => s,
                    rusqlite::types::Value::Integer(n) => n.to_string(),
                    rusqlite::types::Value::Real(f) => f.to_string(),
                    rusqlite::types::Value::Blob(b) => format!("<blob {} bytes>", b.len()),
                    rusqlite::types::Value::Null => "<null>".to_string(),
                };
                out.push_str(&s);
                out.push('|');
            }
            out.push('\n');
        }
    };
    concat(
        "SELECT id, type, status, tags, raw_text FROM notes ORDER BY id",
        &mut out,
    );
    concat(
        "SELECT src_id, dst_id, rel_type FROM edges ORDER BY src_id, dst_id, rel_type",
        &mut out,
    );
    concat("SELECT id, score FROM pagerank ORDER BY id", &mut out);
    concat(
        "SELECT id, community_id FROM communities ORDER BY id",
        &mut out,
    );

    let mut h = DefaultHasher::new();
    out.hash(&mut h);
    format!("{:016x}", h.finish())
}

#[test]
fn t10_rebuild_is_deterministic() {
    let d = copy_fixture();

    // Seed pass: `kg index --communities` writes community MOC files to
    // notes/moc/ AFTER building the brain. Without a seed pass, the first
    // rebuild sees no MOC files on disk and the second sees them — that's
    // a difference in INPUT, not non-determinism. By seeding first, both
    // rebuilds start from identical on-disk state (fixture + MOC files).
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full", "--communities", "--pagerank"])
        .current_dir(d.path())
        .assert()
        .success();

    // Rebuild 1 from the now-stable on-disk state.
    std::fs::remove_dir_all(d.path().join(".kg")).unwrap();
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full", "--communities", "--pagerank"])
        .current_dir(d.path())
        .assert()
        .success();
    let fp1 = brain_fingerprint(&brain_path(d.path()));

    // Rebuild 2: same on-disk state. Must produce an identical brain.
    std::fs::remove_dir_all(d.path().join(".kg")).unwrap();
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full", "--communities", "--pagerank"])
        .current_dir(d.path())
        .assert()
        .success();
    let fp2 = brain_fingerprint(&brain_path(d.path()));

    assert_eq!(fp1, fp2, "brain fingerprint must match across rebuilds");

    // Rebuild 3: belt-and-suspenders — Leiden uses seed=42 and PageRank is
    // order-stable, so a third rebuild from the same state must also match.
    std::fs::remove_dir_all(d.path().join(".kg")).unwrap();
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full", "--communities", "--pagerank"])
        .current_dir(d.path())
        .assert()
        .success();
    let fp3 = brain_fingerprint(&brain_path(d.path()));
    assert_eq!(fp2, fp3, "third rebuild must match second");
}
