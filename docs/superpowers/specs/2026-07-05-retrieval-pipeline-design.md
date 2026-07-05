# Retrieval Pipeline Upgrade — Design

**Date:** 2026-07-05
**Status:** Approved (brainstorm validated with user)
**Depends on:** `2026-07-05-battletest-gaps-design.md` — WS1 (default embeddings)
must be merged before any phase; WS2 (POLE entities + typed edges) before Phase 3.

## Goal

Raise retrieval quality beyond what WS1 (embeddings on by default) delivers, by
upgrading the default hybrid search to the production-standard pipeline:
wide candidate generation → RRF fusion → graph expansion → local cross-encoder
rerank. All additions run locally (ONNX via fastembed, already a dependency),
offline after one model download, with no per-query LLM cost. LLM techniques
(HyDE, query rewriting) stay out of scope for the default path.

## Research grounding

- **Production pattern (2026):** BM25 + dense candidates → RRF → cross-encoder
  rerank is the standard three-stage retrieval architecture. RRF for robust
  fusion without score calibration; cross-encoder for query-document
  interaction the fused list lacks.
- **HippoRAG 2:** matching the *query* against entity/triple nodes by embedding
  and seeding Personalized PageRank from those matches gives +12.5% Recall@5
  over naive entity seeding, and 90.4% vs 76.5% Recall@5 on multi-hop QA
  versus a pure-embedding baseline. KGX already has the PPR engine; only the
  seeding is naive (BM25 top-5 docs) and gated on the dense embedder.
