"""Fixed-width table rendering for TextKit."""


def column_widths(header, rows):
    """Return the display width of each column.

    A column is as wide as the longest cell in it, counting the header as a
    cell. ``header`` is a list of column titles; ``rows`` is a list of equal
    length lists.
    """
    widths = []
    for index in range(len(header)):
        widest = 0
        for row in rows:
            widest = max(widest, len(str(row[index])))
        widths.append(widest)
    return widths


def render_row(cells, widths, separator=" | "):
    """Left-align each cell in its column width and join with ``separator``."""
    return separator.join(
        str(cell).ljust(width) for cell, width in zip(cells, widths)
    )
