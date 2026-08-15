'use strict';

/**
 * @typedef {Object} Entry
 * @property {string} date - ISO date string, "YYYY-MM-DD"
 * @property {string} category
 * @property {string} description
 * @property {number} amountCents - integer cents
 */

/**
 * Sum of all entry amounts, in cents. Empty input sums to 0.
 * @param {Entry[]} entries
 * @returns {number}
 */
function balance(entries) {
  return entries.reduce((sum, entry) => sum + entry.amountCents, 0);
}

/**
 * Total cents per category.
 * @param {Entry[]} entries
 * @returns {Record<string, number>}
 */
function totalsByCategory(entries) {
  /** @type {Record<string, number>} */
  const totals = {};
  for (const entry of entries) {
    totals[entry.category] = (totals[entry.category] || 0) + entry.amountCents;
  }
  return totals;
}

/**
 * The entries belonging to `category`, in their original order.
 * @param {Entry[]} entries
 * @param {string} category
 * @returns {Entry[]}
 */
function entriesInCategory(entries, category) {
  return entries.filter((entry) => entry.category === category);
}

/**
 * The entry with the largest amount. Ties keep the earliest entry in the
 * list. Returns null for an empty list.
 * @param {Entry[]} entries
 * @returns {Entry | null}
 */
function largestEntry(entries) {
  let best = null;
  for (const entry of entries) {
    if (best === null || entry.amountCents > best.amountCents) {
      best = entry;
    }
  }
  return best;
}

module.exports = { balance, totalsByCategory, entriesInCategory, largestEntry };
