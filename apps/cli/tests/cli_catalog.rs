//! Integration tests for `sotp catalog {init,add,import,cite,check}`.
//!
//! Covers: parsing / dispatch, `add` skeleton + `$todo` output, validated-input
//! rejections (InvalidRole / AnchorNotFound / ParseFragment / DuplicateEntry /
//! FileMissing), `check` exit-code mapping, and the WRITE track-id
//! fail-closed guard (D8 / AC-03 / AC-06 / AC-09 / AC-11 / AC-13).
//!
//! WRITE verbs (init/add/import/cite) resolve the track via the git branch, so
//! those tests build a real `track/<id>` git fixture. The READ `check` verb
//! accepts an explicit `--track-id`, so its tests need no git repository.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::process::{Command, Output};

/// Minimal `architecture-rules.json` with a single `tddd.enabled` layer.
const RULES_JSON: &str = r#"{
  "version": 2,
  "layers": [
    {
      "crate": "domain",
      "path": "libs/domain",
      "may_depend_on": [],
      "deny_reason": "",
      "tddd": {
        "enabled": true,
        "catalogue_file": "domain-types.json",
        "schema_export": {"method": "rustdoc", "targets": ["domain"]}
      }
    }
  ]
}"#;

/// Two enabled TDDD layers for partial-catalogue check coverage.
const RULES_TWO_LAYERS_JSON: &str = r#"{
  "version": 2,
  "layers": [
    {
      "crate": "domain",
      "path": "libs/domain",
      "may_depend_on": [],
      "deny_reason": "",
      "tddd": {
        "enabled": true,
        "catalogue_file": "domain-types.json",
        "schema_export": {"method": "rustdoc", "targets": ["domain"]}
      }
    },
    {
      "crate": "usecase",
      "path": "libs/usecase",
      "may_depend_on": ["domain"],
      "deny_reason": "",
      "tddd": {
        "enabled": true,
        "catalogue_file": "usecase-types.json",
        "schema_export": {"method": "rustdoc", "targets": ["usecase"]}
      }
    }
  ]
}"#;

/// Minimal valid `spec.json` (schema_version 2, no requirements).
const SPEC_JSON: &str = r#"{
  "schema_version": 2,
  "version": "1.0.0",
  "title": "Test spec",
  "scope": { "in_scope": [], "out_of_scope": [] }
}"#;

/// A clean, empty v5 catalogue (passes every gate).
const EMPTY_CATALOGUE: &str = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {}
}"#;

/// A v5 catalogue carrying a residual `$todo` hole.
const HOLES_CATALOGUE: &str = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": { "Foo": { "role": { "$todo": "pick a role" } } },
  "traits": {},
  "functions": {}
}"#;

fn sotp_bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sotp"));
    // Never let a spawned run write to the real track/items/ tree.
    cmd.env("SOTP_TELEMETRY", "0");
    cmd
}

