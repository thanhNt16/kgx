# Graph Viewer Improvements: Hover Highlight, Community Colors, Edge Details

## Summary

Enhance the existing Three.js 3D graph viewer produced by `kgx:graph --format html` so that:

1. Hovering a node or edge highlights the hovered element and its immediate graph neighborhood.
2. Clicking a node or edge opens a right-side detail panel showing relevant text content.
3. Nodes are colored by community instead of by type.

The work requires a small Rust data-model change to expose the existing `communities` table to the template, plus focused updates to the HTML/JS template.

## Command Surface

The feature is accessed through the existing composite verb:

```
kgx:graph --format html
```

No new CLI flags or Cargo dependencies are introduced. Documentation references use the `kgx:<verb>` style consistently.

## Data Model Change

### `crates/kgx-viz/src/model.rs`

Add a `community` field to `VizNode`:

```rust
pub struct VizNode {
    pub id: String,
    pub title: String,
    pub r#type: String,
    pub status: String,
    pub pagerank: f64,
    pub entity_type: Option<String>,
    pub community: i64,
}
```

Update the SQL in `from_brain` to join the `communities` table:

```sql
SELECT n.id, n.path, n.type, n.status, COALESCE(p.score,0.0), n.entity_type, COALESCE(c.community_id, -1)
FROM notes n
LEFT JOIN pagerank p ON p.id=n.id
LEFT JOIN (SELECT id, MIN(community_id) AS community_id FROM communities GROUP BY id) c ON c.id=n.id
ORDER BY n.id
```

The grouped `LEFT JOIN` guarantees one row per note, so adding the join never duplicates nodes. Notes with no community row (or with an empty `communities` table because community detection has not been run) fall back to `-1`.

## Template Changes

### `crates/kgx-viz/templates/graph.html.tera`

#### Community Colors

- Build a deterministic palette of ~12 distinct hues.
- Map each `community` value to a palette color: `color = palette[community % palette.length]`.
- Nodes with `community === -1` render in a neutral gray (`#6b7280`).
- Node type is no longer used for the body color; it remains visible in the hover tooltip and the detail panel.

#### Hover Highlight

Track `hoveredNodeId` and `hoveredEdgeId`.

When a node is hovered:
- Brighten the hovered node and scale it up 1.25x.
- Find all edges where the node is `src` or `dst`.
- Brighten those edges.
- Brighten the neighbor nodes connected by those edges.
- Dim all other nodes and edges to 25% opacity / 40% brightness.

When an edge is hovered:
- Brighten the edge.
- Brighten its source and target nodes.
- Dim all other nodes and edges.

Because `THREE.LineSegments` raycasting is imprecise for thin lines, add a small, invisible per-edge hit cylinder (low-poly, e.g., 6 radial segments) that is used only for raycasting. The cylinder is not rendered.

#### Click-to-Detail

**Node click** keeps the existing behavior:
- Title, ID, type, status.
- Outbound and inbound edge counts.
- Clickable lists of references (`References` and `Referenced by`).

**Edge click** opens the right panel with:
- Source node title (clickable to select the source node).
- Target node title (clickable to select the target node).
- Relationship text (`rel`).

Clicking empty canvas space clears the selection and closes the detail panel.

## Tests

Update `crates/kgx-viz/tests/html.rs` to verify that the rendered HTML embeds the `community` field for every node. The existing CDN and node-count assertions remain in place.

## Files Changed

| File | Change |
|------|--------|
| `crates/kgx-viz/src/model.rs` | Add `community` field; update SQL join. |
| `crates/kgx-viz/templates/graph.html.tera` | Community colors, hover highlight, edge hover/click, edge detail panel. |
| `crates/kgx-viz/tests/html.rs` | Assert `community` is present in embedded JSON. |

## Non-Goals

- No new Cargo dependencies.
- No changes to the force-directed layout algorithm.
- No persistence of selection, hover, or camera state.
- No editing of nodes or edges in the viewer.
- No new CLI flags; the entry point remains `kgx:graph --format html`.
