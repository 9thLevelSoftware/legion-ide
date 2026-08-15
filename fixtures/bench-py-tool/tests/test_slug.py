"""Tests for textkit.slug."""

import unittest

from textkit.slug import slugify


class SlugifyTests(unittest.TestCase):
    def test_basic_sentence(self):
        self.assertEqual(slugify("Hello, World!"), "hello-world")

    def test_collapses_runs_of_separators(self):
        self.assertEqual(slugify("a  --  b"), "a-b")

    def test_keeps_digits(self):
        self.assertEqual(slugify("Route 66"), "route-66")

    def test_strips_leading_and_trailing_separators(self):
        self.assertEqual(slugify("  spaced out  "), "spaced-out")

    def test_no_alphanumerics_yields_empty_string(self):
        self.assertEqual(slugify("!!! --- !!!"), "")

    def test_empty_input(self):
        self.assertEqual(slugify(""), "")


if __name__ == "__main__":
    unittest.main()