fn write(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn show(out: &Output) -> String {
    format!(
        "status={:?}\nstdout={}\nstderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// Run `sotp <args>` anchored at `root`.
fn sotp(root: &Path, args: &[&str]) -> Output {
    sotp_bin().current_dir(root).args(args).output().unwrap()
}

/// Run a git command in `dir`, asserting success.
fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

/// Build a git repository on branch `track/<track_id>` with a minimal
/// architecture-rules.json and the track's spec.json committed.
fn setup_git_track(track_id: &str) -> tempfile::TempDir {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    git(root, &["init", "--quiet", &format!("--initial-branch=track/{track_id}")]);
    git(root, &["config", "user.email", "test@example.com"]);
    git(root, &["config", "user.name", "test"]);
    write(&root.join("architecture-rules.json"), RULES_JSON);
    write(&root.join("track/items").join(track_id).join("spec.json"), SPEC_JSON);
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "init"]);
    ws
}

/// Items-dir argument (absolute) for a workspace root.
fn items_arg(root: &Path) -> String {
    root.join("track").join("items").to_str().unwrap().to_owned()
}

// ---------------------------------------------------------------------------
// Parsing / dispatch
// ---------------------------------------------------------------------------

#[test]
fn catalog_help_lists_all_verbs() {
    let out = sotp_bin().args(["catalog", "--help"]).output().unwrap();
    assert!(out.status.success(), "catalog --help: {}", show(&out));
    let stdout = String::from_utf8_lossy(&out.stdout);
    for verb in ["init", "add", "import", "cite", "check"] {
        assert!(stdout.contains(verb), "help must list `{verb}`: {stdout}");
    }
}

// ---------------------------------------------------------------------------
// WRITE lifecycle + validated-input rejections (git-backed)
// ---------------------------------------------------------------------------

#[test]
fn catalog_add_produces_todo_and_rejects_bad_input() {
    let ws = setup_git_track("test-track");
    let root = ws.path();
    let items = items_arg(root);

    // init: generate the skeleton for the domain layer.
    let out = sotp(root, &["catalog", "init", "--items-dir", &items]);
    assert!(out.status.success(), "init: {}", show(&out));

    // add a valid struct → exit 0, and the report lists the residual $todo holes.
    let out = sotp(
        root,
        &[
            "catalog",
            "add",
            "--items-dir",
            &items,
            "--layer",
            "domain",
            "--kind",
            "struct",
            "--name",
            "Foo",
            "--role",
            "ValueObject",
            "--field",
            "count: u32",
        ],
    );
    assert!(out.status.success(), "add Foo: {}", show(&out));
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("$todo"),
        "add must report $todo holes: {}",
        show(&out)
    );

    // duplicate entry → non-zero (no force path).
    let out = sotp(
        root,
        &[
            "catalog",
            "add",
            "--items-dir",
            &items,
            "--layer",
            "domain",
            "--kind",
            "struct",
            "--name",
            "Foo",
            "--role",
            "ValueObject",
        ],
    );
    assert_eq!(out.status.code(), Some(1), "duplicate entry: {}", show(&out));

    // invalid role → non-zero (fail-closed vocabulary check).
    let out = sotp(
        root,
        &[
            "catalog",
            "add",
            "--items-dir",
            &items,
            "--layer",
            "domain",
            "--kind",
            "struct",
            "--name",
            "Bar",
            "--role",
            "Bogus",
        ],
    );
    assert_eq!(out.status.code(), Some(1), "invalid role: {}", show(&out));

    // dangling spec anchor → non-zero (spec.json declares no anchors).
    let out = sotp(
        root,
        &[
            "catalog",
            "add",
            "--items-dir",
            &items,
            "--layer",
            "domain",
            "--kind",
            "struct",
            "--name",
            "Baz",
            "--role",
            "ValueObject",
            "--anchor",
            "ZZ-99",
        ],
    );
    assert_eq!(out.status.code(), Some(1), "anchor not found: {}", show(&out));

    // unparseable fragment → non-zero.
    let out = sotp(
        root,
        &[
            "catalog",
            "add",
            "--items-dir",
            &items,
            "--layer",
            "domain",
            "--kind",
            "struct",
            "--name",
            "Qux",
            "--role",
            "ValueObject",
            "--field",
            "this is not a field",
        ],
    );
    assert_eq!(out.status.code(), Some(1), "parse fragment: {}", show(&out));
}

#[test]
fn catalog_add_without_init_guides_to_init() {
    let ws = setup_git_track("test-track");
    let root = ws.path();
    let items = items_arg(root);

    let out = sotp(
        root,
        &[
            "catalog",
            "add",
            "--items-dir",
            &items,
            "--layer",
            "domain",
            "--kind",
            "struct",
            "--name",
            "Foo",
            "--role",
            "ValueObject",
        ],
    );
    assert_eq!(out.status.code(), Some(1), "missing file: {}", show(&out));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("init"),
        "missing-file error should guide to `init`: {}",
        show(&out)
    );
}

#[test]
fn catalog_write_track_id_mismatch_fails_closed() {
    let ws = setup_git_track("test-track");
    let root = ws.path();
    let items = items_arg(root);

    // Explicit --track-id that does not match the branch-derived id is rejected.
    let out = sotp(root, &["catalog", "init", "--items-dir", &items, "--track-id", "other-track"]);
    assert_eq!(out.status.code(), Some(1), "write track-id mismatch: {}", show(&out));
}

// ---------------------------------------------------------------------------
// READ check exit-code mapping (no git; explicit --track-id override)
// ---------------------------------------------------------------------------

#[test]
fn catalog_check_exit_code_mapping() {
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(&root.join("architecture-rules.json"), RULES_JSON);
    let items = items_arg(root);
    write(&root.join("track/items/clean-track/domain-types.json"), EMPTY_CATALOGUE);
    write(&root.join("track/items/holes-track/domain-types.json"), HOLES_CATALOGUE);

    // Pass: clean catalogue → 0.
    let out = sotp(root, &["catalog", "check", "--track-id", "clean-track", "--items-dir", &items]);
    assert!(out.status.success(), "clean → Pass: {}", show(&out));

    // Blocked: residual holes after catalogue generation starts → non-zero.
    let out = sotp(root, &["catalog", "check", "--track-id", "holes-track", "--items-dir", &items]);
    assert_eq!(out.status.code(), Some(1), "holes → Blocked: {}", show(&out));

    // Skipped: no target catalogue files exist yet → 0.
    let out =
        sotp(root, &["catalog", "check", "--track-id", "missing-track", "--items-dir", &items]);
    assert!(out.status.success(), "missing → Skipped: {}", show(&out));

    // Blocked: once any layer catalogue exists, another expected layer being
    // absent is a partial catalogue and must block.
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path();
    write(&root.join("architecture-rules.json"), RULES_TWO_LAYERS_JSON);
    let items = items_arg(root);
    write(&root.join("track/items/partial-track/domain-types.json"), EMPTY_CATALOGUE);
    let out =
        sotp(root, &["catalog", "check", "--track-id", "partial-track", "--items-dir", &items]);
    assert_eq!(out.status.code(), Some(1), "partial missing layer → Blocked: {}", show(&out));
}
