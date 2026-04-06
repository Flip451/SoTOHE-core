# Nightly Toolchain Dev-Tool Convention

## Purpose

Rules for using Rust nightly as a dev-only tool while keeping crates on stable.

## Scope

- Applies to: `sotp domain export-schema` and any future command that requires rustdoc JSON output
- Does not apply to: normal build, test, CI pipelines (these remain stable-only)

## Rules

- Crate code must compile and pass tests on **stable** Rust (MSRV 1.85). Nightly is never required for `cargo build` or `cargo test`
- Nightly is used **only** for `cargo +nightly rustdoc -- -Z unstable-options --output-format json`
- When nightly is not installed, the command must return `SchemaExportError::NightlyNotFound` (fail-closed). It must not panic or silently degrade
- `rustdoc-types` crate version must match the rustdoc JSON format version produced by the pinned nightly. Document the expected nightly version in this file when pinned
- Tests that require nightly must be marked `#[ignore]` with a comment explaining the nightly dependency. A separate `cargo make test-nightly` task will run them (future)
- Do not add `rust-toolchain.toml` with `channel = "nightly"` — this would force nightly for all developers
- Docker images: nightly installation in the tools container is optional. `export-schema` is primarily a host-side dev tool

## Examples

- Good: `Command::new("cargo").args(["+nightly", "rustdoc", "-p", crate_name, "--", "-Z", "unstable-options", "--output-format", "json"])`
- Bad: Adding `#![feature(...)]` to any crate source file
- Bad: Using nightly-only Rust syntax (e.g., `gen fn`, `async gen`) in production code

## Exceptions

- None currently. If a nightly-only feature is needed in production code, it requires an ADR and tech-stack.md update

## Review Checklist

- [ ] No `#![feature(...)]` in crate source
- [ ] `cargo make test` passes on stable without nightly installed
- [ ] Nightly-dependent code paths return a clear error when nightly is absent
- [ ] `rustdoc-types` version is compatible with the nightly rustdoc format

## Related Documents

- `track/tech-stack.md` — Dev-only Tooling (nightly) section
- `knowledge/conventions/hexagonal-architecture.md` — rustdoc JSON parsing stays in infrastructure layer
- `track/items/bridge01-export-schema-2026-04-06/spec.json` — BRIDGE-01 spec
