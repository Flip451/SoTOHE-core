# Research Notes

## Version Baseline Workflow

At project bootstrap, run Gemini CLI to research latest stable versions for Rust/tooling/crates and store the result as:

- `version-baseline-YYYY-MM-DD.md`

Then reflect the decisions in:

- `Cargo.toml` (`rust-version`)
- `Dockerfile` (`RUST_VERSION` and tool versions)
- `track/tech-stack.md` (`Version Baseline`, MSRV, changelog)
