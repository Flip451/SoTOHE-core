# Verification: Per-Worker Build Directory Isolation

## Scope Verified

- [x] compose.yml WORKER_ID-based CARGO_TARGET_DIR via `CARGO_TARGET_DIR_RELATIVE` env var
- [x] Makefile.toml worker-aware -exec tasks with WORKER_ID passthrough
- [x] Documentation in `.claude/rules/07-dev-environment.md`
- [x] sccache sharing validation (SCCACHE_DIR unchanged across workers)

## Manual Verification Steps

- [ ] Parallel `WORKER_ID=w1` and `WORKER_ID=w2` builds complete without deadlock
  - Requires `cargo make tools-up` and two parallel exec sessions
- [x] Default (no WORKER_ID) uses /workspace/target — confirmed via `cargo make ci`
- [x] sccache config unchanged: `SCCACHE_DIR: /workspace/.cache/sccache` is independent of CARGO_TARGET_DIR
- [x] `cargo make ci` passes — all 218 Rust tests + 448 Python tests pass

## Result / Open Issues

- `CARGO_TARGET_DIR_RELATIVE` env var used in compose.yml for host-side override support
- `-exec` tasks use shell script with conditional `-e CARGO_TARGET_DIR` override
- Parallel deadlock test requires running tools-daemon with multiple exec sessions (manual verification deferred)
- `target-*/` added to `.gitignore`

## verified_at

2026-03-12
