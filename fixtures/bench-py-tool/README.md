# TextKit

A tiny, stdlib-only Python text utility library used as a bench fixture.

- Zero external dependencies — no pip installs, no network access. Everything
  resolves from the Python standard library so the suite runs fully offline.
- Requires Python 3.9+.

## Layout

```
textkit/
  __init__.py   package marker
  stats.py      word statistics (tokenize, word_count, unique_words, top_words)
  slug.py       slugify(text) -> url-friendly slug
  wrap.py       wrap_text(text, width) greedy word wrapping
  cli.py        argparse CLI: `stats` and `slug` subcommands
tests/          unittest test modules (run from the repo root)
checks/         standalone verification scripts (not part of the test suite)
```

## Running tests

From the repository root:

```
python -m unittest tests.test_stats
python -m unittest tests.test_slug
python -m unittest tests.test_cli
```

Individual modules can be exercised via the CLI:

```
python -m textkit.cli stats "some text here"
python -m textkit.cli slug "Hello, World!"
```
