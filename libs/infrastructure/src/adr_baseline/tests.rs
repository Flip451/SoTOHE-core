#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::fs;
use std::path::Path;
use std::process::Command;

use domain::adr_baseline::{
    AdrBaselineKind, AdrBaselineLedgerEntry, AdrBaselineRecordedCopyStatus, AdrBaselineSourceState,
    AdrSourceFileName,
};
use domain::{ContentHash, NonEmptyString, Timestamp, TrackId};
use sha2::{Digest as _, Sha256};
use usecase::adr_baseline::{
    AdrBaselineSourcePort, AdrBaselineStorePort, AdrBaselineStoreReadPort,
};

use super::{FsAdrBaselineStore, FsGitAdrBaselineSource, decode_ledger_line, encode_ledger_entry};

const TRACK_ITEMS: &str = "track/items";
const BASELINE_DIR: &str = "adr-baseline";
const MAX_ADR_BYTES: usize = 8 * 1024 * 1024;
const MAX_LEDGER_BYTES: usize = 8 * 1024 * 1024;
const MAX_LEDGER_LINE_BYTES: usize = 64 * 1024;
const MAX_LEDGER_ENTRIES: usize = 10_000;

fn source() -> AdrSourceFileName {
    AdrSourceFileName::try_new("decision.md".to_owned()).unwrap()
}

fn source_named(name: &str) -> AdrSourceFileName {
    AdrSourceFileName::try_new(name.to_owned()).unwrap()
}

fn drive_prefixed_source() -> AdrSourceFileName {
    AdrSourceFileName::try_new("C:outside.md".to_owned()).unwrap()
}

fn track() -> TrackId {
    TrackId::try_new("adapter-test".to_owned()).unwrap()
}

fn entry() -> AdrBaselineLedgerEntry {
    AdrBaselineLedgerEntry::Init {
        source: source(),
        hash: ContentHash::from_bytes(Sha256::digest(b"contents").into()),
        timestamp: Timestamp::new("2026-07-16T00:00:00Z").unwrap(),
    }
}

fn timestamp() -> Timestamp {
    Timestamp::new("2026-07-16T00:00:00Z").unwrap()
}

fn write_metadata(root: &Path, schema_version: u32, extra_field: &str) {
    let track_dir = root.join(TRACK_ITEMS).join(track().as_ref());
    fs::create_dir_all(&track_dir).unwrap();
    fs::write(
        track_dir.join("metadata.json"),
        format!(
            r#"{{
  "schema_version": {schema_version},
  "id": "adapter-test",
  "title": "Adapter test",
  "created_at": "2026-07-16T00:00:00Z",
  "updated_at": "2026-07-16T00:00:00Z",
  "branch_strategy_snapshot": {{
    "base_branch": "develop",
    "merge_target": "develop",
    "merge_method": "squash"
  }}{extra_field}
}}"#,
        ),
    )
    .unwrap();
}

fn initialize_git_repository(root: &Path) {
    run_git(root, &["init", "-q"]);
    run_git(root, &["config", "user.email", "test@example.invalid"]);
    run_git(root, &["config", "user.name", "Test User"]);
    fs::write(root.join("README.md"), "fixture").unwrap();
    run_git(root, &["add", "."]);
    run_git(root, &["commit", "-qm", "baseline"]);
    run_git(root, &["branch", "develop"]);
    run_git(root, &["checkout", "-qb", "feature"]);
}

#[test]
fn test_adr_baseline_ledger_codec_round_trips_entry() {
    let encoded = encode_ledger_entry(&entry()).unwrap();
    assert_eq!(decode_ledger_line(&encoded).unwrap(), entry());
}

#[test]
fn test_adr_baseline_ledger_codec_init_record_omits_reason_key() {
    let encoded = encode_ledger_entry(&entry()).unwrap();
    let record = serde_json::from_str::<serde_json::Value>(&encoded).unwrap();

    assert!(
        !record.as_object().unwrap().contains_key("reason"),
        "init ledger records must omit an absent reason"
    );
    assert_eq!(decode_ledger_line(&encoded).unwrap(), entry());
}

