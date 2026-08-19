"""Tests for the consent gate that stands between an export and a GPU.

These run without torch. The gate is the part of the trainer that has to hold
when nobody is watching, and requiring a CUDA box to test it would mean it is
never tested.
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from training.qlora_train import (  # noqa: E402
    ConsentRefusal,
    assert_dataset_is_consented,
    build_supervised_batch,
    label_token_ids,
    render_prompt,
)


def manifest(**overrides):
    base = {
        "schema_version": 1,
        "prompt_template_version": "legion-consented-decision-v1",
        "corpus_id": "consented-accept-reject-v1",
        "source_trace_count": 10,
        "candidate_count": 4,
        "skipped_unconsented_count": 5,
        "skipped_non_terminal_count": 1,
        "accepted_count": 2,
        "rejected_count": 2,
        "corpus_fingerprint": "training-corpus-v1:0000000000000000",
        "dataset_fingerprint": "training-adapter-v1:0000000000000000",
        "train_count": 3,
        "holdout_count": 1,
        "holdout_every": 4,
        "expand_seed": 1,
        "expand_to": 10,
        "retained_consent_states": {"Granted": 3, "NotRequired": 1},
        "comparison": {"baseline_id": "legion-bench-v0"},
    }
    base.update(overrides)
    return base


def examples(count=3, start=0):
    return [
        {
            "example_id": f"training-candidate:assist:audit:req-{start + i}:1:accepted",
            "instruction": "Legion proposal review.\ndecision:",
            "output": "Accepted",
            "split": "train",
        }
        for i in range(count)
    ]


class ConsentGateTest(unittest.TestCase):
    def test_a_consented_export_passes(self) -> None:
        provenance = assert_dataset_is_consented(manifest(), examples(3), "train")
        self.assertEqual(provenance["candidate_count"], 4)
        self.assertEqual(provenance["skipped_unconsented_count"], 5)
        self.assertEqual(
            provenance["retained_consent_states"], {"Granted": 3, "NotRequired": 1}
        )

    def test_a_denied_consent_state_refuses_training(self) -> None:
        tampered = manifest(
            retained_consent_states={"Granted": 3, "Denied": 1}, candidate_count=4
        )
        with self.assertRaises(ConsentRefusal) as ctx:
            assert_dataset_is_consented(tampered, examples(3), "train")
        self.assertIn("Denied", str(ctx.exception))

    def test_every_unconsented_state_is_refused(self) -> None:
        for state in ("Denied", "Missing", "RenewalRequired", "SomethingNew"):
            with self.subTest(state=state):
                tampered = manifest(
                    retained_consent_states={"Granted": 3, state: 1}, candidate_count=4
                )
                with self.assertRaises(ConsentRefusal):
                    assert_dataset_is_consented(tampered, examples(3), "train")

    def test_a_hidden_consent_state_is_refused_by_the_count_check(self) -> None:
        # Deleting the offending row from retained_consent_states leaves the
        # counts short of candidate_count, which is the point of checking both.
        tampered = manifest(retained_consent_states={"Granted": 3}, candidate_count=4)
        with self.assertRaises(ConsentRefusal) as ctx:
            assert_dataset_is_consented(tampered, examples(3), "train")
        self.assertIn("unaccounted", str(ctx.exception))

    def test_a_line_appended_after_export_is_refused(self) -> None:
        with self.assertRaises(ConsentRefusal) as ctx:
            assert_dataset_is_consented(manifest(), examples(4), "train")
        self.assertIn("changed after export", str(ctx.exception))

    def test_a_line_without_an_example_id_is_refused(self) -> None:
        rows = examples(3)
        del rows[1]["example_id"]
        with self.assertRaises(ConsentRefusal) as ctx:
            assert_dataset_is_consented(manifest(), rows, "train")
        self.assertIn("no example_id", str(ctx.exception))

    def test_a_duplicated_example_id_is_refused(self) -> None:
        rows = examples(3)
        rows[2]["example_id"] = rows[0]["example_id"]
        with self.assertRaises(ConsentRefusal) as ctx:
            assert_dataset_is_consented(manifest(), rows, "train")
        self.assertIn("duplicated", str(ctx.exception))

    def test_an_unknown_manifest_schema_is_refused(self) -> None:
        with self.assertRaises(ConsentRefusal):
            assert_dataset_is_consented(manifest(schema_version=2), examples(3), "train")

    def test_a_manifest_without_consent_states_is_refused(self) -> None:
        with self.assertRaises(ConsentRefusal):
            assert_dataset_is_consented(
                manifest(retained_consent_states={}), examples(3), "train"
            )

    def test_the_holdout_split_is_checked_against_its_own_count(self) -> None:
        assert_dataset_is_consented(manifest(), examples(1), "holdout")
        with self.assertRaises(ConsentRefusal):
            assert_dataset_is_consented(manifest(), examples(2), "holdout")


class SupervisedBatchTest(unittest.TestCase):
    class FakeTokenizer:
        """Character-per-token tokenizer: enough to assert the masking shape."""

        eos_token = "<eos>"
        pad_token_id = 0

        def __call__(self, text, add_special_tokens=True):
            return {"input_ids": [ord(ch) for ch in text.replace("<eos>", "\x00")]}

    def test_prompt_tokens_are_masked_and_completion_tokens_are_not(self) -> None:
        tokenizer = self.FakeTokenizer()
        rows = [{"instruction": "abc", "output": "Ok"}]
        input_ids, attention, labels = build_supervised_batch(tokenizer, rows, 64)

        # "abc " is 4 prompt tokens; "Ok" + eos is 3 completion tokens.
        self.assertEqual(labels[0][:4], [-100] * 4)
        self.assertEqual(labels[0][4:], input_ids[0][4:])
        self.assertEqual(attention[0], [1] * len(input_ids[0]))

    def test_padding_is_masked_out_of_both_labels_and_attention(self) -> None:
        tokenizer = self.FakeTokenizer()
        rows = [
            {"instruction": "abcdefgh", "output": "Ok"},
            {"instruction": "ab", "output": "Ok"},
        ]
        input_ids, attention, labels = build_supervised_batch(tokenizer, rows, 64)

        self.assertEqual(len(input_ids[0]), len(input_ids[1]))
        padding = len(input_ids[0]) - (3 + 3)
        self.assertEqual(attention[1][-padding:], [0] * padding)
        self.assertEqual(labels[1][-padding:], [-100] * padding)
        self.assertEqual(input_ids[1][-padding:], [tokenizer.pad_token_id] * padding)


class TrainEvalTokenAgreementTest(unittest.TestCase):
    """The eval must score the exact token sequence training taught.

    This is the test the first version of the harness did not have. Training
    showed the model ``"...decision: "`` then ``"Accepted"``; the eval scored
    ``"...decision:"`` then ``" Accepted"``, which on a BPE tokenizer is a
    different final token. The trained arm scored 47.1% against a 52.9%
    majority-class floor and looked like a failed training run. It was a failed
    *measurement*, and the training loss curve could not tell the difference.
    """

    class SpacedTokenizer:
        """Merges a space into the following word, the way BPE tokenizers do.

        Without the merge this test passes under the old, broken construction
        too, and proves nothing.
        """

        eos_token = "<eos>"
        pad_token_id = 0

        def __call__(self, text, add_special_tokens=True):
            ids = []
            index = 0
            while index < len(text):
                if text.startswith("<eos>", index):
                    ids.append(1)
                    index += 5
                elif text[index] == " " and index + 1 < len(text):
                    # " X" is one token, distinct from the token for "X".
                    ids.append(100_000 + ord(text[index + 1]))
                    index += 2
                else:
                    ids.append(ord(text[index]))
                    index += 1
            return {"input_ids": ids}

    def test_the_eval_prompt_and_label_reproduce_the_training_tokens(self) -> None:
        tokenizer = self.SpacedTokenizer()
        row = {"instruction": "risk: Low\ndecision:", "output": "Accepted"}

        input_ids, _, labels = build_supervised_batch(tokenizer, [row], 64)
        eos_ids = tokenizer(tokenizer.eos_token, add_special_tokens=False)["input_ids"]
        trained_tokens = input_ids[0][: len(input_ids[0]) - len(eos_ids)]

        scored_tokens = (
            tokenizer(render_prompt(row["instruction"]), add_special_tokens=False)[
                "input_ids"
            ]
            + label_token_ids(tokenizer, row["output"])
        )
        self.assertEqual(
            trained_tokens,
            scored_tokens,
            "the eval scores tokens the trainer never showed the model",
        )

        # And the construction the eval used to have is genuinely different, so
        # the assertion above is not satisfied by any pair of strings.
        stale_tokens = (
            tokenizer(row["instruction"], add_special_tokens=False)["input_ids"]
            + tokenizer(f" {row['output']}", add_special_tokens=False)["input_ids"]
        )
        self.assertNotEqual(trained_tokens, stale_tokens)

    def test_the_completion_is_the_only_thing_carrying_a_label(self) -> None:
        tokenizer = self.SpacedTokenizer()
        row = {"instruction": "risk: Low\ndecision:", "output": "Accepted"}
        _, _, labels = build_supervised_batch(tokenizer, [row], 64)

        prompt_len = len(
            tokenizer(render_prompt(row["instruction"]), add_special_tokens=False)[
                "input_ids"
            ]
        )
        self.assertEqual(labels[0][:prompt_len], [-100] * prompt_len)
        self.assertTrue(all(token != -100 for token in labels[0][prompt_len:]))


class RealModeCliTest(unittest.TestCase):
    def test_real_mode_refuses_to_train_without_a_consent_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            dataset = Path(tmpdir) / "train.jsonl"
            dataset.write_text(
                "\n".join(json.dumps(row) for row in examples(3)) + "\n",
                encoding="utf-8",
            )
            proc = subprocess.run(
                [
                    sys.executable,
                    str(ROOT / "training" / "qlora_train.py"),
                    "--dataset",
                    str(dataset),
                    "--output-dir",
                    str(Path(tmpdir) / "out"),
                    "--max-steps",
                    "1",
                    "--device",
                    "cpu",
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(proc.returncode, 2, proc.stderr)
            self.assertIn("--consent-manifest is required", proc.stderr)
            self.assertFalse((Path(tmpdir) / "out").exists())


if __name__ == "__main__":
    unittest.main()
