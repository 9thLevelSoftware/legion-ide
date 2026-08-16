"""Tests for textkit.dates."""

import unittest

from textkit import dates


class MonthSpanTests(unittest.TestCase):
    def test_single_month(self):
        self.assertEqual(dates.month_span(2024, 5, 2024, 5), 1)

    def test_within_one_year(self):
        self.assertEqual(dates.month_span(2024, 1, 2024, 3), 3)

    def test_across_a_year_boundary(self):
        self.assertEqual(dates.month_span(2023, 11, 2024, 2), 4)

    def test_end_before_start_is_rejected(self):
        with self.assertRaises(ValueError):
            dates.month_span(2024, 5, 2024, 4)


class QuarterTests(unittest.TestCase):
    def test_quarter_boundaries(self):
        self.assertEqual(dates.quarter_of(1), 1)
        self.assertEqual(dates.quarter_of(3), 1)
        self.assertEqual(dates.quarter_of(4), 2)
        self.assertEqual(dates.quarter_of(12), 4)

    def test_out_of_range_is_rejected(self):
        with self.assertRaises(ValueError):
            dates.quarter_of(0)


if __name__ == "__main__":
    unittest.main()
