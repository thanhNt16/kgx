# Bench Corpus

This directory holds the KGX benchmark corpus for evaluating retrieval and question-answering performance.

## Structure

```
bench-corpus/
  README.md          # this file
  gold.json          # gold standard questions with expected answer patterns and relevant note IDs
  notes/             # vault notes used as the corpus
  config.toml        # bench runner configuration
```

## Gold Set Format (`gold.json`)

```json
[
  {
    "question": "How do I configure the Postgres connection?",
    "expected_patterns": ["connection string", "postgres://"],
    "relevant_note_ids": ["01J9X2ABC", "01J9X2ABD"]
  }
]
```

## Usage

```bash
kg bench --corpus tests/fixtures/bench-corpus --gold tests/fixtures/bench-corpus/gold.json
```

## Notes

- Add questions that exercise specific retrieval capabilities
- Keep `expected_patterns` broad enough to allow multiple correct phrasings
- `relevant_note_ids` must correspond to actual note IDs in the corpus
