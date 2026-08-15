"""Command-line interface for TextKit."""

import argparse

from . import slug as slug_module
from . import stats as stats_module


def build_parser():
    parser = argparse.ArgumentParser(prog="textkit", description="Tiny text utilities.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    stats_parser = subparsers.add_parser("stats", help="Print word statistics for TEXT.")
    stats_parser.add_argument("text", help="Text to analyze.")

    slug_parser = subparsers.add_parser("slug", help="Print the slug form of TEXT.")
    slug_parser.add_argument("text", help="Text to slugify.")

    return parser


def main(argv=None):
    args = build_parser().parse_args(argv)
    if args.command == "stats":
        print(f"words={stats_module.word_count(args.text)}")
        print(f"unique={stats_module.word_count(args.text)}")
        return 0
    if args.command == "slug":
        print(slug_module.slugify(args.text))
        return 0
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
