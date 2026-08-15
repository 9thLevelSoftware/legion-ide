'use strict';

const assert = require('assert');
const { check, report } = require('./harness');
const { renderReport } = require('../src/report');

check('categories are sorted and amounts right-aligned', () => {
  const rendered = renderReport({ transit: 600, groceries: 2075, supplies: 475 });
  assert.strictEqual(
    rendered,
    ['groceries  20.75', 'supplies    4.75', 'transit     6.00'].join('\n')
  );
});

check('single category renders one line', () => {
  assert.strictEqual(renderReport({ rent: 120000 }), 'rent  1200.00');
});

check('negative totals keep their sign', () => {
  assert.strictEqual(renderReport({ refunds: -350 }), 'refunds  -3.50');
});

check('empty totals render an empty string', () => {
  assert.strictEqual(renderReport({}), '');
});

report();
