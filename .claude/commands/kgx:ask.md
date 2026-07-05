---
name: kgx:ask
description: Ask a question with citation-backed answers from the knowledge graph
disable-model-invocation: true
---

Ask a question using the knowledge graph.

1. Ask the user for their question
2. Use `nl_query_memory` MCP tool with the query and `--cite` style response
3. Present the answer with citations to source note IDs
