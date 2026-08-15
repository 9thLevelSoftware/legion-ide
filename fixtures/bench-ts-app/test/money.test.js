'use strict';

const assert = require('assert');
const { check, report } = require('./harness');
const { parseAmount } = require('../src/money');

check('whole dollars parse as hundreds of cents', () => {
  assert.strictEqual(parseAmount('3'), 300);
  assert.strictEqual(parseAmount('0'), 0);
  assert.strictEqual(parseAmount('120'), 12000);
});

check('two decimal digits parse exactly', () => {
  assert.strictEqual(parseAmount('12.50'), 1250);
  assert.strictEqual(parseAmount('0.05'), 5);
  assert.strictEqual(parseAmount('99.99'), 9999);
});

check('one decimal digit means tenths of a dollar', () => {
  assert.strictEqual(parseAmount('12.5'), 1250);
  assert.strictEqual(parseAmount('0.5'), 50);
  assert.strictEqual(parseAmount('-4.2'), -420);
});

check('negative amounts parse with sign preserved', () => {
  assert.strictEqual(parseAmount('-4.05'), -405);
  assert.strictEqual(parseAmount('-1'), -100);
});

check('surrounding whitespace is tolerated', () => {
  assert.strictEqual(parseAmount(' 7.25 '), 725);
});

check('invalid inputs throw', () => {
  assert.throws(() => parseAmount(''));
  assert.throws(() => parseAmount('abc'));
  assert.throws(() => parseAmount('1.234'));
  assert.throws(() => parseAmount('1,50'));
  assert.throws(() => parseAmount('$5'));
});

report();
