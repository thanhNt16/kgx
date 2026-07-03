# 3D Graph Viewer for `kg graph --format html`

## Summary

Replace the current 2D SVG graph visualization in the `kg graph --format html` output with a 3D Three.js-based viewer. The Rust backend stays unchanged — only the HTML template and one test assertion are modified.

## Architecture

The data pipeline is unchanged:

```
Brain SQLite → GraphModel (Rust) → serde_json → embedded in HTML → Three.js renders it
```

Only two files change:

| File | Change |
|------|--------|
| `crates/kgx-viz/templates/graph.html.tera` | Replace entire template with Three.js version |
| `crates/kgx-viz/tests/html.rs` | Relax "no https://" assertion to allow CDN import |

## Template Design

### Module Loading

Use an importmap to load Three.js from an ESM CDN (esm.sh):

```html
<script type="importmap">
{
  "imports": {
    "three": "https://esm.sh/three@0.170",
    "three/addons/": "https://esm.sh/three@0.170/examples/jsm/"
  }
}
</script>
```

### Dual Renderers

- **WebGLRenderer** — 3D scene (spheres for nodes, lines for edges, lighting)
- **CSS2DRenderer** — overlay for labels (always face camera, crisp text)

### Scene Elements

**Nodes:** `SphereGeometry` + `MeshStandardMaterial`. Color by node type: blue (`#2563eb`) for `entity`, amber (`#b45309`) for `decision`, green (`#059669`) for everything else. Radius scales with PageRank. Emissive color on hover.

**Edges:** `BufferGeometry` lines with `LineBasicMaterial`, gray (`#8b8f97`), slight opacity. Arrow-like direction indicators optional.

**Labels:** CSS2D text above each node, shows node title.

**Lighting:** One ambient light + one directional light for depth.

### Layout — 3D Force-Directed

On page load, run a simple spring-electric force simulation:

- Repulsion between all nodes (Coulomb's law)
- Attraction along edges (Hooke's law)
- Centering force to keep graph near origin
- Velocity damping to settle

Runs for ~100-200 iterations, then stops. User orbit controls take over.

### Interaction

| Input | Action |
|-------|--------|
| Left-click drag | Orbit / rotate |
| Right-click drag | Pan |
| Scroll | Zoom in/out |
| Left-click node | Select → detail panel |
| Left-click empty | Deselect |
| Left-click + drag node | Pull / reposition node |

**OrbitControls** from Three.js addons provides rotate/pan/zoom.

**Raycaster** on mousedown detects node hits. On a hit node, start drag mode: project mouse onto a plane through the node perpendicular to the camera. Update node position + connected edges on each mousemove. Release on mouseup.

**Detail panel** (right sidebar) shows selected node's ID, type, status.

**Controls overlay** in corner: "Rotate: click+drag | Zoom: scroll | Pan: right-click+drag".

### Scene Background

Light warm-gray (`#f7f7f4`) matching the current design.

### Header

Shows "KGX Graph — N nodes, M edges" like the current version.

## Test Changes

The existing test `html_is_self_contained_and_counts_match` asserts `!h.contains("https://")`. This needs to be relaxed to allow the known CDN pattern, while still verifying the HTML structure is valid and node/edge counts match.

The new assertion: verify nodes and edges count match, verify the HTML contains the importmap and `<script type="module">`, and still check that no other unexpected URLs are present (allow only the known esm.sh three.js import).

## Non-Goals

- No new Rust code, no new crate dependencies, no Cargo.toml changes
- No new `kg graph` flags — the `--format html` output is replaced entirely
- No animation on load beyond the force simulation settling
- No node/edge creation or deletion — read-only viewer
- No persistence of dragged node positions (lost on refresh)
