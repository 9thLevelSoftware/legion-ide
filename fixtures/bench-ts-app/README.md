# bench-ts-app

A small expense-ledger CLI written in plain JavaScript (CommonJS) with JSDoc
type annotations.

- Zero external dependencies. No `npm install` is ever required; everything
  runs with a stock `node` binary, fully offline.
- Requires Node.js 18+.

## Layout

```
src/
  money.js    parseAmount("12.50") -> integer cents
  csv.js      minimal CSV parser (quoted fields, escaped quotes)
  ledger.js   entry aggregation: balance, totalsByCategory, largestEntry
  report.js   renderReport(totals) -> aligned two-column text report
  cli.js      command dispatcher: balance | report
sample/
  expenses.csv  example ledger used by the CLI examples below
test/
  harness.js        tiny check/report test harness
  run.js            spawns test files: node test/run.js [name ...]
  *.test.js         test modules, one per src module
  verify-*.js       standalone verification scripts (not run by run.js)
```

## Running tests

From the repository root:

```
node test/run.js            # every test/*.test.js
node test/run.js money csv  # just test/money.test.js and test/csv.test.js
```

Each test file is also directly runnable (`node test/money.test.js`) and
exits 0 on success, 1 on any failure.

## CLI usage

```
node src/cli.js balance sample/expenses.csv
node src/cli.js report sample/expenses.csv
```

CSV input format: a `date,category,description,amount` header row followed by
one record per line. Descriptions containing commas are double-quoted;
amounts are decimal dollar strings such as `12.50`.

## Conventions

- CommonJS modules (`require` / `module.exports`), `'use strict'` at the top
  of every file.
- Money is handled as integer cents everywhere except at the parse/format
  boundary.
- Tests use `test/harness.js`: `check(name, fn)` with `node:assert` inside,
  then `report()` at the end of the file.
