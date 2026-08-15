'use strict';

/**
 * Parse a single CSV record (one line, no trailing newline) into its fields.
 *
 * Fields are separated by commas. A field may be wrapped in double quotes,
 * in which case it can contain commas. Inside a quoted field, a doubled
 * quote (`""`) is an escaped literal quote character.
 *
 * @param {string} line
 * @returns {string[]}
 */
function parseLine(line) {
  /** @type {string[]} */
  const fields = [];
  let field = '';
  let inQuotes = false;
  for (let i = 0; i < line.length; i += 1) {
    const ch = line[i];
    if (inQuotes) {
      if (ch === '"') {
        inQuotes = false;
      } else {
        field += ch;
      }
    } else if (ch === '"') {
      inQuotes = true;
    } else if (ch === ',') {
      fields.push(field);
      field = '';
    } else {
      field += ch;
    }
  }
  fields.push(field);
  return fields;
}

/**
 * Parse CSV text into rows of fields. Handles `\n` and `\r\n` line endings
 * and ignores a single trailing newline.
 *
 * @param {string} text
 * @returns {string[][]}
 */
function parseCsv(text) {
  const normalized = text.replace(/\r\n/g, '\n');
  const lines = normalized.split('\n');
  if (lines.length > 0 && lines[lines.length - 1] === '') {
    lines.pop();
  }
  return lines.map(parseLine);
}

module.exports = { parseLine, parseCsv };
