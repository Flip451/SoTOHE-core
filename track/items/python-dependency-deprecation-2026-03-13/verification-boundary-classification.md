# Verification Boundary Classification

Date: 2026-03-13
Track: `python-dependency-deprecation-2026-03-13`

## Required Path Already Moved To Rust

| Entrypoint | Current owner | Reason |
| --- | --- | --- |
| `scripts/verify_plan_progress.py` | `sotp track views validate` | `plan.md` drift is now checked from Rust-rendered views |
| `scripts/verify_track_metadata.py` | `sotp track views validate` | metadata validation already lives in Rust track codecs/renderers |
| `scripts/verify_track_registry.py` | `sotp track views validate` | registry drift is the same rendered-view validation boundary |
| `scripts/git_ops.py` / `scripts/branch_switch.py` / `scripts/pr_merge.py` | `sotp git` / `sotp pr` | workflow-critical wrappers no longer require Python |

## Keep As Python Until Source Of Truth Is Redesigned

| Entrypoint | Why not Rust yet | Exit condition |
| --- | --- | --- |
| `scripts/verify_orchestra_guardrails.py` | validates `.claude/settings.json` allowlists and launcher strings directly; guardrail SSoT is still the settings file itself | move hook/permission schema to a Rust-readable canonical config model |
| `scripts/verify_latest_track_files.py` | latest-track selection still depends on file-tree scanning policy outside the Rust track aggregate | define branch-aware latest-track resolution in the track domain/infrastructure |
| `scripts/verify_tech_stack_ready.py` | planning completeness rules still depend on markdown/file-presence conventions rather than domain types | move planning readiness rules behind a typed track/project policy |

## Optional Python Utilities For Now

| Entrypoint | Why optional |
| --- | --- |
| `scripts/check_layers.py` | architecture-rule validation is useful in CI, but it is driven by `docs/architecture-rules.json` and `cargo metadata`; it is not on the security-critical or track-mutation path |
| `scripts/verify_architecture_docs.py` | doc/index synchronization is a repository hygiene check, not a workflow-critical runtime dependency |
| `scripts/pr_review.py` | PR review orchestration is useful automation, but local `/track:review` and `sotp pr` already cover required merge safety without this Python loop |

## Adapter Boundary Decision

- `git` subprocess execution, repo-root resolution, and track branch claim discovery belong to `libs/infrastructure/src/git_cli.rs`.
- `gh` subprocess execution and PR transport decoding belong to `libs/infrastructure/src/gh_cli.rs`.
- CLI modules should only parse arguments, map infrastructure records into usecase inputs, and translate user-visible output / exit codes.
- Remaining Python verification scripts should be treated as repository-policy checks until their data sources are modeled in Rust.
