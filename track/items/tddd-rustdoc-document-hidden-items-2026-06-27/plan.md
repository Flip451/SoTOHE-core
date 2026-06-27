<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# TDDD chain ③ の cargo rustdoc 呼び出しに --document-hidden-items を追加する

## Summary

Single-task implementation: add `--document-hidden-items` to the `cargo rustdoc` invocation inside `run_rustdoc` in `libs/infrastructure/src/schema_export/bin_target.rs` and add a unit test asserting the flag is present in the constructed args vector. The flag addition is one line; extracting the inline closure into a testable private function is a small structural refactor; the test is 5-10 lines. All spec elements are satisfied by T001.

## Tasks (0/1 resolved)

### S1 — Flag addition and unit test

> Add `"--document-hidden-items"` to the `v.extend(...)` call inside `run_rustdoc`'s args construction in `libs/infrastructure/src/schema_export/bin_target.rs`.
> The existing extend appends `["--", "-Z", "unstable-options", "--output-format", "json"]`; `"--document-hidden-items"` is appended after `"json"` in the same array.
> Since both `--lib` and `--bin` paths share the same args construction, a single addition to that construction covers both baseline and actual capture paths (IN-01 / CN-01).
> Extract the inline closure body into a private `build_rustdoc_args` function to make the flag-list testable without a real cargo invocation, then add a unit test that asserts `--document-hidden-items` is present (AC-01).
> No caller changes outside `bin_target.rs` are permitted (CN-01). No new toolchain dependency is introduced (CN-02).
> After T001, `cargo make ci` passes and `bin/sotp signal calc-impl-catalog` no longer emits `DanglingId` Yellow/Red for `pub #[doc(hidden)]` elements (AC-02, AC-03).

- [~] **T001**: In `libs/infrastructure/src/schema_export/bin_target.rs`, add `"--document-hidden-items"` to the flag slice appended by the `args` closure in `run_rustdoc` (current extend call: `["--", "-Z", "unstable-options", "--output-format", "json"]`; add `"--document-hidden-items"` to that array). Extract the inline `args` closure body into a private `build_rustdoc_args(crate_name: &str, target: &[&str]) -> Vec<String>` function so the flag-list construction is unit-testable without a real cargo invocation. Replace the two `.args(args(&[...]))` call sites in `run_rustdoc` with `build_rustdoc_args(crate_name, &[...])`. Add a unit test in the existing `#[cfg(test)]` module that calls `build_rustdoc_args("my_crate", &["--lib"])` and asserts `"--document-hidden-items"` is contained in the returned `Vec<String>`. No caller changes outside of `bin_target.rs` are permitted (CN-01). No new toolchain dependency is introduced: the flag is already covered by the existing `-Z unstable-options` nightly requirement (CN-02).
