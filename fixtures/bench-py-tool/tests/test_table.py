"""Tests for textkit.table."""

import unittest

from textkit import table


class ColumnWidthTests(unittest.TestCase):
    def test_widest_cell_wins(self):
        widths = table.column_widths(["id", "name"], [["1", "alice"], ["2", "bo"]])
        self.assertEqual(widths, [2, 5])

    def test_header_counts_as_a_cell(self):
        widths = table.column_widths(["identifier", "n"], [["1", "alice"]])
        self.assertEqual(widths, [10, 5])

    def test_no_rows_falls_back_to_the_header(self):
        self.assertEqual(table.column_widths(["abc", "de"], []), [3, 2])


class RenderTests(unittest.TestCase):
    def test_cells_are_padded_to_their_column(self):
        self.assertEqual(table.render_row(["a", "b"], [3, 2]), "a   | b ")


if __name__ == "__main__":
    unittest.main()
