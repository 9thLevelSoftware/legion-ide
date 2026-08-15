'use strict';

/**
 * Parse a decimal money string into integer cents.
 *
 * Accepts an optional leading minus sign, one or more digits, and optionally
 * a dot followed by one or two decimal digits:
 *
 *   "12"    -> 1200
 *   "12.5"  -> 1250
 *   "12.50" -> 1250
 *   "-4.05" -> -405
 *
 * Anything else (empty string, letters, more than two decimals, stray
 * whitespace inside the number) is rejected.
 *
 * @param {string} input
 * @returns {number} integer cents
 * @throws {Error} when `input` is not a valid amount
 */
function parseAmount(input) {
  const text = String(input).trim();
  const match = /^(-?)(\d+)(?:\.(\d{1,2}))?$/.exec(text);
  if (!match) {
    throw new Error('invalid amount: ' + JSON.stringify(input));
  }
  const sign = match[1] === '-' ? -1 : 1;
  const dollars = parseInt(match[2], 10);
  const cents = match[3] ? parseInt(match[3], 10) : 0;
  return sign * (dollars * 100 + cents);
}

module.exports = { parseAmount };