#[test]
fn test_fs_adr_baseline_store_appends_identical_snapshot_without_duplicate_copy() {
    let temp = tempfile::tempdir().unwrap();
    let store = FsAdrBaselineStore::from(temp.path().to_path_buf());
    store
        .snapshot(
            &track(),
            &source(),
            b"contents".to_vec(),
            AdrBaselineKind::Init,
            None,
            timestamp(),
        )
        .unwrap();
    store
        .snapshot(
            &track(),
            &source(),
            b"contents".to_vec(),
            AdrBaselineKind::NonSemanticFix,
            None,
            timestamp(),
        )
        .unwrap();
    let dir = temp.path().join(TRACK_ITEMS).join(track().as_ref()).join(BASELINE_DIR);
    let copies = fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|extension| extension == "md"))
        .count();
    assert_eq!(copies, 1);
    assert_eq!(store.read_entries(&track()).unwrap().len(), 2);
}

#[test]
fn test_fs_adr_baseline_store_verifies_and_restores_latest_copy() {
    let temp = tempfile::tempdir().unwrap();
    let store = FsAdrBaselineStore::from(temp.path().to_path_buf());
    let source_path = temp.path().join("knowledge/adr");
    fs::create_dir_all(&source_path).unwrap();
    fs::write(source_path.join(source().as_str()), b"old").unwrap();
    store
        .snapshot(
            &track(),
            &source(),
            b"baseline".to_vec(),
            AdrBaselineKind::Init,
            None,
            timestamp(),
        )
        .unwrap();
    let recorded = store.read_entries(&track()).unwrap().pop().unwrap();
    assert_eq!(
        store.verify_recorded_copy(&track(), &recorded).unwrap(),
        AdrBaselineRecordedCopyStatus::Matches
    );
    fs::write(source_path.join(source().as_str()), b"changed").unwrap();
    store.restore(&track(), &source()).unwrap();
    assert_eq!(fs::read(source_path.join(source().as_str())).unwrap(), b"baseline");
}

#[test]
fn test_fs_adr_baseline_store_reports_tampered_copy_hash() {
    let temp = tempfile::tempdir().unwrap();
    let store = FsAdrBaselineStore::from(temp.path().to_path_buf());
    store
        .snapshot(
            &track(),
            &source(),
            b"contents".to_vec(),
            AdrBaselineKind::Init,
            None,
            timestamp(),
        )
        .unwrap();
    let recorded = store.read_entries(&track()).unwrap().pop().unwrap();
    let baseline_dir = temp.path().join(TRACK_ITEMS).join(track().as_ref()).join(BASELINE_DIR);
    let copy = fs::read_dir(&baseline_dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|extension| extension == "md"))
        .unwrap();
    fs::write(copy, b"tampered").unwrap();

    assert_eq!(
        store.verify_recorded_copy(&track(), &recorded).unwrap(),
        AdrBaselineRecordedCopyStatus::HashMismatch {
            actual: ContentHash::from_bytes(Sha256::digest(b"tampered").into()),
        }
    );
    assert!(store.restore(&track(), &source()).is_err());
}

#[test]
fn test_fs_adr_baseline_store_uses_full_hash_when_short_copy_prefix_collides() {
    let temp = tempfile::tempdir().unwrap();
    let store = FsAdrBaselineStore::from(temp.path().to_path_buf());
    let recorded = entry();
    let hash = recorded.hash().to_hex();
    let baseline_dir = temp.path().join(TRACK_ITEMS).join(track().as_ref()).join(BASELINE_DIR);
    fs::create_dir_all(&baseline_dir).unwrap();
    fs::write(baseline_dir.join(format!("decision.{}.md", &hash[..8])), b"wrong snapshot").unwrap();
    fs::write(baseline_dir.join(format!("decision.{}.md", &hash[..9])), b"contents").unwrap();
    fs::write(
        baseline_dir.join("ledger.jsonl"),
        format!("{}\n", encode_ledger_entry(&recorded).unwrap()),
    )
    .unwrap();
    let adr_dir = temp.path().join("knowledge/adr");
    fs::create_dir_all(&adr_dir).unwrap();
    fs::write(adr_dir.join(source().as_str()), b"current").unwrap();

    assert_eq!(
        store.verify_recorded_copy(&track(), &recorded).unwrap(),
        AdrBaselineRecordedCopyStatus::Matches
    );
    store.restore(&track(), &source()).unwrap();
    assert_eq!(fs::read(adr_dir.join(source().as_str())).unwrap(), b"contents");
}

