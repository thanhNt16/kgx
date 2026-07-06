---
name: kgx-codebase
description: Codebase graph querying via codebase-memory-mcp — search functions, trace call paths, read source code, and inspect architecture.
---

# kgx-codebase

Use codebase-memory-mcp tools for structural code queries.

## When to use
- Explore the codebase, understand the architecture
- Find functions, classes, routes, variables
- Trace call chains, find callers of a function
- Impact analysis, dead code detection, refactor candidates

## Graph commands
- `search_graph` — BM25 full-text search for functions/classes/routes
- `trace_path` — follow CALLS/DATA_FLOWS edges inbound/outbound
- `get_code_snippet` — read source for a qualified name
- `get_architecture` — high-level project overview with Leiden clusters
- `query_graph` — Cypher queries for complex multi-hop patterns
- `search_code` — graph-augmented grep with structural ranking

## Workflow
1. Run `kgx:codebase-index` first if the graph is stale
2. Use `search_graph` to find what you need
3. Use `trace_path` to understand callers/callees
4. Use `get_code_snippet` to read the source
