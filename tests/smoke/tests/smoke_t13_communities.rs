/// T13: fixture yields at least 3 communities; graph unit tests cover connectedness,
/// and this smoke also verifies each detected community is internally connected.
use assert_cmd::Command;
use std::collections::{BTreeMap, BTreeSet};

mod common;

#[test]
fn t13_every_community_has_summary_and_moc() {
    let d = common::copy_fixture();
    Command::cargo_bin("kg")
        .unwrap()
        .env("KGX_LLM", "mock")
        .args(["index", "--full", "--communities"])
        .current_dir(d.path())
        .assert()
        .success();

    let conn = rusqlite::Connection::open(d.path().join(".brain/.kg/brain.sqlite")).unwrap();
    let mut stmt = conn
        .prepare("SELECT DISTINCT community_id FROM communities ORDER BY community_id")
        .unwrap();
    let communities: Vec<i64> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let summaries: i64 = conn
        .query_row("SELECT count(*) FROM community_summaries", [], |r| r.get(0))
        .unwrap();

    assert!(
        communities.len() >= 3,
        "expected at least 3 communities, got {}",
        communities.len()
    );
    assert_eq!(summaries as usize, communities.len());
    for cid in &communities {
        assert!(
            d.path()
                .join(format!(".brain/notes/moc/community-{cid}.md"))
                .exists(),
            "missing MOC for community {cid}"
        );
        assert_community_connected(&conn, *cid);
    }
}

fn assert_community_connected(conn: &rusqlite::Connection, community_id: i64) {
    let mut stmt = conn
        .prepare("SELECT id FROM communities WHERE community_id = ?1 ORDER BY id")
        .unwrap();
    let members: Vec<String> = stmt
        .query_map([community_id], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    if members.len() <= 1 {
        return;
    }

    let member_set: BTreeSet<String> = members.iter().cloned().collect();
    let mut adj: BTreeMap<String, Vec<String>> =
        members.iter().map(|id| (id.clone(), Vec::new())).collect();
    let mut edge_stmt = conn.prepare("SELECT src_id, dst_id FROM edges").unwrap();
    let edges: Vec<(String, String)> = edge_stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    for (src, dst) in edges {
        if member_set.contains(&src) && member_set.contains(&dst) {
            adj.get_mut(&src).unwrap().push(dst.clone());
            adj.get_mut(&dst).unwrap().push(src);
        }
    }

    let mut seen = BTreeSet::new();
    let mut stack = vec![members[0].clone()];
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        if let Some(neighbors) = adj.get(&id) {
            stack.extend(neighbors.iter().cloned());
        }
    }
    assert_eq!(
        seen.len(),
        members.len(),
        "community {community_id} is not connected"
    );
}