- **fastembed-rs** (already KGX's WS1 dependency) ships all needed models
  locally: `SparseTextEmbedding` (SPLADE++ en v1 — learned sparse, fixes
  vocabulary mismatch without an LLM) and `TextRerank` (cross-encoders,
  including jina-reranker-v1-turbo-en built for CPU speed). No new
  infrastructure, only models.

Current bench baseline (keyword-only): Recall@5 = 0.733, MRR = 0.7. The four
misses ("Who owns Flink", "What causes Flink backpressure", "Trino latency
target", "billing pipeline alerting") are all vocabulary-mismatch failures.

## Constraints (user-set)

1. Follow-on to the battletest-gaps plan; assumes WS1 lands. Do not reopen the
   embedder decision (BGE-M3 unified model rejected on those grounds).
2. Default `kg search` / MCP `query_memory` path: local ONNX models allowed,
   must work offline after first download, p95 end-to-end < 200 ms on the
   220-note bench corpus. No LLM tokens per query.
3. Success is measured on an expanded bench (15 → 45 questions) with
   per-category floors; the original 15 questions must not regress.

## Architecture

Default hybrid mode inside `kgx-retrieval::search()` — no new crates, no CLI
or MCP API break:

```
Stage 1 — Candidate generation (each → top 50)
  ├─ BM25 (FTS5)              existing
  ├─ LIKE substring           existing
  ├─ Tag expansion            existing
  ├─ Dense vector KNN         existing (sqlite-vec)
  └─ SPLADE sparse            NEW

Stage 2 — RRF fusion (fuse_multi_k)      existing; k values retuned on bench

Stage 3 — Graph expansion (PPR)          existing engine, two changes:
  • seeds = BM25 top-5 harmonic weights  (existing)
          + query-matched entity nodes   NEW (HippoRAG-style)
  • un-gated: runs whenever the brain has ≥1 edge
    (today it is gated on has_vector and never fires in keyword mode)

Stage 4 — Cross-encoder rerank           NEW — local ONNX reranker scores
  (query, note) pairs for the top 30 fused candidates → final top-k
```

Key properties:

- **Brain stays disposable.** SPLADE postings are a new SQLite table rebuilt
  by `kg index`; `SCHEMA_VERSION` bumps to 3 (after WS2's 2). Re-index, never
  migrate.
- **Keyword-only mode still benefits.** With `KGX_EMBED=off`, SPLADE + PPR +
  rerank all still run (none needs the dense embedder); only entity seeding
  degrades to BM25-seeds-only.

## Components

### C1 — SPLADE sparse signal

Module: `crates/kgx-graph/src/sparse.rs`.

- **Indexing:** `kg index` runs fastembed `SparseTextEmbedding`
  (`Splade_PP_en_v1`, ~130 MB one-time download, cached in the user cache dir)
  over note bodies alongside dense embeddings. Output per note: 50–300
  `(term_id, weight)` pairs.
- **Storage:** table `sparse_postings(term_id INTEGER, note_id TEXT,
  weight REAL)` with an index on `term_id` — an inverted index in plain
  SQLite, no extension.
- **Query:** sparse-embed the query; one SQL join accumulates
  `SUM(query_weight * doc_weight)` per note; top 50 join the RRF rankings
  with k = 60 (same as BM25/dense).
- **Selection:** on by default when the `semantic` cargo feature is built
  (WS1 makes it default). `KGX_SPARSE=off` opts out. Model-load failure →
  warn once, skip the stage.

### C2 — Cross-encoder reranker

Module: `crates/kgx-retrieval/src/rerank.rs`; trait in `kgx-core::llm`.

- **Trait:** `Reranker { fn rerank(&self, query: &str, docs: &[(String, String)])
  -> Result<Vec<f32>>; fn model_name(&self) -> String; }` where docs are
  `(id, text)` pairs. A deterministic `MockReranker` ships for tests.
- **Implementation:** fastembed `TextRerank`. Default model
  `jina-reranker-v1-turbo-en` (~38 MB quantized, CPU-fast).
  `KGX_RERANK_MODEL` overrides; `KGX_RERANK=off` disables.
- **Placement:** after fusion + PPR, over the top 30 candidates
  (`KGX_RERANK_TOPK`), each truncated to title + first 512 chars of body.
  Final order = reranker scores descending; candidates beyond top-30 keep
  fused order below them.
- **Relationship to `--rerank-llm`:** unchanged; the LLM reranker remains the
  opt-in higher-quality path. The cross-encoder is the *default* third stage.

### C3 — Entity-seeded PPR

Changes in `crates/kgx-retrieval/src/hybrid.rs` and `ppr.rs` (seed
construction only; the PPR engine is untouched).

- Remove the `has_vector` gate; run PPR expansion whenever the brain has ≥1
  edge.
- New seed source: dense-embed the query, cosine against stored embeddings of
  entity notes only (`type = 'entity'`, which after WS2 includes POLE-typed
  entities). Entities with cosine ≥ 0.60 join the seed set, capped at 5,
  each weighted at half the corresponding BM25 harmonic weight scale
  (i.e. entity seed i gets weight `0.5 / (i + 1)`).
- BM25 harmonic top-5 seeds unchanged. If the dense embedder is unavailable,
  entity seeding is skipped and BM25 seeds alone are used (today's behavior).
- Effect: "Who owns Flink?" seeds PPR at the Flink entity node; typed edges
  (`owns`, `participates_in`) carry rank to the answer note even with zero
  lexical overlap.

### C4 — Expanded bench

Files: `bench/gen_corpus.py`, `bench/bench.py`.

- Gold set grows 15 → 45: +10 vocabulary-mismatch (paraphrased, zero lexical
  overlap with the evidence note), +10 multi-hop (evidence reachable only via
  an edge from the lexical match), +5 temporal ("what superseded X"),
  +5 entity-relation ("who owns / decided Y").
- `bench.py` gains `--category` filtering and a per-category results table;
  `results.json` records per-category aggregates and p95 latency.
- **Acceptance floors:** original 15 — Recall@5 ≥ 0.85 (no regression from
  post-WS1 state); vocab-mismatch ≥ 0.70; multi-hop ≥ 0.60; temporal ≥ 0.60;
  entity-relation ≥ 0.70; p95 end-to-end latency < 200 ms.

Component boundaries: C1 is pure `kgx-graph` (storage + scoring), C2 is pure
`kgx-retrieval` (post-fusion stage), C3 touches only seed construction, C4 is
tooling. Each lands and tests independently.

## Error handling

1. **Independent, visible degradation.** Sparse model missing → warn once at
   index/search time, stage skipped. Reranker missing → warn, return fused
   order. No edges → PPR skipped silently (nothing to expand). `kg status`
   grows a `retrieval:` line listing active stages, e.g.
   `retrieval: bm25+like+tags+dense+sparse | ppr(entity-seeded) | rerank(jina-turbo)`.
2. **Shape stability.** A degraded stage changes ranking quality only — never
   result shape. MCP tools and `--json` output are unaffected.
3. **No mid-search downloads.** Models download at `kg index` time with the
   WS1 cache-dir + fail-loud behavior.

## Testing strategy

- **Unit:** sparse posting round-trip and join-scoring math vs hand-computed
  values; pipeline ordering with `MockReranker`; entity-seed selection
  (threshold 0.60, cap 5, half-weight) as a pure function.
- **Integration:** `cli_search.rs` asserts the `signals` field reports
  `sparse` / `rerank` / `ppr` when stages are active.
- **Bench gate:** expanded 45-question set run before/after each phase;
  per-category floors above; latency recorded in `results.json`.

## Rollout

| Phase | Ships | Depends on | Validated by |
|---|---|---|---|
| 1 | C2 reranker + C3's PPR un-gating | WS1 merged | original 15 + latency gate |
| 2 | C1 SPLADE (schema v3) | WS1 merged | vocab-mismatch category |
| 3 | C3 entity seeding + C4 expanded bench | WS2 merged | multi-hop + entity-relation categories |

Each phase is independently shippable and benchmarked.

## Out of scope

- ColBERT / late-interaction retrieval and the BGE-M3 unified model (reopens
  WS1's embedder decision).
- LLM-per-query techniques (HyDE, query rewriting, decomposition) — candidates
  for a later `deep_search_memory` spec.
- ANN index swaps — sqlite-vec brute-force KNN is fine at current corpus
  scale; revisit only if the bench latency gate fails at larger scale.
