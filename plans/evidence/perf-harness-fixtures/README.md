# Perf harness fixture benchmarks

These manifests back the `xtask perf-harness` large-fixture search benchmarks.
They are read by the harness at runtime and must not be regenerated as a side
effect of running the benchmark command.

- `50k-file-search.toml`: bounded search over a 50K-file fixture corpus
- `100k-file-search.toml`: bounded search over a 100K-file fixture corpus
