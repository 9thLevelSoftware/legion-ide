'use strict';

const assert = require('assert');
const { check, report } = require('./harness');
const { parseLine, parseCsv } = require('../src/csv');

check('plain fields split on commas', () => {
  assert.deepStrictEqual(parseLine('a,b,c'), ['a', 'b', 'c']);
  assert.deepStrictEqual(parseLine('one'), ['one']);
});

check('empty fields are preserved', () => {
  assert.deepStrictEqual(parseLine('a,,c'), ['a', '', 'c']);
  assert.deepStrictEqual(parseLine(',b,'), ['', 'b', '']);
});

check('quoted fields may contain commas', () => {
  assert.deepStrictEqual(parseLine('a,"b,c",d'), ['a', 'b,c', 'd']);
  assert.deepStrictEqual(parseLine('"x,y"'), ['x,y']);
});

check('a quoted empty field parses to an empty string', () => {
  assert.deepStrictEqual(parseLine('""'), ['']);
});

check('doubled quotes inside a quoted field are literal quotes', () => {
  assert.deepStrictEqual(parseLine('"say ""hi""",b'), ['say "hi"', 'b']);
  assert.deepStrictEqual(parseLine('"""quoted""",x'), ['"quoted"', 'x']);
});

check('parseCsv splits rows and drops a single trailing newline', () => {
  assert.deepStrictEqual(parseCsv('a,b\nc,d\n'), [
    ['a', 'b'],
    ['c', 'd'],
  ]);
  assert.deepStrictEqual(parseCsv('a,b\r\nc,d'), [
    ['a', 'b'],
    ['c', 'd'],
  ]);
});

report();