#[test]
fn test_fs_adr_baseline_store_treats_valid_hash8_collision_sibling_as_missing_copy() {
    let temp = tempfile::tempdir().unwrap();
    let store = FsAdrBaselineStore::from(temp.path().to_path_buf());
    let actual = ContentHash::from_bytes(Sha256::digest(b"collision sibling").into());
    let mut expected_hex = actual.to_hex();
    let replacement = if expected_hex.as_bytes().get(8) == Some(&b'0') { "1" } else { "0" };
    expected_hex.replace_range(8..9, replacement);
    let expected = ContentHash::try_from_hex(expected_hex).unwrap();
    let recorded =
        AdrBaselineLedgerEntry::Init { source: source(), hash: expected, timestamp: timestamp() };
    let baseline_dir = temp.path().join(TRACK_ITEMS).join(track().as_ref()).join(BASELINE_DIR);
    fs::create_dir_all(&baseline_dir).unwrap();
    fs::write(
        baseline_dir.join(format!("decision.{}.md", &actual.to_hex()[..8])),
        b"collision sibling",
    )
    .unwrap();

    assert_eq!(
        store.verify_recorded_copy(&track(), &recorded).unwrap(),
        AdrBaselineRecordedCopyStatus::Missing
    );
}

#[test]
fn test_fs_adr_baseline_store_rejects_windows_drive_prefixed_source() {
    let temp = tempfile::tempdir().unwrap();
    let store = FsAdrBaselineStore::from(temp.path().to_path_buf());

    assert!(
        store
            .snapshot(
                &track(),
                &drive_prefixed_source(),
                b"contents".to_vec(),
                AdrBaselineKind::Init,
                None,
                timestamp(),
            )
            .is_err()
    );
    assert!(!temp.path().join(TRACK_ITEMS).exists());
}

#[test]
fn test_fs_adr_baseline_store_rejects_invalid_ledger_before_creating_copy() {
    let temp = tempfile::tempdir().unwrap();
    let store = FsAdrBaselineStore::from(temp.path().to_path_buf());
    let baseline_dir = temp.path().join(TRACK_ITEMS).join(track().as_ref()).join(BASELINE_DIR);
    fs::create_dir_all(baseline_dir.join("ledger.jsonl")).unwrap();

    assert!(
        store
            .snapshot(
                &track(),
                &source(),
                b"contents".to_vec(),
                AdrBaselineKind::Init,
                None,
                timestamp(),
            )
            .is_err()
    );

    let copy_count = fs::read_dir(&baseline_dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|extension| extension == "md"))
        .count();
    assert_eq!(copy_count, 0);

    fs::remove_dir(baseline_dir.join("ledger.jsonl")).unwrap();
    store
        .snapshot(
            &track(),
            &source(),
            b"contents".to_vec(),
            AdrBaselineKind::Init,
            None,
            timestamp(),
        )
        .unwrap();
    assert_eq!(store.read_entries(&track()).unwrap().len(), 1);
}

#[cfg(unix)]
#[test]
fn test_fs_adr_baseline_store_rejects_symlinked_ledger() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();
    let baseline_dir = temp.path().join(TRACK_ITEMS).join(track().as_ref()).join(BASELINE_DIR);
    fs::create_dir_all(&baseline_dir).unwrap();
    std::os::unix::fs::symlink(outside.path(), baseline_dir.join("ledger.jsonl")).unwrap();
    let store = FsAdrBaselineStore::from(temp.path().to_path_buf());

    assert!(store.read_entries(&track()).is_err());
    assert!(
        store
            .snapshot(
                &track(),
                &source(),
                b"contents".to_vec(),
                AdrBaselineKind::Init,
                None,
                timestamp(),
            )
            .is_err()
    );
}

#[cfg(unix)]
#[test]
fn test_fs_adr_baseline_store_rejects_symlinked_recorded_copy() {
    let temp = tempfile::tempdir().unwrap();
    let store = FsAdrBaselineStore::from(temp.path().to_path_buf());
    store
        .snapshot(
            &track(),
            &source(),
            b"contents".to_vec(),
            AdrBaselineKind::Init,
            None,
            timestamp(),
        )
        .unwrap();
    let recorded = store.read_entries(&track()).unwrap().pop().unwrap();
    let baseline_dir = temp.path().join(TRACK_ITEMS).join(track().as_ref()).join(BASELINE_DIR);
    let copy = fs::read_dir(&baseline_dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|extension| extension == "md"))
        .unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();
    fs::remove_file(&copy).unwrap();
    std::os::unix::fs::symlink(outside.path(), &copy).unwrap();

    assert!(store.verify_recorded_copy(&track(), &recorded).is_err());
}

