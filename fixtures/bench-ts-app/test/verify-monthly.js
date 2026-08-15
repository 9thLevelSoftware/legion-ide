'use strict';

// Standalone verification for the monthly-totals feature:
//   - src/monthly.js exports groupByMonth(entries) and renderMonthly(byMonth),
//   - the CLI gains a `monthly <ledger.csv>` command printing the rendering.

const path = require('path');
const assert = require('assert');
const { spawnSync } = require('child_process');
const { check, report } = require('./harness');

const root = path.join(__dirname, '..');

check('groupByMonth totals cents per YYYY-MM', () => {
  const { groupByMonth } = require('../src/monthly');
  const entries = [
    { date: '2026-03-01', category: 'a', description: '', amountCents: 1250 },
    { date: '2026-03-15', category: 'b', description: '', amountCents: 300 },
    { date: '2026-04-02', category: 'a', description: '', amountCents: 825 },
  ];
  assert.deepStrictEqual(groupByMonth(entries), { '2026-03': 1550, '2026-04': 825 });
});

check('groupByMonth of no entries is an empty object', () => {
  const { groupByMonth } = require('../src/monthly');
  assert.deepStrictEqual(groupByMonth([]), {});
});

check('renderMonthly sorts months ascending and formats cents', () => {
  const { renderMonthly } = require('../src/monthly');
  assert.strictEqual(
    renderMonthly({ '2026-04': 825, '2026-03': 1550 }),
    '2026-03 15.50\n2026-04 8.25'
  );
});

check('renderMonthly of an empty object is an empty string', () => {
  const { renderMonthly } = require('../src/monthly');
  assert.strictEqual(renderMonthly({}), '');
});

check('cli monthly command renders the sample ledger', () => {
  const cli = path.join(root, 'src', 'cli.js');
  const sample = path.join(root, 'sample', 'expenses.csv');
  const result = spawnSync(process.execPath, [cli, 'monthly', sample], {
    encoding: 'utf8',
  });
  assert.strictEqual(result.status, 0, 'cli exited ' + result.status + ': ' + result.stderr);
  assert.strictEqual(
    result.stdout.replace(/\r\n/g, '\n').trimEnd(),
    '2026-03 23.75\n2026-04 7.75'
  );
});

check('cli balance command still works', () => {
  const cli = path.join(root, 'src', 'cli.js');
  const sample = path.join(root, 'sample', 'expenses.csv');
  const result = spawnSync(process.execPath, [cli, 'balance', sample], {
    encoding: 'utf8',
  });
  assert.strictEqual(result.status, 0, 'cli exited ' + result.status + ': ' + result.stderr);
  assert.strictEqual(result.stdout.replace(/\r\n/g, '\n').trimEnd(), 'balance: 31.50');
});

report();
