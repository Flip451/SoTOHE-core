# Verification: Shell Command Guard CLI

## Scope Verified

- [x] domain::guard module (verdict.rs, parser.rs, policy.rs)
- [x] parser.rs rewritten to use conch-parser AST (vendored, patched)
- [x] CLI guard check subcommand (binary name: sotp)
- [x] Python hook migration (CLI delegation with fallback)
- [x] All acceptance criteria from spec.md

## Manual Verification Steps

- [x] `sotp guard check --command "git add ."` returns block verdict
- [x] `sotp guard check --command "git status"` returns allow verdict
- [x] `sotp guard check --command "env git commit -m msg"` returns block
- [x] `sotp guard check --command "bash -c 'git push'"` returns block
- [x] `sotp guard check --command '$VAR add'` returns block
- [x] `sotp guard check --command "cargo test"` returns allow
- [x] Python hook delegates to CLI and blocks correctly (fallback tested)
- [x] `cargo make ci-rust` passes

## Result / Open Issues

- 100 Rust tests pass (including guard module tests)
- Parser rewritten from hand-written state machine (585 lines) to conch-parser adapter
- conch-parser vendored at vendor/conch-parser with future-incompat patch (trailing semicolons)
- Binary name changed from `cli` to `sotp` via [[bin]] in apps/cli/Cargo.toml
- Policy handles env -S, attached args, launcher-wrapped env -S, launchers in find -exec/xargs
- Review round 1: 3 findings (python versioned binary, absolute path git in python, env -S) — fixed
- Review round 2: 3 findings (binary name mismatch, env -S edge cases, launcher in find/xargs) — fixed
- Python hook retains full fallback logic when CLI binary unavailable

## verified_at

2026-03-11
