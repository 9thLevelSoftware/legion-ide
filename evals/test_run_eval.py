from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from evals.run_eval import _load_jsonl, _run_reviewer_fixture


ROOT = Path(__file__).resolve().parents[1]
REVIEW_FIXTURE = ROOT / "evals" / "fixtures" / "reviewer_seeded_bug.jsonl"


class ReviewerFixtureEvalTest(unittest.TestCase):
    def test_reviewer_fixture_flags_seeded_bug(self) -> None:
        examples = _load_jsonl(REVIEW_FIXTURE, None)
        result = _run_reviewer_fixture(examples)

        self.assertEqual(result["mode"], "reviewer-fixture")
        self.assertEqual(result["eval_count"], 3)
        self.assertEqual(result["summary"]["seeded_bug_detection_rate"], 1.0)

        seeded = next(row for row in result["results"] if row["seeded_bug"])
        self.assertTrue(seeded["seeded_bug_flagged"])
        self.assertTrue(seeded["high_risk_label"])

    def test_reviewer_fixture_cli_writes_output(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            output = Path(tmpdir) / "review.json"
            proc = subprocess.run(
                [
                    sys.executable,
                    str(ROOT / "evals" / "run_eval.py"),
                    "--reviewer-fixture",
                    "--dataset",
                    str(REVIEW_FIXTURE),
                    "--output",
                    str(output),
                ],
                check=True,
                capture_output=True,
                text=True,
            )

            self.assertIn('"mode": "reviewer-fixture"', proc.stdout)
            data = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(data["summary"]["seeded_bug_detection_rate"], 1.0)
            self.assertEqual(data["results"][1]["example_id"], "rev-002")
            self.assertTrue(data["results"][1]["seeded_bug_flagged"])


if __name__ == "__main__":
    unittest.main()
