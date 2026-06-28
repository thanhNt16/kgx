# Retrieve → Graph Rerank (Phase 1)

## Problem

Current `kg search --mode keyword` fuses BM25, LIKE, and tag expansion via parallel RRF. All signals contribute simultaneously at equal weight tiers. This may bury graph-adjacent relevant items that score poorly on text overlap but are close to BM25 seeds in the knowledge graph.

## Design

New two-stage pipeline behind `SearchOpts.rerank_graph: bool` flag.

**Stage 1 — Retrieve**: BM25 + LIKE each at limit=100 (up from 50). Deduplicated union into candidate pool.

**Stage 2 — Rerank**: Pool-scoped Personalized PageRank from BM25 top-5 seeds (harmonic weights: 1, 1/2, 1/3, 1/4, 1/5). Builds subgraph adjacency from only pool nodes' edges. Runs PPR with 0.85 damping, 20 iterations, hop-distance damping. Sort pool by PPR score, take `opts.limit`.

### Why pool-scoped instead of full-graph PPR

Full-graph PPR (`personalized()`) iterates over all 32 nodes. Pool-scoped iterates over only the candidate pool (~50-150 nodes). With more vaults (1000s of nodes), the savings grow. Also isolates the signal to the retrieved set — distant graph nodes can't inject noise.

### Integration

```rust
// SearchOpts addition
pub rerank_graph: bool,  // default false
```

When `mode == Keyword && rerank_graph`, the existing fused RRF path is bypassed. `Mode::Hybrid` and `Mode::Semantic` unchanged.

### Files

| File | Change |
|------|--------|
| `ppr.rs` | Add `personalized_scoped(brain, scope, seeds, damping, iters)` |
| `hybrid.rs` | Add `rerank_graph` field to `SearchOpts`; new keyword flow |
| `search.rs` | Pass `--rerank-graph` CLI flag |
| `ask.rs` | Pass rerank_graph to SearchOpts |

### CLI

```
kg search --mode keyword --rerank-graph "query"
```

### Success criteria

- Benchmark P@5, R@5, F1, MRR, NDCG vs old fused RRF keyword mode
- Ideally improves long-tail recall for notes with weak text overlap but strong graph connectivity