#[cfg(unix)]
#[test]
fn test_fs_git_adr_baseline_source_rejects_symlinked_track_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();
    let track_dir = temp.path().join(TRACK_ITEMS).join(track().as_ref());
    fs::create_dir_all(&track_dir).unwrap();
    std::os::unix::fs::symlink(outside.path(), track_dir.join("metadata.json")).unwrap();
    let adapter = FsGitAdrBaselineSource::from(temp.path().to_path_buf());

    assert!(adapter.fork_point_bytes(&track(), &source()).is_err());
}

#[cfg(unix)]
#[test]
fn test_fs_git_adr_baseline_source_rejects_symlinked_spec() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();
    let track_dir = temp.path().join(TRACK_ITEMS).join(track().as_ref());
    fs::create_dir_all(&track_dir).unwrap();
    std::os::unix::fs::symlink(outside.path(), track_dir.join("spec.json")).unwrap();
    let adapter = FsGitAdrBaselineSource::from(temp.path().to_path_buf());

    assert!(adapter.cited_sources(&track()).is_err());
}

#[test]
fn test_fs_git_adr_baseline_source_rejects_windows_drive_prefixed_source() {
    let temp = tempfile::tempdir().unwrap();
    let adapter = FsGitAdrBaselineSource::from(temp.path().to_path_buf());

    assert!(adapter.working_bytes(&drive_prefixed_source()).is_err());
}

#[test]
fn test_fs_git_adr_baseline_source_rejects_unsupported_metadata_schema() {
    let temp = tempfile::tempdir().unwrap();
    write_metadata(temp.path(), 5, "");
    let adapter = FsGitAdrBaselineSource::from(temp.path().to_path_buf());

    assert!(adapter.fork_point_bytes(&track(), &source()).is_err());
}

#[test]
fn test_fs_git_adr_baseline_source_rejects_unknown_metadata_field() {
    let temp = tempfile::tempdir().unwrap();
    write_metadata(temp.path(), 6, ",\n  \"unexpected\": true");
    let adapter = FsGitAdrBaselineSource::from(temp.path().to_path_buf());

    assert!(adapter.fork_point_bytes(&track(), &source()).is_err());
}

#[test]
fn test_fs_git_adr_baseline_source_detects_flow_mapping_user_decision_ref() {
    let temp = tempfile::tempdir().unwrap();
    initialize_git_repository(temp.path());
    write_metadata(temp.path(), 6, "");
    let adr_dir = temp.path().join("knowledge/adr");
    fs::create_dir_all(&adr_dir).unwrap();
    let source = source_named("flow-mapping.md");
    fs::write(
        adr_dir.join(source.as_str()),
        "---\nadr_id: flow-mapping\ndecisions: [{id: D1, status: proposed, user_decision_ref: chat:2026-07-16}]\n---\n",
    )
    .unwrap();
    let adapter = FsGitAdrBaselineSource::from(temp.path().to_path_buf());

    assert_eq!(
        adapter.source_state(&track(), &source).unwrap(),
        AdrBaselineSourceState::TrackBornPromoted
    );
}

#[test]
fn test_fs_git_adr_baseline_source_ignores_user_decision_ref_in_body_prose() {
    let temp = tempfile::tempdir().unwrap();
    initialize_git_repository(temp.path());
    write_metadata(temp.path(), 6, "");
    let adr_dir = temp.path().join("knowledge/adr");
    fs::create_dir_all(&adr_dir).unwrap();
    let source = source_named("body-prose.md");
    fs::write(
        adr_dir.join(source.as_str()),
        "---\nadr_id: body-prose\ndecisions:\n  - id: D1\n    status: proposed\n---\nuser_decision_ref: this is ordinary ADR body prose\n",
    )
    .unwrap();
    let adapter = FsGitAdrBaselineSource::from(temp.path().to_path_buf());

    assert_eq!(
        adapter.source_state(&track(), &source).unwrap(),
        AdrBaselineSourceState::TrackBornDraft
    );
}

