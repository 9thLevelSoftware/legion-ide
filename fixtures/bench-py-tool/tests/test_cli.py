"""Tests for the textkit CLI."""

import contextlib
import io
import unittest

from textkit import cli


def run_cli(argv):
    buffer = io.StringIO()
    with contextlib.redirect_stdout(buffer):
        exit_code = cli.main(argv)
    return exit_code, buffer.getvalue()


class StatsCommandTests(unittest.TestCase):
    def test_stats_reports_words_and_unique(self):
        exit_code, output = run_cli(["stats", "one two two three three three"])
        self.assertEqual(exit_code, 0)
        self.assertEqual(output, "words=6\nunique=3\n")


class SlugCommandTests(unittest.TestCase):
    def test_slug_prints_slugified_text(self):
        exit_code, output = run_cli(["slug", "Hello, World!"])
        self.assertEqual(exit_code, 0)
        self.assertEqual(output, "hello-world\n")


if __name__ == "__main__":
    unittest.main()
