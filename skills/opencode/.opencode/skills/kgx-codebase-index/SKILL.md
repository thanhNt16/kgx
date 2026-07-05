---
name: kgx-codebase-index
description: Index the current repository into the codebase-memory-mcp graph. Run this first before searching or tracing.
---

# kgx-codebase-index

Index the repo into the codebase-memory-mcp knowledge graph.

## When to use
- First time working in this repo
- Codebase-memory-mcp returns no or stale results
- After significant code changes

## Usage
The indexer analyzes all source files, extracts functions/classes/routes,
builds CALLS/DATA_FLOWS/IMPLEMENTS edges, and runs community detection
(Leiden clustering) for architecture overview.

Index modes:
- `full` — all files + similarity/semantic edges
- `moderate` — filtered files + similarity/semantic
- `fast` — filtered files, no similarity/semantic
