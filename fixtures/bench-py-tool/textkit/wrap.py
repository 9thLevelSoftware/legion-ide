"""Greedy word wrapping for TextKit."""


def wrap_text(text, width):
    """Wrap ``text`` into lines of at most ``width`` characters.

    Splits on whitespace and re-joins words greedily with single spaces. A
    single word longer than ``width`` is emitted on its own line without
    being split. Returns a list of lines; empty or whitespace-only input
    yields an empty list.

    Raises ``ValueError`` when ``width`` is zero or negative.
    """
    if width <= 0:
        raise ValueError("width must be positive")
    lines = []
    current = ""
    for word in text.split():
        candidate = word if not current else current + " " + word
        if len(candidate) <= width:
            current = candidate
        else:
            if current:
                lines.append(current)
            current = word
    if current:
        lines.append(current)
    return lines
