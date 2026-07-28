//! Regression tests for catalogue-lint files shipped in
//! `.harness/catalogue-lint/config.json` and
//! `.harness/catalogue-lint/presets/ddd-strict.json`.
//!
//! These tests load both files through the real [`FsLintConfigLoader`]
//! production adapter — the same adapter `apps/cli-composition` wires at
//! runtime — so a successful `load()` call is direct evidence the shipped
//! files parse into a valid `usecase::catalogue_lint_workflow::LintConfig`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};

use infrastructure::tddd::fs_lint_config_loader::FsLintConfigLoader;
use usecase::catalogue_lint_workflow::LintConfigLoader;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn config_path() -> PathBuf {
    repo_root().join(".harness/catalogue-lint/config.json")
}

fn preset_path() -> PathBuf {
    repo_root().join(".harness/catalogue-lint/presets/ddd-strict.json")
}

// ---------------------------------------------------------------------------
// Both files parse successfully into LintConfig via the real
// FsLintConfigLoader production adapter
// ---------------------------------------------------------------------------

#[test]
fn test_config_json_loads_successfully_via_fs_lint_config_loader() {
    let loader = FsLintConfigLoader::new(config_path());
    let config = loader.load().expect("config.json must load as a valid LintConfig");
    assert!(!config.rules().is_empty(), "config.json must declare at least one rule");
}

#[test]
fn test_ddd_strict_preset_loads_successfully_via_fs_lint_config_loader() {
    let loader = FsLintConfigLoader::new(preset_path());
    let config = loader.load().expect("presets/ddd-strict.json must load as a valid LintConfig");
    assert!(!config.rules().is_empty(), "presets/ddd-strict.json must declare at least one rule");
}
