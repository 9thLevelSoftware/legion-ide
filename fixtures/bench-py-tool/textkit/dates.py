"""Calendar span helpers for TextKit."""


def month_span(start_year, start_month, end_year, end_month):
    """Return the inclusive number of months from start to end.

    ``month_span(2024, 1, 2024, 3)`` is 3 — January, February and March. The
    end must not precede the start; ``ValueError`` is raised when it does.
    """
    if (end_year, end_month) < (start_year, start_month):
        raise ValueError("end must not precede start")
    return (end_year - start_year) * 12 + (end_month - start_month)


def quarter_of(month):
    """Return the 1-based calendar quarter containing ``month`` (1-12)."""
    if not 1 <= month <= 12:
        raise ValueError("month must be between 1 and 12")
    return (month - 1) // 3 + 1
