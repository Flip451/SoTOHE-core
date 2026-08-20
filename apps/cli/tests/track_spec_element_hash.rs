//! Process-level CLI contract coverage for `track spec-element-hash`.

#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const VALID_SPEC_JSON: &str = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Test Spec",
  "goal": [
    {"id": "GL-01", "text": "First goal"}
  ],
  "scope": {
    "in_scope": [],
    "out_of_scope": []
  }
}"#;

fn list_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).expect("fixture directory is readable") {
            let path = entry.expect("fixture entry is readable").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.push(path.strip_prefix(root).expect("path is relative").to_path_buf());
            }
        }
    }
    files.sort();
    files
}

#[test]
fn test_track_spec_element_hash_call_site_preserves_cli_contract_across_migration() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let root = workspace.path();
    let items_dir = root.join("track/items");
    let track_dir = items_dir.join("hash-track");
    fs::create_dir_all(&track_dir).expect("track directory exists");
    fs::write(track_dir.join("spec.json"), VALID_SPEC_JSON).expect("spec document is written");

    let before = list_files(root);
    let items_dir_arg = items_dir.to_string_lossy().into_owned();
    let argv = vec![
        "track".to_owned(),
        "spec-element-hash".to_owned(),
        "--items-dir".to_owned(),
        items_dir_arg,
        "--track-id".to_owned(),
        "hash-track".to_owned(),
        "--anchor".to_owned(),
        "GL-01".to_owned(),
    ];
    let output = Command::new(env!("CARGO_BIN_EXE_sotp"))
        .current_dir(root)
        .args(&argv)
        .output()
        .expect("sotp process starts");

    assert!(output.status.success(), "stderr={:?}", output.stderr);
    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    let hash = stdout.trim();
    assert_eq!(hash.len(), 64, "single-anchor output must be one SHA-256 hash");
    assert!(hash.chars().all(|character| character.is_ascii_hexdigit()));
    assert!(output.stderr.is_empty(), "successful lookup must not write stderr");
    assert_eq!(list_files(root), before, "spec-element-hash must not persist extra files");
}
