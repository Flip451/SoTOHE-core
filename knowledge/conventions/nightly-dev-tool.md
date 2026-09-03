# Nightly Toolchain Dev-Tool Convention

## Purpose

Rules for using Rust nightly as a dev-only tool while keeping crates on stable.

## Scope

- Applies to: `sotp domain export-schema` and any future command that requires rustdoc JSON output
- Does not apply to: normal build, test, CI pipelines (these remain stable-only)

> **強制先**: review 観点 — harness-policy scope

## Rules

- Crate code must compile and pass tests on **stable** Rust (MSRV 1.91). Nightly is never required for `cargo build` or `cargo test`
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope
- Nightly is used **only** for `cargo +nightly rustdoc -- -Z unstable-options --output-format json`
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope
- When nightly is not installed, the command must return `SchemaExportError::NightlyNotFound` (fail-closed). It must not panic or silently degrade
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope
- `rustdoc-types` crate version must match the rustdoc JSON format version produced by the pinned nightly. Document the expected nightly version in this file when pinned
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope
- Tests that require nightly must be marked `#[ignore]` with a comment explaining the nightly dependency. Add an explicit CI path only when nightly coverage is adopted.
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope
- Do not add `rust-toolchain.toml` with `channel = "nightly"` — this would force nightly for all developers
  > **強制先**: 強制なし (明記) — root rust-toolchain.toml の nightly 設定を判定する既存機構なし
- `export-schema` is primarily a host-side dev tool; nightly is not part of the default toolchain
  > **強制先**: review 観点 — infrastructure / cli / cli_driver / cli_composition scope

## Examples

- Good: `Command::new("cargo").args(["+nightly", "rustdoc", "-p", crate_name, "--", "-Z", "unstable-options", "--output-format", "json"])`
- Bad: Adding `#![feature(...)]` to any crate source file
- Bad: Using nightly-only Rust syntax (e.g., `gen fn`, `async gen`) in production code

## Exceptions

- None currently. If a nightly-only feature is needed in production code, it requires an ADR
  > **強制先**: review 観点 — adr scope

## Review Checklist

- [ ] No `#![feature(...)]` in crate source
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope
- [ ] `cargo make test` passes on stable without nightly installed
  > **強制先**: 機械 lint — cargo make test
- [ ] Nightly-dependent code paths return a clear error when nightly is absent
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope
- [ ] `rustdoc-types` version is compatible with the nightly rustdoc format
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope

## Related Documents

- `architecture-rules.json` — rustdoc JSON parsing stays in the permitted infrastructure layer
- `knowledge/adr/README.md` — rustdoc_types-based TDDD TypeGraph を含む設計判断の索引
