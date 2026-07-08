# Research Notes

## Naming Convention

ファイル名は日時プレフィックス形式を使う:

- `YYYY-MM-DD-HHmm-<topic>.md`（例: `2026-04-06-1257-crate-survey.md`）

## Version Baseline Workflow

At project bootstrap, run the researcher capability to research latest stable versions for Rust/tooling/crates and store the result as:

- `YYYY-MM-DD-HHmm-version-baseline.md`

Then reflect the decisions in:

- `Cargo.toml` (`rust-version`)
- `Dockerfile` (`RUST_VERSION` and tool versions)
- a pre-track ADR under `knowledge/adr/` when the version choice is a decision worth recording