#[test]
fn test_fs_git_adr_baseline_source_rejects_oversized_working_adr() {
    let temp = tempfile::tempdir().unwrap();
    let adr_dir = temp.path().join("knowledge/adr");
    fs::create_dir_all(&adr_dir).unwrap();
    fs::write(adr_dir.join(source().as_str()), vec![b'x'; MAX_ADR_BYTES + 1]).unwrap();
    let adapter = FsGitAdrBaselineSource::from(temp.path().to_path_buf());

    assert!(adapter.working_bytes(&source()).is_err());
}

#[test]
fn test_fs_git_adr_baseline_source_rejects_oversized_fork_point_adr() {
    let temp = tempfile::tempdir().unwrap();
    run_git(temp.path(), &["init", "-q"]);
    run_git(temp.path(), &["config", "user.email", "test@example.invalid"]);
    run_git(temp.path(), &["config", "user.name", "Test User"]);
    let adr_dir = temp.path().join("knowledge/adr");
    fs::create_dir_all(&adr_dir).unwrap();
    fs::write(adr_dir.join(source().as_str()), vec![b'x'; MAX_ADR_BYTES + 1]).unwrap();
    run_git(temp.path(), &["add", "."]);
    run_git(temp.path(), &["commit", "-qm", "baseline"]);
    run_git(temp.path(), &["branch", "develop"]);
    run_git(temp.path(), &["checkout", "-qb", "feature"]);
    write_metadata(temp.path(), 6, "");
    let adapter = FsGitAdrBaselineSource::from(temp.path().to_path_buf());

    assert!(adapter.fork_point_bytes(&track(), &source()).is_err());
}

#[test]
fn test_fs_adr_baseline_store_rejects_oversized_ledger() {
    let temp = tempfile::tempdir().unwrap();
    let baseline_dir = temp.path().join(TRACK_ITEMS).join(track().as_ref()).join(BASELINE_DIR);
    fs::create_dir_all(&baseline_dir).unwrap();
    fs::write(baseline_dir.join("ledger.jsonl"), vec![b'x'; MAX_LEDGER_BYTES + 1]).unwrap();
    let store = FsAdrBaselineStore::from(temp.path().to_path_buf());

    assert!(store.read_entries(&track()).is_err());
}

#[test]
fn test_fs_adr_baseline_store_rejects_oversized_snapshot_before_writing() {
    let temp = tempfile::tempdir().unwrap();
    let store = FsAdrBaselineStore::from(temp.path().to_path_buf());

    assert!(
        store
            .snapshot(
                &track(),
                &source(),
                vec![b'x'; MAX_ADR_BYTES + 1],
                AdrBaselineKind::Init,
                None,
                timestamp(),
            )
            .is_err()
    );
    assert!(!temp.path().join(TRACK_ITEMS).exists());
}

#[test]
fn test_fs_adr_baseline_store_rejects_append_when_ledger_byte_limit_would_be_exceeded() {
    let temp = tempfile::tempdir().unwrap();
    let baseline_dir = temp.path().join(TRACK_ITEMS).join(track().as_ref()).join(BASELINE_DIR);
    fs::create_dir_all(&baseline_dir).unwrap();
    fs::write(baseline_dir.join("ledger.jsonl"), vec![b'\n'; MAX_LEDGER_BYTES]).unwrap();
    let store = FsAdrBaselineStore::from(temp.path().to_path_buf());

    assert!(
        store
            .snapshot(
                &track(),
                &source(),
                b"contents".to_vec(),
                AdrBaselineKind::Init,
                None,
                timestamp(),
            )
            .is_err()
    );
    assert_eq!(
        fs::read_dir(&baseline_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|extension| extension == "md"))
            .count(),
        0
    );
}

#[test]
fn test_fs_adr_baseline_store_rejects_append_when_ledger_entry_limit_is_reached() {
    let temp = tempfile::tempdir().unwrap();
    let baseline_dir = temp.path().join(TRACK_ITEMS).join(track().as_ref()).join(BASELINE_DIR);
    fs::create_dir_all(&baseline_dir).unwrap();
    let line = format!("{}\n", encode_ledger_entry(&entry()).unwrap());
    fs::write(baseline_dir.join("ledger.jsonl"), line.repeat(MAX_LEDGER_ENTRIES)).unwrap();
    let store = FsAdrBaselineStore::from(temp.path().to_path_buf());

    assert!(
        store
            .snapshot(
                &track(),
                &source(),
                b"contents".to_vec(),
                AdrBaselineKind::Init,
                None,
                timestamp(),
            )
            .is_err()
    );
}

