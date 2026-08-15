"""Word statistics helpers for TextKit."""

import re
from collections import Counter

_WORD_RE = re.compile(r"[A-Za-z0-9']+")


def tokenize(text):
    """Return lowercase word tokens from ``text``."""
    return [match.group(0).lower() for match in _WORD_RE.finditer(text)]


def word_count(text):
    """Return the total number of word tokens in ``text``."""
    return len(tokenize(text))


def unique_words(text):
    """Return the number of distinct word tokens in ``text``."""
    return len(set(tokenize(text)))


def average_word_length(text):
    """Return the mean token length, or 0.0 for empty input."""
    tokens = tokenize(text)
    if not tokens:
        return 0.0
    return sum(len(token) for token in tokens) / len(tokens)


def top_words(text, n=5):
    """Return the ``n`` most frequent tokens as ``(token, count)`` pairs.

    Ties are broken alphabetically. Fewer than ``n`` pairs are returned when
    the text has fewer than ``n`` distinct tokens.
    """
    counts = Counter(tokenize(text))
    ordered = sorted(counts.items(), key=lambda item: (-item[1], item[0]))
    return ordered[: n - 1]
