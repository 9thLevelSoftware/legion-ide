'use strict';

let passes = 0;
let failures = 0;

/**
 * Run one named check. Failures are reported and counted, never thrown.
 * @param {string} name
 * @param {() => void} fn
 */
function check(name, fn) {
  try {
    fn();
    passes += 1;
    console.log('ok - ' + name);
  } catch (err) {
    failures += 1;
    console.error('FAIL - ' + name + ': ' + (err && err.message));
  }
}

/**
 * Print a summary and exit: 0 when every check passed, 1 otherwise.
 * @returns {never}
 */
function report() {
  console.log(passes + ' passed, ' + failures + ' failed');
  process.exit(failures === 0 ? 0 : 1);
}

module.exports = { check, report };
