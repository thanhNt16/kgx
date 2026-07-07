use kgx_core::{KgError, Result};
use kgx_graph::Brain;
use std::collections::BTreeSet;

#[derive(Debug, Clone, serde::Serialize)]
pub struct VizNode {
    pub id: String,
    pub title: String,
    pub r#type: String,
    pub status: String,
    pub pagerank: f64,
    pub entity_type: Option<String>,
    pub community: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VizEdge {
    pub src: String,
    pub dst: String,
    pub rel: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GraphModel {
    pub nodes: Vec<VizNode>,
    pub edges: Vec<VizEdge>,
}

pub fn from_brain(brain: &Brain, filter: Option<&str>) -> Result<GraphModel> {
    let (sql, param) = match filter {
        Some(_) => (
            "SELECT n.id, n.path, n.type, n.status, COALESCE(p.score,0.0), n.entity_type, COALESCE(c.community_id, -1) \
             FROM notes n \
             LEFT JOIN pagerank p ON p.id=n.id \
             LEFT JOIN (SELECT id, MIN(community_id) AS community_id FROM communities GROUP BY id) c ON c.id=n.id \
             WHERE n.type=?1 ORDER BY n.id",
            true,
        ),
        None => (
            "SELECT n.id, n.path, n.type, n.status, COALESCE(p.score,0.0), n.entity_type, COALESCE(c.community_id, -1) \
             FROM notes n \
             LEFT JOIN pagerank p ON p.id=n.id \
             LEFT JOIN (SELECT id, MIN(community_id) AS community_id FROM communities GROUP BY id) c ON c.id=n.id \
             ORDER BY n.id",
            false,
        ),
    };
    let mut stmt = brain
        .conn()
        .prepare(sql)
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let rows = if param {
        stmt.query_map([filter.unwrap_or_default()], node_from_row)
            .map_err(|e| KgError::Brain(e.to_string()))?
            .collect::<std::result::Result<Vec<_>, _>>()
    } else {
        stmt.query_map([], node_from_row)
            .map_err(|e| KgError::Brain(e.to_string()))?
            .collect::<std::result::Result<Vec<_>, _>>()
    }
    .map_err(|e| KgError::Brain(e.to_string()))?;

    let ids: BTreeSet<&str> = rows.iter().map(|n| n.id.as_str()).collect();
    let mut edge_stmt = brain
        .conn()
        .prepare("SELECT src_id, dst_id, rel_type FROM edges ORDER BY src_id, dst_id, rel_type")
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let edges = edge_stmt
        .query_map([], |r| {
            Ok(VizEdge {
                src: r.get(0)?,
                dst: r.get(1)?,
                rel: r.get(2)?,
            })
        })
        .map_err(|e| KgError::Brain(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| KgError::Brain(e.to_string()))?
        .into_iter()
        .filter(|e| ids.contains(e.src.as_str()) && ids.contains(e.dst.as_str()))
        .collect();

    Ok(GraphModel { nodes: rows, edges })
}

fn node_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<VizNode> {
    Ok(VizNode {
        id: r.get(0)?,
        title: r.get(1)?,
        r#type: r.get(2)?,
        status: r.get(3)?,
        pagerank: r.get(4)?,
        entity_type: r.get(5)?,
        community: r.get(6)?,
    })
}
