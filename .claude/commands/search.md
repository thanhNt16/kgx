---
name: kgx:search
description: Hybrid keyword + semantic search over the knowledge graph
disable-model-invocation: true
---

Search the knowledge graph.

1. Ask the user for their search query
2. Use the `nl_query_memory` or `deep_search_memory` MCP tool with the query
3. Show results with their note IDs and relevance
