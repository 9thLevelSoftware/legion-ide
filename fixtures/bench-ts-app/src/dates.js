'use strict';

/**
 * Return the ISO-8601 week-numbering year and week for a date.
 *
 * ISO weeks start on Monday and week 1 is the week containing the first
 * Thursday of the year, so the first days of January can belong to the last
 * week of the previous year.
 *
 * @param {Date} date
 * @returns {{year: number, week: number}}
 */
function isoWeek(date) {
  const target = new Date(Date.UTC(
    date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate()
  ));
  const day = target.getUTCDay() || 7;
  target.setUTCDate(target.getUTCDate() + 4 - day);
  const yearStart = new Date(Date.UTC(target.getUTCFullYear(), 0, 1));
  const week = Math.ceil((((target - yearStart) / 86400000) + 1) / 7);
  return { year: date.getUTCFullYear(), week: week };
}

/**
 * Format an ISO week as "YYYY-Www", zero padded.
 * @param {{year: number, week: number}} value
 * @returns {string}
 */
function formatIsoWeek(value) {
  return value.year + '-W' + String(value.week).padStart(2, '0');
}

module.exports = { isoWeek: isoWeek, formatIsoWeek: formatIsoWeek };
