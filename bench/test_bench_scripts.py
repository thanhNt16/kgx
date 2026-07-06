import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from bench import bench as bench_harness
from bench import gen_large_corpus


class LargeCorpusGeneratorTests(unittest.TestCase):
    def test_base_vault_notes_are_preserved_when_generating_distractors(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            base = root / "base"
            out = root / "large"
            note = base / "notes" / "facts" / "base.md"
            note.parent.mkdir(parents=True)
            note.write_text(
                """---
type: fact
id: BASE123
title: Base Fact
---
# Base Fact

backpressure and exactly-once evidence
"""
            )

            argv = [
                "gen_large_corpus.py",
                "--out",
                str(out),
                "--nodes",
                "80",
                "--facts-per-sprint",
                "2",
                "--base-vault",
                str(base),
            ]
            with mock.patch.object(sys, "argv", argv):
                gen_large_corpus.main()

            copied = out / "notes" / "facts" / "base.md"
            self.assertTrue(copied.exists())
            self.assertIn("backpressure and exactly-once", copied.read_text())


class BenchHarnessTests(unittest.TestCase):
    def test_unreachable_gold_entries_are_reported_before_scoring(self):
        note_texts = {
            "NOTE1": "This note mentions backpressure.",
            "NOTE2": "This note mentions exactly-once delivery.",
        }
        gold = [
            {"question": "reachable by id", "relevant_note_ids": ["NOTE1"]},
            {
                "question": "reachable by pattern",
                "relevant_note_ids": ["MISSING"],
                "expected_patterns": ["exactly-once"],
            },
            {
                "question": "unreachable",
                "relevant_note_ids": ["MISSING"],
                "expected_patterns": ["billing_ledger"],
            },
        ]

        missing = bench_harness.unreachable_gold_entries(gold, note_texts)

        self.assertEqual(
            missing,
            [
                {
                    "question": "unreachable",
                    "relevant_note_ids": ["MISSING"],
                    "expected_patterns": ["billing_ledger"],
                }
            ],
        )


if __name__ == "__main__":
    unittest.main()
