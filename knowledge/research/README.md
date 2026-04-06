# Research Notes

## Naming Convention

ファイル名は日時プレフィックス形式を使う:

- `YYYY-MM-DD-HHmm-<topic>.md`（例: `2026-04-06-1257-claude-code-reviewer-capability.md`）

## Version Baseline Workflow

At project bootstrap, run Gemini CLI to research latest stable versions for Rust/tooling/crates and store the result as:

- `YYYY-MM-DD-HHmm-version-baseline.md`

Then reflect the decisions in:

- `Cargo.toml` (`rust-version`)
- `Dockerfile` (`RUST_VERSION` and tool versions)
- `track/tech-stack.md` (`Version Baseline`, MSRV, changelog)
