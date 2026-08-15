"""Tests for textkit.stats."""

import unittest

from textkit import stats


class TokenizeTests(unittest.TestCase):
    def test_lowercases_and_splits_on_punctuation(self):
        self.assertEqual(stats.tokenize("Hello, World!"), ["hello", "world"])

    def test_empty_input(self):
        self.assertEqual(stats.tokenize(""), [])


class CountTests(unittest.TestCase):
    def test_word_count(self):
        self.assertEqual(stats.word_count("one two two three"), 4)

    def test_unique_words(self):
        self.assertEqual(stats.unique_words("one two two three"), 3)

    def test_average_word_length(self):
        self.assertEqual(stats.average_word_length("aa bbbb"), 3.0)

    def test_average_word_length_empty(self):
        self.assertEqual(stats.average_word_length(""), 0.0)


class TopWordsTests(unittest.TestCase):
    def test_top_words_returns_requested_count(self):
        text = "red red red blue blue green"
        self.assertEqual(stats.top_words(text, 2), [("red", 3), ("blue", 2)])

    def test_top_words_breaks_ties_alphabetically(self):
        text = "beta alpha beta alpha gamma"
        self.assertEqual(
            stats.top_words(text, 3),
            [("alpha", 2), ("beta", 2), ("gamma", 1)],
        )

    def test_top_words_with_fewer_distinct_tokens_than_n(self):
        self.assertEqual(stats.top_words("a b a", 5), [("a", 2), ("b", 1)])


if __name__ == "__main__":
    unittest.main()
