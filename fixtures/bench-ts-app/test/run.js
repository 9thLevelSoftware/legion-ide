'use strict';

// Test runner. Usage:
//   node test/run.js              run every test/*.test.js
//   node test/run.js money csv    run test/money.test.js and test/csv.test.js
//
// Each test file runs in its own node process; the runner exits 0 only when
// every selected file exists and exits 0.

const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const names = process.argv.slice(2);
/** @type {string[]} */
let files;
if (names.length > 0) {
  files = names.map((name) => path.join(__dirname, name + '.test.js'));
} else {
  files = fs
    .readdirSync(__dirname)
    .filter((entry) => entry.endsWith('.test.js'))
    .sort()
    .map((entry) => path.join(__dirname, entry));
}

let failed = 0;
for (const file of files) {
  const rel = path.relative(path.join(__dirname, '..'), file).replace(/\\/g, '/');
  if (!fs.existsSync(file)) {
    console.error('FAIL ' + rel + ' (file not found)');
    failed += 1;
    continue;
  }
  const result = spawnSync(process.execPath, [file], { stdio: 'inherit' });
  if (result.status === 0) {
    console.log('PASS ' + rel);
  } else {
    console.error('FAIL ' + rel);
    failed += 1;
  }
}
process.exit(failed === 0 ? 0 : 1);