#[test]
fn test_fs_adr_baseline_store_rejects_append_when_encoded_record_exceeds_line_limit() {
    let temp = tempfile::tempdir().unwrap();
    let store = FsAdrBaselineStore::from(temp.path().to_path_buf());

    assert!(
        store
            .snapshot(
                &track(),
                &source(),
                b"contents".to_vec(),
                AdrBaselineKind::Escalation,
                Some(NonEmptyString::try_new("x".repeat(MAX_LEDGER_LINE_BYTES)).unwrap()),
                timestamp(),
            )
            .is_err()
    );
    let baseline_dir = temp.path().join(TRACK_ITEMS).join(track().as_ref()).join(BASELINE_DIR);
    assert_eq!(
        fs::read_dir(&baseline_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|extension| extension == "md"))
            .count(),
        0
    );
}

#[test]
fn test_fs_adr_baseline_store_rejects_oversized_ledger_line() {
    let temp = tempfile::tempdir().unwrap();
    let baseline_dir = temp.path().join(TRACK_ITEMS).join(track().as_ref()).join(BASELINE_DIR);
    fs::create_dir_all(&baseline_dir).unwrap();
    fs::write(
        baseline_dir.join("ledger.jsonl"),
        format!("{}\n", "x".repeat(MAX_LEDGER_LINE_BYTES + 1)),
    )
    .unwrap();
    let store = FsAdrBaselineStore::from(temp.path().to_path_buf());

    assert!(store.read_entries(&track()).is_err());
}

#[test]
fn test_fs_adr_baseline_store_rejects_excessive_ledger_entries() {
    let temp = tempfile::tempdir().unwrap();
    let baseline_dir = temp.path().join(TRACK_ITEMS).join(track().as_ref()).join(BASELINE_DIR);
    fs::create_dir_all(&baseline_dir).unwrap();
    let line = format!("{}\n", encode_ledger_entry(&entry()).unwrap());
    fs::write(baseline_dir.join("ledger.jsonl"), line.repeat(MAX_LEDGER_ENTRIES + 1)).unwrap();
    let store = FsAdrBaselineStore::from(temp.path().to_path_buf());

    assert!(store.read_entries(&track()).is_err());
}

#[test]
fn test_fs_git_adr_baseline_source_reads_cite_bytes_from_fork_point() {
    let temp = tempfile::tempdir().unwrap();
    run_git(temp.path(), &["init", "-q"]);
    run_git(temp.path(), &["config", "user.email", "test@example.invalid"]);
    run_git(temp.path(), &["config", "user.name", "Test User"]);
    let adr_dir = temp.path().join("knowledge/adr");
    fs::create_dir_all(&adr_dir).unwrap();
    fs::write(adr_dir.join(source().as_str()), b"fork-point bytes").unwrap();
    run_git(temp.path(), &["add", "."]);
    run_git(temp.path(), &["commit", "-qm", "baseline"]);
    run_git(temp.path(), &["branch", "develop"]);
    run_git(temp.path(), &["checkout", "-qb", "feature"]);
    fs::write(adr_dir.join(source().as_str()), b"working-tree bytes").unwrap();

    let track_dir = temp.path().join(TRACK_ITEMS).join(track().as_ref());
    fs::create_dir_all(&track_dir).unwrap();
    fs::write(
        track_dir.join("metadata.json"),
        r#"{
  "schema_version": 6,
  "id": "adapter-test",
  "title": "Adapter test",
  "created_at": "2026-07-16T00:00:00Z",
  "updated_at": "2026-07-16T00:00:00Z",
  "branch_strategy_snapshot": {
    "base_branch": "develop",
    "merge_target": "develop",
    "merge_method": "squash"
  }
}"#,
    )
    .unwrap();

    let adapter = FsGitAdrBaselineSource::from(temp.path().to_path_buf());
    assert_eq!(adapter.fork_point_bytes(&track(), &source()).unwrap(), b"fork-point bytes");
}

fn run_git(root: &Path, args: &[&str]) {
    let status = Command::new("git").args(args).current_dir(root).status().unwrap();
    assert!(status.success(), "git command failed: git {}", args.join(" "));
}
