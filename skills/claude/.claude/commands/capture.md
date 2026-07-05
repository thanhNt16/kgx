---
name: kgx:capture
description: Capture raw source material verbatim (immutable)
disable-model-invocation: true
---

Capture raw source material.

1. Ask the user what to capture: a file path, URL, or pasted content
2. Use the appropriate MCP tool:
   - `ingest_file` for file content
   - `ingest_url` for a URL
   - `ingest_conversation` for conversation text
3. Show the captured source ID to the user
