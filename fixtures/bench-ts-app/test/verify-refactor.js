'use strict';

// Standalone verification for the formatCents deduplication refactor.
// Passes only when:
//   - src/money.js exports a correct formatCents alongside parseAmount,
//   - src/report.js and src/cli.js no longer define their own copies and
//     instead require it from './money',
//   - report rendering and CLI behavior are unchanged.

const fs = require('fs');
const path = require('path');
const assert = require('assert');
const { spawnSync } = require('child_process');
const { check, report } = require('./harness');

const root = path.join(__dirname, '..');

check('src/money.js exports formatCents', () => {
  const money = require('../src/money');
  assert.strictEqual(typeof money.formatCents, 'function');
  assert.strictEqual(money.formatCents(1234), '12.34');
  assert.strictEqual(money.formatCents(-5), '-0.05');
  assert.strictEqual(money.formatCents(0), '0.00');
  assert.strictEqual(money.formatCents(100), '1.00');
});

check('src/money.js still exports parseAmount', () => {
  const money = require('../src/money');
  assert.strictEqual(typeof money.parseAmount, 'function');
  assert.strictEqual(money.parseAmount('12.50'), 1250);
});

check('src/report.js has no local formatCents and requires ./money', () => {
  const source = fs.readFileSync(path.join(root, 'src', 'report.js'), 'utf8');
  assert.ok(
    !/function\s+formatCents/.test(source),
    'src/report.js still defines its own formatCents'
  );
  assert.ok(
    /require\(['"]\.\/money['"]\)/.test(source),
    'src/report.js does not require ./money'
  );
});

check('src/cli.js has no local formatCents', () => {
  const source = fs.readFileSync(path.join(root, 'src', 'cli.js'), 'utf8');
  assert.ok(
    !/function\s+formatCents/.test(source),
    'src/cli.js still defines its own formatCents'
  );
});

check('renderReport output is unchanged', () => {
  const { renderReport } = require('../src/report');
  const rendered = renderReport({ transit: 600, groceries: 2075, supplies: 475 });
  assert.strictEqual(
    rendered,
    ['groceries  20.75', 'supplies    4.75', 'transit     6.00'].join('\n')
  );
});

check('cli balance output is unchanged', () => {
  const cli = path.join(root, 'src', 'cli.js');
  const sample = path.join(root, 'sample', 'expenses.csv');
  const result = spawnSync(process.execPath, [cli, 'balance', sample], {
    encoding: 'utf8',
  });
  assert.strictEqual(result.status, 0, 'cli exited ' + result.status + ': ' + result.stderr);
  assert.strictEqual(result.stdout.replace(/\r\n/g, '\n').trimEnd(), 'balance: 31.50');
});

report();
