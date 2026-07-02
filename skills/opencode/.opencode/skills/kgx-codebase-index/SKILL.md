---
name: kgx-codebase-index
description: Index the current repository into the codebase-memory-mcp graph. Run this first before searching or tracing. Use when the user says "index the codebase", "build the graph", or codebase-memory-mcp returns no results.
---

# kgx:codebase-index

Index the current repository into the codebase graph. Required before `search_graph`, `trace_path`, or any other codebase-memory-mcp tool.

```
kg codebase index
```

Returns indexed node and edge counts. Re-index when the codebase changes significantly.
