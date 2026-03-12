# Verification: Security Control Tests

## Scope Verified

- [x] Container .git read-only test in CI
- [x] Container sensitive directory test in CI
- [x] Hook fail-closed behavior test
- [x] Concurrent lock test (already covered by existing Rust integration tests in `libs/infrastructure/tests/concurrency.rs`)
- [x] cargo make ci integration
- [x] atomic_write.py `_find_sotp()` binary selection Python unit tests (Open Issue from atomic-write-standard track)

## Manual Verification Steps

- [x] All new tests pass in `cargo make ci` — **PASS** (218 Rust tests + all Python selftests)
- [x] CI pipeline includes new test targets — **PASS** (`test_atomic_write.py` added to `scripts-selftest-local`)
- [x] Tests verify security controls via source inspection or compose config assertions

## Result

- **PASS** — 全 acceptance criteria 達成

### Implemented Tests

1. **T1: Container .git read-only** — `test_compose_mounts_git_readonly` in `scripts/test_verify_scripts.py`: verifies `.git:ro` mount in both `compose.yml` and `compose.dev.yml`
2. **T2: Sensitive directory tmpfs** — `test_compose_masks_sensitive_dirs_with_tmpfs` in `scripts/test_verify_scripts.py`: verifies `tmpfs` overlays for `private/` and `config/secrets/`
3. **T3: Hook fail-closed behavior** — 3 tests in `.claude/hooks/test_policy_hooks.py`:
   - `test_main_exits_2_on_invalid_json`: JSON parse failure → `os._exit(2)`
   - `test_main_function_has_fail_closed_os_exit`: `os._exit(2)` + `except BaseException` present
   - `test_main_function_exits_0_for_non_bash_tool`: non-Bash tool → `os._exit(0)`
4. **T4: Concurrent lock test** — Already covered by `parallel_updates_are_serialized_by_lock_manager` and `parallel_updates_then_complete_all_results_in_done` in `libs/infrastructure/tests/concurrency.rs`
5. **T5: CI integration** — All new tests wired into `cargo make ci` pipeline; `test_atomic_write.py` added to `scripts-selftest-local`
6. **Bonus: atomic_write.py tests** — `scripts/test_atomic_write.py` covers `_find_sotp()` probe/cache, `atomic_write_file()` fallback, and `_probe_supports_file_write_atomic()` error handling

## Open Issues

None.

## verified_at

- 2026-03-12
