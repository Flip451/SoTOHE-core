<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Clippy Zero Warnings: centralized lint policy with workspace.lints and clippy.toml

Centralize all lint configuration: eliminate duplicated #![deny(...)] blocks from 4 crate roots.
vendor/conch-parser (Rust 2015 edition) emits 198 warnings — suppress with #![allow(warnings)].
Introduce [workspace.lints] in root Cargo.toml for consistent clippy/rust deny policy.
Introduce clippy.toml for threshold tuning (msrv, cognitive-complexity, etc.).
Per-module #![allow(...)] exceptions (test files, guard/*.rs) remain as local overrides.
Maintain existing -D warnings CI enforcement in Makefile.toml as a secondary gate.

## Vendored Crate Warning Suppression

Add #![allow(warnings)] to vendor/conch-parser/src/lib.rs.
This suppresses 198 warnings from deprecated Rust 2015 patterns (bare_trait_objects, deprecated methods, mismatched_lifetime_syntaxes).
Vendored code修正は保守コスト増のため抑制が妥当。

- [x] Add #![allow(warnings)] to vendor/conch-parser/src/lib.rs to suppress vendored crate warnings

## Workspace Lint Policy Centralization

Add [workspace.lints.clippy] to root Cargo.toml with deny-level lints: indexing_slicing, unwrap_used, expect_used, panic, unreachable, todo, unimplemented.
Add [workspace.lints.rust] with warnings = 'deny' for zero-warning enforcement.
Each workspace member inherits via [lints] workspace = true.
Remove duplicated #![deny(...)] blocks from all 4 crate root files.
Per-module #![allow(...)] in test files and guard/*.rs remain as targeted local exceptions.

- [x] Add [workspace.lints] section to root Cargo.toml — move duplicated #![deny(clippy::*)] from 4 crate roots into workspace-level lint config
- [x] Add [lints] workspace = true to each workspace member Cargo.toml (domain, usecase, infrastructure, cli)
- [x] Remove duplicated #![deny(...)] blocks from apps/cli/src/main.rs, libs/domain/src/lib.rs, libs/usecase/src/lib.rs, libs/infrastructure/src/lib.rs

## clippy.toml Configuration

Create clippy.toml at workspace root.
Set msrv = '1.85' to align with workspace rust-version.
Configure threshold defaults explicitly: cognitive-complexity-threshold, too-many-arguments-threshold, type-complexity-threshold.
This makes lint thresholds visible and versionable alongside the codebase.

- [x] Create clippy.toml at workspace root with msrv and threshold settings (cognitive-complexity, too-many-arguments, type-complexity)

## Verification

Run cargo make clippy and confirm zero warnings.
Run cargo make ci to verify full CI pipeline passes.
Verify no #![deny(clippy::...)] remains in crate root files.

- [x] Verify cargo make clippy produces zero warnings and cargo make ci passes
