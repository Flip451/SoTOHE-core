//! Tests for review_v2 persistence adapters.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use domain::review_v2::{
    FastVerdict, MainScopeName, ReviewExistsPort, ReviewHash, ReviewReader, ReviewWriter,
    ReviewerFinding, ScopeName, Verdict,
};

use super::review_store::FsReviewStore;

fn make_store(dir: &std::path::Path) -> FsReviewStore {
    FsReviewStore::new(dir.join("review.json"), dir.to_path_buf())
}

fn sample_scope() -> ScopeName {
    ScopeName::Main(MainScopeName::new("domain").unwrap())
}

fn sample_hash() -> ReviewHash {
    ReviewHash::computed("rvw1:sha256:abcdef0123456789").unwrap()
}

fn sample_finding() -> ReviewerFinding {
    ReviewerFinding::new(
        "test finding",
        Some("P2".to_owned()),
        Some("lib.rs".to_owned()),
        Some(42),
        Some("style".to_owned()),
    )
    .unwrap()
}

// ── init / read basics ──────────────────────────────────────────

#[test]
fn test_read_missing_file_returns_empty_map() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path());
    let result = store.read_latest_finals().unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_init_creates_v2_empty_doc() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path());
    store.init().unwrap();

    let content = std::fs::read_to_string(dir.path().join("review.json")).unwrap();
    let value: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(value["schema_version"], 2);
    assert!(value["scopes"].as_object().unwrap().is_empty());
}

#[test]
fn test_read_after_init_returns_empty_map() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path());
    store.init().unwrap();
    let result = store.read_latest_finals().unwrap();
    assert!(result.is_empty());
}

// ── write_verdict round trips ───────────────────────────────────

#[test]
fn test_write_zero_findings_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path());
    let scope = sample_scope();
    let hash = sample_hash();

    store.write_verdict(&scope, &Verdict::ZeroFindings, &hash).unwrap();
    let map = store.read_latest_finals().unwrap();

    assert_eq!(map.len(), 1);
    let (verdict, read_hash) = map.get(&scope).unwrap();
    assert!(matches!(verdict, Verdict::ZeroFindings));
    assert_eq!(read_hash, &hash);
}

#[test]
fn test_write_findings_remain_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path());
    let scope = sample_scope();
    let hash = sample_hash();
    let finding = sample_finding();
    let verdict = Verdict::findings_remain(vec![finding.clone()]).unwrap();

    store.write_verdict(&scope, &verdict, &hash).unwrap();
    let map = store.read_latest_finals().unwrap();

    let (read_verdict, _) = map.get(&scope).unwrap();
    match read_verdict {
        Verdict::FindingsRemain(nef) => {
            assert_eq!(nef.as_slice().len(), 1);
            assert_eq!(nef.as_slice()[0].message(), "test finding");
            assert_eq!(nef.as_slice()[0].severity(), Some("P2"));
            assert_eq!(nef.as_slice()[0].file(), Some("lib.rs"));
            assert_eq!(nef.as_slice()[0].line(), Some(42));
            assert_eq!(nef.as_slice()[0].category(), Some("style"));
        }
        _ => panic!("expected FindingsRemain"),
    }
}

#[test]
fn test_write_fast_verdict_not_in_latest_finals() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path());
    let scope = sample_scope();
    let hash = sample_hash();

    store.write_fast_verdict(&scope, &FastVerdict::ZeroFindings, &hash).unwrap();
    let map = store.read_latest_finals().unwrap();
    // fast rounds are not "final" rounds, so should not appear
    assert!(map.is_empty());
}

#[test]
fn test_multiple_rounds_returns_latest_final() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path());
    let scope = sample_scope();
    let hash = sample_hash();

    // First final round: findings_remain
    let finding = sample_finding();
    let v1 = Verdict::findings_remain(vec![finding]).unwrap();
    store.write_verdict(&scope, &v1, &hash).unwrap();

    // Second final round: zero_findings
    store.write_verdict(&scope, &Verdict::ZeroFindings, &hash).unwrap();

    let map = store.read_latest_finals().unwrap();
    let (verdict, _) = map.get(&scope).unwrap();
    assert!(matches!(verdict, Verdict::ZeroFindings));
}

// ── reset ────────────────────────────────────────────────────────

#[test]
fn test_reset_archives_and_creates_fresh() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path());
    let scope = sample_scope();
    let hash = sample_hash();

    // Write something
    store.write_verdict(&scope, &Verdict::ZeroFindings, &hash).unwrap();
    assert!(dir.path().join("review.json").exists());

    // Reset
    store.reset().unwrap();

    // Archive file should exist (review-<timestamp>-<pid>.json)
    let archive_files: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name().to_str().is_some_and(|n| n.starts_with("review-") && n.ends_with(".json"))
        })
        .collect();
    assert_eq!(archive_files.len(), 1, "expected exactly one archive file");

    // New review.json should be fresh (empty scopes)
    let map = store.read_latest_finals().unwrap();
    assert!(map.is_empty());
}

// ── empty hash round trip ───────────────────────────────────────

#[test]
fn test_write_empty_hash_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path());
    let scope = sample_scope();

    store.write_verdict(&scope, &Verdict::ZeroFindings, &ReviewHash::Empty).unwrap();
    let map = store.read_latest_finals().unwrap();
    let (_, read_hash) = map.get(&scope).unwrap();
    assert!(read_hash.is_empty());
}

// ── multiple scopes ─────────────────────────────────────────────

