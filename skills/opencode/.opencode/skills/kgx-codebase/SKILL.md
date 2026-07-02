---
name: kgx-codebase
description: Codebase graph querying via codebase-memory-mcp — search functions, trace call paths, read source code, and inspect architecture.
---

# kgx:codebase

Codebase-memory-mcp tools for understanding project structure.

## Priority Order
1. `search_graph` — find functions, classes, routes, variables by pattern
2. `trace_path` — trace who calls a function or what it calls
3. `get_code_snippet` — read specific function/class source code
4. `query_graph` — run Cypher queries for complex patterns
5. `get_architecture` — high-level project summary

## CLI Commands
```
kg codebase install     # Install codebase-memory-mcp binary
kg codebase update      # Update existing binary
kg codebase index       # Index the current repo
kg codebase search <q>  # Search codebase graph
kg codebase trace <fn>  # Trace function call paths
kg codebase status      # Show indexed projects
```
