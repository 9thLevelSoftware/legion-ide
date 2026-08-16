'use strict';

const { check, report } = require('./harness');
const { isoWeek, formatIsoWeek } = require('../src/dates');

function assertEqual(actual, expected, label) {
  const a = JSON.stringify(actual);
  const e = JSON.stringify(expected);
  if (a !== e) {
    throw new Error(label + ': expected ' + e + ' but got ' + a);
  }
}

check('a mid-year date reports its own year', function () {
  assertEqual(isoWeek(new Date(Date.UTC(2024, 5, 12))), { year: 2024, week: 24 }, 'june');
});

check('1 January 2021 belongs to week 53 of 2020', function () {
  assertEqual(isoWeek(new Date(Date.UTC(2021, 0, 1))), { year: 2020, week: 53 }, 'jan 1 2021');
});

check('31 December 2019 belongs to week 1 of 2020', function () {
  assertEqual(isoWeek(new Date(Date.UTC(2019, 11, 31))), { year: 2020, week: 1 }, 'dec 31 2019');
});

check('formatting zero pads the week', function () {
  assertEqual(formatIsoWeek({ year: 2024, week: 7 }), '2024-W07', 'padding');
});

report();
