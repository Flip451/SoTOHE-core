# Harness Engineering Best Practices 2026 — Gap Analysis

Source: https://nyosegawa.github.io/posts/harness-engineering-best-practices-2026/
Date: 2026-03-09

## Summary of Key Practices

- **The Harness Metaphor**: The "Harness" (constraints, feedback, context) is the critical system that makes the "Horse" (the AI agent) reliable and productive.
- **High-Speed Feedback Loops**: Sub-second feedback using Rust-based tools (Oxlint, Ruff, Biome) in `PostToolUse` hooks is essential to maintain agent momentum.
- **Documentation as Pointers**: Maintain a concise `CLAUDE.md` (<50 lines) that points to deeper documentation (ADRs, specs) rather than containing exhaustive detail.
- **Architectural Constraints**: Use machine-readable rules and CI to prevent agents from violating architectural boundaries.
- **Separation of Strategy & Execution**: Explicitly decouple the Planning Phase (strategy/design) from the Execution Phase (coding).
- **Git as a Session Bridge**: Use Git status and commits as the primary mechanism to maintain state across different agent sessions.
- **Universal E2E Testing**: Employ the Accessibility Tree as a structured, agent-friendly interface for verifying UI changes.
- **Harness as the Moat**: A team's competitive advantage lies in the quality of its engineering environment, not just the underlying LLM.

## Gap Analysis

| Practice | Status | template-003 Implementation |
|---|:---:|---|
| Architectural Constraints | ✅ Covered | `docs/architecture-rules.json`, `scripts/check_layers.py`, `deny.toml` |
| Separation of Planning/Execution | ✅ Covered | Track workflow (`track/items/`), `check-codex-after-plan.py` |
| Git as a Session Bridge | ✅ Covered | `block-direct-git-ops.py`, track registry |
| Entropy Management | ✅ Covered | `cargo-deny`, `clippy -D warnings`, layer enforcement |
| Context Engineering | ✅ Covered | `.claude/rules/`, `agent-profiles.json`, `track/` |
| Documentation as Pointers | ⚠️ Partial | `CLAUDE.md` is ~180 lines (article recommends <50) |
| High-Speed Feedback Loops | ⚠️ Partial | `tools-daemon` helps but clippy >1s; no Ruff for Python |
| Universal E2E Testing | ❌ N/A | Backend-only template; relevant if frontend is added |

## Actionable Recommendations

| Priority | Practice | Recommendation |
|---|---|---|
| High | High-Speed Feedback | Adopt Ruff for Python linting in `PostToolUse` hooks (sub-100ms) |
| Medium | Doc Pointers | Shrink `CLAUDE.md` to <50 lines; move details to `.claude/docs/` |
| Medium | High-Speed Feedback | Investigate `cargo check` for fastest PostToolUse feedback, reserve `clippy` for CI |
| Low | Universal E2E | Add Accessibility Tree testing if a frontend layer is introduced |
