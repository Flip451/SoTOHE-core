<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# llvm-cov を nextest 経路に統一

## Summary

Single-task plan targeting the one-line args change in `Makefile.toml` `llvm-cov-local`. The change inserts the `nextest` subcommand and adds `--locked` so `cargo llvm-cov` uses the same per-process-isolated nextest harness as `cargo make test`, resolving the libtest-vs-nextest harness divergence documented in ADR D1.

## Tasks (1/1 resolved)

### S001 — Makefile.toml: switch llvm-cov-local to nextest harness

> Update the `args` array of the `[tasks.llvm-cov-local]` entry in `Makefile.toml` (L445-449) to `["llvm-cov", "nextest", "--html", "--all-features", "--locked"]`. This is the only file change in the track. The outer `llvm-cov` compose-wrapper task is unaffected — it delegates to `llvm-cov-local` without change, preserving the public `cargo make llvm-cov` interface (CN-02). After the change, verify via `cargo make llvm-cov` that all tests pass and the HTML report is generated (AC-01, AC-02), then confirm `cargo make ci` passes end-to-end (AC-03).

- [x] **T001**: Change `llvm-cov-local` task args in `Makefile.toml` from `["llvm-cov", "--html", "--all-features"]` to `["llvm-cov", "nextest", "--html", "--all-features", "--locked"]`, aligning the coverage harness with `cargo make test` (nextest). No other files are modified. (`194eaa924faaa94c9140f705018d7de9c85a7d84`)