#[test]
fn test_multiple_scopes_independent() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path());
    let scope1 = ScopeName::Main(MainScopeName::new("domain").unwrap());
    let scope2 = ScopeName::Other;
    let hash = sample_hash();

    store.write_verdict(&scope1, &Verdict::ZeroFindings, &hash).unwrap();
    let finding = sample_finding();
    let v2 = Verdict::findings_remain(vec![finding]).unwrap();
    store.write_verdict(&scope2, &v2, &hash).unwrap();

    let map = store.read_latest_finals().unwrap();
    assert_eq!(map.len(), 2);
    assert!(matches!(map.get(&scope1).unwrap().0, Verdict::ZeroFindings));
    assert!(matches!(map.get(&scope2).unwrap().0, Verdict::FindingsRemain(_)));
}

// ── init is idempotent ──────────────────────────────────────────

#[test]
fn test_init_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path());
    store.init().unwrap();
    store.init().unwrap(); // second call should not fail

    let content = std::fs::read_to_string(dir.path().join("review.json")).unwrap();
    let value: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(value["schema_version"], 2);
}

// ── symlink rejection on read (P0 fix) ──────────────────────────

#[cfg(unix)]
#[test]
fn test_read_rejects_symlinked_review_json() {
    let dir = tempfile::tempdir().unwrap();
    let real_dir = tempfile::tempdir().unwrap();

    // Create a real review.json with zero_findings in a different directory
    let real_path = real_dir.path().join("review.json");
    std::fs::write(&real_path, r#"{"schema_version":2,"scopes":{}}"#).unwrap();

    // Symlink review.json → real_path
    let link_path = dir.path().join("review.json");
    std::os::unix::fs::symlink(&real_path, &link_path).unwrap();

    let store = make_store(dir.path());
    let result = store.read_latest_finals();
    assert!(result.is_err(), "should reject symlinked review.json on read");
}

// ── malformed schema_version (P1 fix) ───────────────────────────

#[test]
fn test_write_rejects_malformed_schema_version() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("review.json");

    // Write a file with schema_version as a string (non-numeric)
    std::fs::write(&path, r#"{"schema_version":"two","scopes":{}}"#).unwrap();

    let store = make_store(dir.path());
    let scope = sample_scope();
    let result = store.write_verdict(&scope, &Verdict::ZeroFindings, &sample_hash());
    assert!(result.is_err(), "should reject non-numeric schema_version on write");
}

#[test]
fn test_write_rejects_missing_schema_version() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("review.json");

    // Write a file with no schema_version field
    std::fs::write(&path, r#"{"scopes":{}}"#).unwrap();

    let store = make_store(dir.path());
    let scope = sample_scope();
    let result = store.write_verdict(&scope, &Verdict::ZeroFindings, &sample_hash());
    assert!(result.is_err(), "should reject missing schema_version on write");
}

#[test]
fn test_read_treats_missing_schema_version_as_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("review.json");

    // Write a file with no schema_version field
    std::fs::write(&path, r#"{"scopes":{}}"#).unwrap();

    let store = make_store(dir.path());
    // Read path should treat as empty (fail-closed), not error
    let map = store.read_latest_finals().unwrap();
    assert!(map.is_empty());
}

// ── ReviewExistsPort ────────────────────────────────────────────

#[test]
fn test_review_json_exists_returns_false_when_file_absent() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path());
    // No review.json created — must return Ok(false)
    let result = store.review_json_exists().unwrap();
    assert!(!result);
}

#[test]
fn test_review_json_exists_returns_true_after_init() {
    let dir = tempfile::tempdir().unwrap();
    let store = make_store(dir.path());
    store.init().unwrap();
    let result = store.review_json_exists().unwrap();
    assert!(result);
}

#[test]
fn test_review_json_exists_returns_true_when_file_present() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("review.json");
    std::fs::write(&path, r#"{"schema_version":2,"scopes":{}}"#).unwrap();
    let store = make_store(dir.path());
    let result = store.review_json_exists().unwrap();
    assert!(result);
}

#[cfg(unix)]
#[test]
fn test_review_json_exists_returns_err_on_permission_denied() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();

    // Create a sub-directory where review.json would live, then remove all permissions.
    let locked_dir = dir.path().join("locked");
    std::fs::create_dir(&locked_dir).unwrap();
    // Place review.json inside the locked dir so that metadata() on the file fails.
    let path = locked_dir.join("review.json");
    std::fs::write(&path, r#"{"schema_version":2,"scopes":{}}"#).unwrap();
    // Remove read+execute from the parent dir → metadata() on the file returns PermissionDenied.
    std::fs::set_permissions(&locked_dir, std::fs::Permissions::from_mode(0o000)).unwrap();

    let store = FsReviewStore::new(path, dir.path().to_path_buf());
    let result = store.review_json_exists();

    // Restore permissions so tempdir cleanup can succeed.
    std::fs::set_permissions(&locked_dir, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert!(result.is_err(), "expected Err on PermissionDenied, got {result:?}");
}

// ── empty file handling ─────────────────────────────────────────

#[test]
fn test_read_empty_file_returns_empty_map() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("review.json");

    // Zero-length file (e.g., interrupted write)
    std::fs::write(&path, "").unwrap();

    let store = make_store(dir.path());
    let map = store.read_latest_finals().unwrap();
    assert!(map.is_empty());
}

#[test]
fn test_read_whitespace_only_file_returns_empty_map() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("review.json");

    std::fs::write(&path, "  \n  ").unwrap();

    let store = make_store(dir.path());
    let map = store.read_latest_finals().unwrap();
    assert!(map.is_empty());
}
