'use strict';

/**
 * Format integer cents as a decimal dollar string, e.g. 1234 -> "12.34",
 * -5 -> "-0.05".
 * @param {number} cents
 * @returns {string}
 */
function formatCents(cents) {
  const sign = cents < 0 ? '-' : '';
  const abs = Math.abs(cents);
  const dollars = Math.floor(abs / 100);
  const rest = String(abs % 100).padStart(2, '0');
  return sign + dollars + '.' + rest;
}

/**
 * Render category totals as an aligned two-column report. Categories are
 * sorted alphabetically; amounts are right-aligned with two spaces between
 * the widest category name and the amount column.
 *
 * @param {Record<string, number>} totals - cents per category
 * @returns {string}
 */
function renderReport(totals) {
  const categories = Object.keys(totals).sort();
  const amounts = categories.map((category) => formatCents(totals[category]));
  const nameWidth = categories.reduce((w, c) => Math.max(w, c.length), 0);
  const amountWidth = amounts.reduce((w, a) => Math.max(w, a.length), 0);
  return categories
    .map((category, i) => category.padEnd(nameWidth) + '  ' + amounts[i].padStart(amountWidth))
    .join('\n');
}

module.exports = { renderReport };
