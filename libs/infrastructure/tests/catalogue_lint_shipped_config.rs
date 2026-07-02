//! Regression tests for the two ADR D7 default `ForbidPrimitiveInTypes`
//! catalogue-lint rules shipped in `.harness/catalogue-lint/config.json` and
//! `.harness/catalogue-lint/presets/ddd-strict.json`
//! (track `catalogue-primitive-obsession-guard-2026-07-01`, T007; ADR
//! `knowledge/adr/2026-07-01-0004-catalogue-primitive-obsession-guard.md` §D7).
//!
//! In *this* repository the two files are kept byte-identical. They are only
//! expected to diverge downstream, after a template consumer copies
//! `presets/ddd-strict.json` and edits it, per the copy-and-edit workflow
//! described in ADR `knowledge/adr/2026-05-25-0000-tddd-pattern-semantics-extension.md`.
//!
//! These tests load both files through the real [`FsLintConfigLoader`]
//! production adapter — the same adapter `apps/cli-composition` wires at
//! runtime — so a successful `load()` call is direct evidence the shipped
//! files parse into a valid `usecase::catalogue_lint_workflow::LintConfig`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};

use infrastructure::tddd::fs_lint_config_loader::FsLintConfigLoader;
use usecase::catalogue_lint_workflow::{LintConfigLoader, LintRuleKind, LintRuleSpec};

/// Every `RoleKind` variant name except `Dto` and `Command` (ADR D7 rule 2's
/// role-exclusion set), listed in the declaration order of
/// `domain::tddd::catalogue_linter::RoleKind::ALL`
/// (`libs/domain/src/tddd/catalogue_linter.rs`).
///
/// `RoleKind::ALL` is `pub(crate)` to the `domain` crate, and `domain` has an
/// empty `may_depend_on` in `architecture-rules.json`, so neither this test
/// (an external crate) nor `domain` itself (layering) can re-derive this list
/// live against `usecase::catalogue_lint_workflow::LintRuleSpec`. This literal
/// was produced by reading `RoleKind::ALL` directly and removing `Dto` and
/// `Command`; keep it in sync by hand if `RoleKind` ever gains or loses a
/// variant.
fn expected_roles_excluding_dto_and_command() -> Vec<String> {
    [
        "ValueObject",
        "Entity",
        "AggregateRoot",
        "DomainService",
        "Specification",
        "Factory",
        "UseCase",
        "Interactor",
        "Query",
        "ErrorType",
        "SecondaryAdapter",
        "EventPolicy",
        "DomainEvent",
        "CompositionRoot",
        "PrimaryAdapter",
        "SpecificationPort",
        "ApplicationService",
        "SecondaryPort",
        "Repository",
        "FreeFunction",
        "UseCaseFunction",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

/// TDDD-enabled layer names, enumerated from the real `architecture-rules.json`
/// at test run time (not hard-coded) — every layer whose `tddd.enabled` is
/// `true`, in file declaration order.
fn expected_layers() -> Vec<String> {
    let path = repo_root().join("architecture-rules.json");
    let content =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let value: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
    value["layers"]
        .as_array()
        .unwrap_or_else(|| panic!("{} must have a 'layers' array", path.display()))
        .iter()
        .filter(|layer| layer["tddd"]["enabled"].as_bool().unwrap_or(false))
        .map(|layer| {
            layer["crate"]
                .as_str()
                .unwrap_or_else(|| panic!("layer entry missing 'crate' string: {layer}"))
                .to_owned()
        })
        .collect()
}

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
// Requirement 1: config.json and presets/ddd-strict.json remain byte-identical
// ---------------------------------------------------------------------------

#[test]
fn test_config_and_ddd_strict_preset_are_byte_identical() {
    let config_bytes = std::fs::read(config_path()).expect("config.json must exist");
    let preset_bytes = std::fs::read(preset_path()).expect("presets/ddd-strict.json must exist");
    assert_eq!(
        config_bytes, preset_bytes,
        ".harness/catalogue-lint/config.json and .harness/catalogue-lint/presets/ddd-strict.json \
         must remain byte-identical in this repository"
    );
}

// ---------------------------------------------------------------------------
// Requirement 2: both files parse successfully into LintConfig via the real
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

// ---------------------------------------------------------------------------
// Requirement 3: the two ADR D7 default ForbidPrimitiveInTypes rules are
// present with the expected target_roles / primitives / layers / positions.
// ---------------------------------------------------------------------------

#[test]
fn test_config_json_has_two_forbid_primitive_in_types_rules_with_expected_content() {
    let loader = FsLintConfigLoader::new(config_path());
    let config = loader.load().unwrap();
    assert_adr_d7_default_rules(config.rules());
}

#[test]
fn test_ddd_strict_preset_has_two_forbid_primitive_in_types_rules_with_expected_content() {
    let loader = FsLintConfigLoader::new(preset_path());
    let config = loader.load().unwrap();
    assert_adr_d7_default_rules(config.rules());
}

/// Shared assertion body: verifies the exact two ADR D7 default
/// `ForbidPrimitiveInTypes` rules (T007) are present, appended as the last
/// two rule entries, with the expected content on every axis.
fn assert_adr_d7_default_rules(rules: &[LintRuleSpec]) {
    let primitive_rule_count = rules
        .iter()
        .filter(|r| matches!(r.kind, LintRuleKind::ForbidPrimitiveInTypes { .. }))
        .count();
    assert_eq!(
        primitive_rule_count, 2,
        "expected exactly 2 ForbidPrimitiveInTypes rules (ADR D7 default), found \
         {primitive_rule_count}"
    );
    assert!(rules.len() >= 2, "rules list must have at least 2 entries");

    let last_two = &rules[rules.len() - 2..];
    let rule1 = &last_two[0];
    let rule2 = &last_two[1];
    let layers = expected_layers();

    // --- Rule 1: result_err x String, all roles (empty target_roles ==
    // "all roles" per RuleTarget::matches / LintRuleSpec's own doc comment,
    // T006). ---
    assert!(
        rule1.target_roles.is_empty(),
        "rule 1 (result_err) must target all roles via an empty target_roles vec, got {:?}",
        rule1.target_roles
    );
    match rule1.kind.clone() {
        LintRuleKind::ForbidPrimitiveInTypes { primitives, layers: rule_layers, positions } => {
            assert_eq!(primitives, vec!["String".to_owned()]);
            assert_eq!(rule_layers, layers);
            assert_eq!(positions, vec!["result_err".to_owned()]);
        }
        other => panic!("rule 1: expected ForbidPrimitiveInTypes, got {other:?}"),
    }

    // --- Rule 2: named_field + variant_field x String, all roles except
    // Dto (all layers, D6/CN-06) and Command (usecase, D6/CN-06). ---
    assert_eq!(
        rule2.target_roles,
        expected_roles_excluding_dto_and_command(),
        "rule 2 (named_field/variant_field) target_roles must be every RoleKind except \
         Dto and Command"
    );
    assert!(!rule2.target_roles.iter().any(|r| r == "Dto"));
    assert!(!rule2.target_roles.iter().any(|r| r == "Command"));
    match rule2.kind.clone() {
        LintRuleKind::ForbidPrimitiveInTypes { primitives, layers: rule_layers, positions } => {
            assert_eq!(primitives, vec!["String".to_owned()]);
            assert_eq!(rule_layers, layers);
            assert_eq!(positions, vec!["named_field".to_owned(), "variant_field".to_owned()]);
        }
        other => panic!("rule 2: expected ForbidPrimitiveInTypes, got {other:?}"),
    }
}
