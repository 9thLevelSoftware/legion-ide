#!/usr/bin/env node
'use strict';

const fs = require('fs');
const { parseCsv } = require('./csv');
const { parseAmount } = require('./money');
const { balance, totalsByCategory } = require('./ledger');
const { renderReport } = require('./report');

const EXPECTED_HEADER = 'date,category,description,amount';

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
 * Parse ledger CSV text (header row + records) into entries.
 * @param {string} text
 * @returns {import('./ledger').Entry[]}
 */
function entriesFromCsvText(text) {
  const rows = parseCsv(text);
  if (rows.length === 0 || rows[0].join(',') !== EXPECTED_HEADER) {
    throw new Error('expected header row: ' + EXPECTED_HEADER);
  }
  return rows.slice(1).map((row) => ({
    date: row[0],
    category: row[1],
    description: row[2],
    amountCents: parseAmount(row[3]),
  }));
}

function usage() {
  console.error('usage: node src/cli.js <command> <ledger.csv>');
  console.error('  balance <ledger.csv>   print the summed balance');
  console.error('  report <ledger.csv>    print per-category totals');
  process.exit(1);
}

/**
 * @param {string[]} argv
 */
function main(argv) {
  const command = argv[0];
  const file = argv[1];
  if (command === 'balance' && file) {
    const entries = entriesFromCsvText(fs.readFileSync(file, 'utf8'));
    console.log('balance: ' + formatCents(balance(entries)));
    return;
  }
  if (command === 'report' && file) {
    const entries = entriesFromCsvText(fs.readFileSync(file, 'utf8'));
    console.log(renderReport(totalsByCategory(entries)));
    return;
  }
  usage();
}

main(process.argv.slice(2));
