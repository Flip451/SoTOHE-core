use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::deletions::DeletionRecord;
use domain::tddd::catalogue_v2::identifiers::{CatalogueItemNamespace, CrateName};
use domain::tddd::catalogue_v2::{CatalogueDocument, CatalogueEntryKey};
use domain::tddd::signal_evaluator::ThreeWaySignalIdentity;
use domain::{CommitHash, ConfidenceSignal, FreeText, Timestamp, TypeSignal};

use super::*;

static CWD_LOCK: Mutex<()> = Mutex::new(());

fn test_cache_key(
    declaration_hash: domain::CatalogueDeclarationHash,
    head_commit: domain::CommitHash,
    baseline_hash: domain::BaselineHash,
) -> domain::TypeSignalsCacheKey {
    let target = domain::ResolvedCargoTargetDirectory::try_new(PathBuf::from(
        "/tmp/sotohe-infrastructure-test-target",
    ))
    .expect("test target is absolute");
    let expected =
        domain::ExpectedRustdocJsonPath::try_new(target.as_path().join("doc/legacy.json"), &target)
            .expect("test output is contained");
    let identity = domain::RustdocExecutionIdentity::new(
        target,
        domain::tddd::catalogue_v2::CrateName::new("legacy").expect("test crate is valid"),
        vec![],
        domain::CargoProfileName::try_new("dev".to_owned()).expect("test profile is valid"),
        expected,
    )
    .expect("test identity is internally consistent");
    let zero = domain::Sha256Digest::try_new("0".repeat(64)).expect("test digest is valid");
    domain::TypeSignalsCacheKey::new(
        declaration_hash,
        head_commit,
        baseline_hash,
        domain::ImplementationFingerprint::new(zero.clone()),
        domain::ResolutionFingerprint::new(zero),
        identity,
    )
}

fn test_signal_document(signals: Vec<TypeSignal>) -> domain::TypeSignalsDocument {
    domain::TypeSignalsDocument::new(
        Timestamp::new("2026-08-27T00:00:00Z").expect("test timestamp must be valid"),
        test_cache_key(
            type_signals_codec::declaration_hash(b"test catalogue"),
            CommitHash::try_new("a".repeat(40)).expect("test commit must be valid"),
            type_signals_codec::baseline_hash(b"test baseline"),
        ),
        signals,
    )
}

fn same_name_deleted_type_and_trait_catalogue() -> CatalogueDocument {
    let mut catalogue = CatalogueDocument::new(
        5,
        CrateName::new("infrastructure").expect("test crate name must be valid"),
        LayerId::try_new("infrastructure").expect("test layer id must be valid"),
    );
    catalogue.push_deletion(DeletionRecord::Type {
        name: CatalogueEntryKey::try_new("Shared".to_owned()).expect("test type key must be valid"),
        spec_refs: vec![],
        informal_grounds: vec![],
    });
    catalogue.push_deletion(DeletionRecord::Trait {
        name: CatalogueEntryKey::try_new("Shared".to_owned())
            .expect("test trait key must be valid"),
        spec_refs: vec![],
        informal_grounds: vec![],
    });
    catalogue
}

fn catalogue_item_signal(
    name: &str,
    namespace: CatalogueItemNamespace,
    kind_tag: &str,
    signal: ConfidenceSignal,
) -> TypeSignal {
    TypeSignal::new(
        ThreeWaySignalIdentity::CatalogueItem { item_name: FreeText::new(name), namespace },
        kind_tag.to_owned(),
        signal,
        false,
        vec![],
        vec![],
        vec![],
    )
}

#[test]
fn test_coverage_keeps_same_name_deleted_type_and_trait_rows_independent() {
    let catalogue = same_name_deleted_type_and_trait_catalogue();
    let document = test_signal_document(vec![
        catalogue_item_signal(
            "Shared",
            CatalogueItemNamespace::Type,
            "unknown",
            ConfidenceSignal::Blue,
        ),
        catalogue_item_signal(
            "Shared",
            CatalogueItemNamespace::Trait,
            "unknown",
            ConfidenceSignal::Yellow,
        ),
    ]);

    validate_impl_catalog_coverage(&catalogue, &document)
        .expect("same-name type and trait deletion identities must both be covered");
}

#[test]
fn test_coverage_keeps_label_distinct_from_same_named_catalogue_item() {
    let catalogue = CatalogueDocumentCodec::decode(infrastructure_catalogue(), "infrastructure")
        .expect("fixture catalogue must decode");
    let document = test_signal_document(vec![
        catalogue_item_signal(
            "SystemSignalReportSourceAdapter",
            CatalogueItemNamespace::Type,
            "secondary_adapter",
            ConfidenceSignal::Blue,
        ),
        TypeSignal::new(
            ThreeWaySignalIdentity::Label {
                label: FreeText::new("SystemSignalReportSourceAdapter"),
            },
            "unknown".to_owned(),
            ConfidenceSignal::Red,
            true,
            vec![],
            vec![],
            vec![],
        ),
    ]);

    validate_impl_catalog_coverage(&catalogue, &document)
        .expect("a report label may share text with a catalogue item");
}

#[test]
fn test_coverage_keeps_same_name_live_type_and_trait_rows_independent() {
    let catalogue =
        CatalogueDocumentCodec::decode(&duplicate_bare_name_catalogue(), "infrastructure")
            .expect("fixture catalogue must decode");
    let document = test_signal_document(vec![
        catalogue_item_signal(
            "SystemSignalReportSourceAdapter",
            CatalogueItemNamespace::Type,
            "secondary_adapter",
            ConfidenceSignal::Yellow,
        ),
        catalogue_item_signal(
            "SystemSignalReportSourceAdapter",
            CatalogueItemNamespace::Trait,
            "secondary_port",
            ConfidenceSignal::Red,
        ),
    ]);

    validate_impl_catalog_coverage(&catalogue, &document)
        .expect("live type and trait rows must be matched by namespace");
}

#[test]
fn test_occurrence_control_character_in_rendered_field_returns_source_unavailable() {
    for fields in [
        ("entry\n", "reference", "reason", "location.md"),
        ("entry", "reference\u{1b}", "reason", "location.md"),
        ("entry", "reference", "reason\r", "location.md"),
        ("entry", "reference", "reason", "location\t.md"),
        ("entry\u{2028}", "reference", "reason", "location.md"),
        ("entry", "reference\u{2029}", "reason", "location.md"),
    ] {
        let error = occurrence(
            SignalReportChain::AdrUser,
            SignalReportLevel::Yellow,
            fields.0.to_owned(),
            fields.1.to_owned(),
            fields.2.to_owned(),
            fields.3.to_owned(),
        )
        .expect_err("control characters must be rejected before rendering");
        assert!(matches!(error, SignalReportError::SourceUnavailable(SignalReportChain::AdrUser)));
    }
}

struct CwdGuard {
    original: PathBuf,
}

impl CwdGuard {
    fn enter(path: &Path) -> Self {
        let original = std::env::current_dir().expect("current directory must be readable");
        std::env::set_current_dir(path).expect("test must enter fixture repository");
        Self { original }
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.original);
    }
}

fn run_git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("git must run for the fixture repository");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_output(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("git must run for the fixture repository");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("git fixture output must be UTF-8")
}

fn infrastructure_catalogue() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../track/items/signal-report-command-2026-07-31/infrastructure-types.json"
    ))
}

fn fresh_catalogue_spec_signals(catalogue_text: &str, signal: &str) -> String {
    let catalogue = CatalogueDocumentCodec::decode(catalogue_text, "infrastructure")
        .expect("fixture catalogue must decode");
    let signals = iter_catalogue_entries(&catalogue)
        .map(|entry| {
            let (section, entry_key) = entry
                .section_key
                .split_once(':')
                .expect("fixture entry must have a section-qualified key");
            serde_json::json!({
                "type_name": entry.key,
                "signal": signal,
                "entry_hash": compute_catalogue_entry_hash(catalogue_text, section, entry_key)
                    .expect("fixture entry hash must compute"),
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string_pretty(&serde_json::json!({
        "schema_version": 1,
        "catalogue_declaration_hash": compute_catalogue_declaration_hash(catalogue_text.as_bytes())
            .as_digest()
            .as_str(),
        "signals": signals,
    }))
    .expect("fixture signals must serialize")
}

/// Bytes of the fixture type baseline; the persisted signal fixture's
/// `baseline_hash` must be the hash of exactly these bytes.
const FIXTURE_BASELINE_BYTES: &[u8] = b"fixture-baseline";

fn current_head(root: &Path) -> String {
    let mut output = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(root)
        .output()
        .expect("git must run for the fixture repository");
    if !output.status.success() {
        run_git(root, &["add", "."]);
        run_git(root, &["commit", "--no-gpg-sign", "-m", "fixture inputs"]);
        output = Command::new("git")
            .args(["rev-parse", "--verify", "HEAD"])
            .current_dir(root)
            .output()
            .expect("git must run for the fixture repository");
    }
    assert!(output.status.success(), "fixture HEAD must resolve");
    String::from_utf8(output.stdout).expect("fixture HEAD must be UTF-8").trim().to_owned()
}

fn fresh_impl_catalog_signals_with_timestamp(
    root: &Path,
    catalogue_text: &str,
    generated_at: &str,
) -> String {
    let mut document = serde_json::json!({
        "schema_version": domain::TYPE_SIGNALS_SCHEMA_VERSION,
        "generated_at": generated_at,
        "declaration_hash": type_signals_codec::declaration_hash(catalogue_text.as_bytes())
            .as_digest()
            .as_str(),
        "head_commit": current_head(root),
        "baseline_hash": type_signals_codec::baseline_hash(FIXTURE_BASELINE_BYTES)
            .as_digest()
            .as_str(),
        "signals": [{
            "namespace": "type",
            "type_name": "SystemSignalReportSourceAdapter",
            "kind_tag": "secondary_adapter",
            "signal": "yellow",
            "found_type": true,
        }],
    });
    type_signals_codec::merge_fixture_reuse_identity(&mut document);
    serde_json::to_string_pretty(&document).expect("fixture signals must serialize")
}

fn fresh_impl_catalog_signals(root: &Path, catalogue_text: &str) -> String {
    fresh_impl_catalog_signals_with_timestamp(root, catalogue_text, "2026-07-31T00:00:00Z")
}

fn commit_signal_artifact(root: &Path, signal_path: &Path, signal: &str, message: &str) {
    fs::write(signal_path, signal).expect("fixture signal artifact must be written");
    let relative_path = signal_path
        .strip_prefix(root)
        .expect("signal path must be inside fixture root")
        .to_str()
        .expect("signal path must be UTF-8")
        .to_owned();
    run_git(root, &["add", &relative_path]);
    run_git(root, &["commit", "--no-gpg-sign", "-m", message]);
}

fn setup_committed_impl_signal_fixture() -> (tempfile::TempDir, TrackId, PathBuf, String) {
    let root = tempfile::tempdir().expect("fixture root must be created");
    let track = TrackId::try_new("report-test").expect("fixture track id must be valid");
    let track_dir = root.path().join("track/items/report-test");
    let catalogue = prepare_catalogue_spec_source(root.path(), &track_dir);
    let signal_path = track_dir.join("infrastructure-type-signals.json");
    let initial_signal = fresh_impl_catalog_signals(root.path(), &catalogue);
    commit_signal_artifact(root.path(), &signal_path, &initial_signal, "initial signals");
    (root, track, signal_path, catalogue)
}

fn prepare_catalogue_spec_source_with_catalogue(
    root: &Path,
    track_dir: &Path,
    catalogue: &str,
) -> String {
    if !root.join(".git").exists() {
        crate::verify::test_support::git_init(root);
    }
    fs::create_dir_all(track_dir).expect("track fixture directory must exist");
    fs::create_dir_all(root.join("libs/infrastructure/src"))
        .expect("fixture crate source directory must exist");
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"libs/infrastructure\"]\nresolver = \"2\"\n",
    )
    .expect("fixture workspace manifest must be written");
    fs::write(root.join("Cargo.lock"), "version = 4\n").expect("fixture lockfile must be written");
    fs::write(root.join(".test-nightly-toolchain-identity"), "rustc fixture-nightly\n")
        .expect("fixture toolchain identity must be written");
    fs::write(
            root.join("libs/infrastructure/Cargo.toml"),
            "[package]\nname = \"infrastructure\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[features]\nfreshness = []\n",
        )
        .expect("fixture crate manifest must be written");
    fs::write(root.join("libs/infrastructure/src/lib.rs"), "pub struct Fixture;\n")
        .expect("fixture crate source must be written");
    fs::write(
            root.join("architecture-rules.json"),
            r#"{"version":2,"layers":[{"crate":"infrastructure","path":"libs/infrastructure","may_depend_on":[],"tddd":{"enabled":true,"catalogue_file":"infrastructure-types.json","catalogue_spec_signal":{"enabled":true}}}]}"#,
        )
        .expect("architecture rules fixture must be written");
    fs::write(track_dir.join("infrastructure-types.json"), catalogue)
        .expect("catalogue fixture must be written");
    let feature_declaration =
        "{\n  \"schema_version\": 1,\n  \"layers\": {\n    \"infrastructure\": []\n  }\n}\n";
    fs::write(track_dir.join("tddd-features.json"), feature_declaration)
        .expect("feature declaration fixture must be written");
    fs::write(track_dir.join("tddd-features-baseline.json"), feature_declaration)
        .expect("feature declaration baseline fixture must be written");
    fs::write(track_dir.join("infrastructure-types-baseline.json"), FIXTURE_BASELINE_BYTES)
        .expect("type baseline fixture must be written");
    catalogue.to_owned()
}

fn prepare_catalogue_spec_source(root: &Path, track_dir: &Path) -> String {
    prepare_catalogue_spec_source_with_catalogue(root, track_dir, infrastructure_catalogue())
}

fn duplicate_bare_name_catalogue() -> String {
    let mut catalogue: serde_json::Value = serde_json::from_str(infrastructure_catalogue())
        .expect("fixture catalogue must be valid JSON");
    let type_entry = catalogue
        .pointer_mut("/types/SystemSignalReportSourceAdapter")
        .and_then(serde_json::Value::as_object_mut)
        .expect("fixture catalogue must contain the adapter type");
    type_entry.insert(
        "spec_refs".to_owned(),
        serde_json::json!([{
            "file": "track/items/report-test/spec.json",
            "anchor": "IN-04",
        }]),
    );
    catalogue
        .pointer_mut("/traits")
        .and_then(serde_json::Value::as_object_mut)
        .expect("fixture catalogue must contain the traits section")
        .insert(
            "SystemSignalReportSourceAdapter".to_owned(),
            serde_json::json!({
                "action": "add",
                "role": {"SecondaryPort": {}},
                "methods": [],
                "supertrait_bounds": [],
                "module_path": "signal_report",
                "docs": "A deliberately duplicate bare entry name.",
                "spec_refs": [],
                "informal_grounds": [],
            }),
        );
    serde_json::to_string_pretty(&catalogue).expect("fixture catalogue must serialize")
}

fn signal_document_object(
    signals: &mut serde_json::Value,
) -> &mut serde_json::Map<String, serde_json::Value> {
    signals.as_object_mut().expect("fixture signals document must be a JSON object")
}

fn seed_report_source_repo() -> tempfile::TempDir {
    let fixture = tempfile::tempdir().expect("fixture directory must be created");
    let root = fixture.path();
    let track_dir = root.join("track/items/report-test");

    run_git(root, &["init", "-q"]);
    run_git(root, &["config", "user.email", "test@example.com"]);
    run_git(root, &["config", "user.name", "Signal Report Test"]);
    run_git(root, &["commit", "--allow-empty", "--no-gpg-sign", "-m", "fixture"]);
    run_git(root, &["checkout", "-b", "track/report-test"]);
    fs::create_dir_all(root.join("knowledge/adr")).expect("ADR fixture directory must exist");
    let catalogue = prepare_catalogue_spec_source(root, &track_dir);
    fs::write(
            root.join("knowledge/adr/report-source.md"),
            "---\nadr_id: report-source\ndecisions:\n  - id: D1\n    status: proposed\n    review_finding_ref: review:123\n---\n",
        )
        .expect("ADR fixture must be written");

    let mut spec: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../track/items/signal-report-command-2026-07-31/spec.json"
    )))
    .expect("track spec fixture must be valid JSON");
    *spec
        .pointer_mut("/scope/in_scope/0/adr_refs")
        .expect("fixture must include the first in-scope ADR reference") = serde_json::json!([]);
    fs::write(
        track_dir.join("spec.json"),
        serde_json::to_string_pretty(&spec).expect("fixture spec must serialize"),
    )
    .expect("spec fixture must be written");
    fs::write(
        track_dir.join("infrastructure-catalogue-spec-signals.json"),
        fresh_catalogue_spec_signals(&catalogue, "yellow"),
    )
    .expect("catalogue signals fixture must be written");
    fs::write(
        track_dir.join("infrastructure-type-signals.json"),
        fresh_impl_catalog_signals(root, &catalogue),
    )
    .expect("implementation signals fixture must be written");
    fixture
}

fn snapshot_tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn collect(root: &Path, directory: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let mut paths = fs::read_dir(directory)
            .expect("fixture directory must be readable")
            .map(|entry| entry.expect("fixture entry must be readable").path())
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            let metadata = fs::symlink_metadata(&path).expect("fixture metadata must be readable");
            if metadata.is_dir() {
                collect(root, &path, files);
            } else if metadata.is_file() {
                files.insert(
                    path.strip_prefix(root)
                        .expect("fixture path must remain below its root")
                        .to_path_buf(),
                    fs::read(&path).expect("fixture file must be readable"),
                );
            }
        }
    }

    let mut files = BTreeMap::new();
    collect(root, root, &mut files);
    files
}

#[test]
fn test_signal_report_adapter_catalogue_context_describes_unpersisted_ground() {
    let ground = domain::InformalGroundRef::new(
        domain::InformalGroundKind::Discussion,
        domain::InformalGroundSummary::try_new("pending approval").unwrap(),
    );
    let (reference, reason) = catalogue_context(&[], &[ground]);
    assert_eq!(reference, "discussion:pending approval");
    assert!(reason.contains("unpromoted"));
}

#[test]
fn test_signal_report_adapter_non_blue_levels_map_to_report_levels() {
    assert_eq!(report_level(ConfidenceSignal::Blue), None);
    assert_eq!(report_level(ConfidenceSignal::Yellow), Some(SignalReportLevel::Yellow));
    assert_eq!(report_level(ConfidenceSignal::Red), Some(SignalReportLevel::Red));
}

#[test]
fn test_signal_report_adapter_derives_spec_occurrences_without_persisting() {
    let root = tempfile::tempdir().unwrap();
    let track = TrackId::try_new("report-test").unwrap();
    let track_dir = root.path().join("track/items/report-test");
    fs::create_dir_all(&track_dir).unwrap();
    let mut spec: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../track/items/signal-report-command-2026-07-31/spec.json"
    )))
    .unwrap();
    *spec
        .pointer_mut("/scope/in_scope/0/adr_refs")
        .expect("fixture must include the first in-scope ADR reference") = serde_json::json!([]);
    let spec_text = serde_json::to_string_pretty(&spec).unwrap();
    let spec_path = track_dir.join("spec.json");
    fs::write(&spec_path, &spec_text).unwrap();

    let occurrences =
        SystemSignalReportSourceAdapter::derive_spec_adr(root.path(), &track).unwrap();

    assert!(occurrences.iter().any(|row| {
        row.chain == SignalReportChain::SpecAdr
            && row.level == SignalReportLevel::Red
            && row.entry_id.to_string() == "IN-01"
    }));
    assert_eq!(fs::read_to_string(spec_path).unwrap(), spec_text);
}

#[test]
fn test_signal_report_adapter_read_text_rejects_oversized_artifact() {
    let root = tempfile::tempdir().expect("fixture root must be created");
    let artifact = root.path().join("oversized.json");
    File::create(&artifact)
        .expect("fixture artifact must be created")
        .set_len(crate::capability_exec::MAX_CAPABILITY_EXEC_TEXT_BYTES + 1)
        .expect("fixture artifact must become oversized");

    let error = read_text(root.path(), &artifact, SignalReportChain::SpecAdr)
        .expect_err("oversized artifact must be rejected before decoding");

    assert!(matches!(error, SignalReportError::SourceUnavailable(SignalReportChain::SpecAdr)));
}

#[test]
fn test_signal_report_adapter_derives_adr_user_occurrences_without_persisting() {
    let root = tempfile::tempdir().unwrap();
    let adr_dir = root.path().join("knowledge/adr");
    fs::create_dir_all(&adr_dir).unwrap();
    let adr_path = adr_dir.join("report-source.md");
    let adr = "---\nadr_id: report-source\ndecisions:\n  - id: D1\n    status: proposed\n    review_finding_ref: review:123\n---\n";
    fs::write(&adr_path, adr).unwrap();

    let occurrences = SystemSignalReportSourceAdapter::derive_adr_user(root.path()).unwrap();

    assert!(matches!(
        occurrences.as_slice(),
        [SignalReportOccurrence {
            chain: SignalReportChain::AdrUser,
            level: SignalReportLevel::Yellow,
            ..
        }]
    ));
    assert_eq!(fs::read_to_string(adr_path).unwrap(), adr);
}

#[test]
fn test_bounded_adr_paths_selects_and_sorts_markdown_files() {
    let root = tempfile::tempdir().expect("fixture root must be created");
    let adr_dir = root.path().join("knowledge/adr");
    fs::create_dir_all(&adr_dir).expect("ADR fixture directory must exist");
    fs::write(adr_dir.join("zeta.md"), "ADR").expect("Markdown fixture must be written");
    fs::write(adr_dir.join("alpha.MD"), "ADR").expect("Markdown fixture must be written");
    fs::write(adr_dir.join("ignored.txt"), "junk").expect("junk fixture must be written");
    fs::create_dir(adr_dir.join("nested.md")).expect("directory fixture must be created");

    let paths = bounded_adr_paths(&adr_dir).expect("Markdown paths must be selected");

    assert_eq!(paths, vec![adr_dir.join("alpha.MD"), adr_dir.join("zeta.md")]);
}

#[test]
fn test_signal_report_adapter_reads_persisted_catalogue_signal_artifact() {
    let root = tempfile::tempdir().unwrap();
    let track = TrackId::try_new("report-test").unwrap();
    let track_dir = root.path().join("track/items/report-test");
    let catalogue = prepare_catalogue_spec_source(root.path(), &track_dir);
    let signals_path = track_dir.join("infrastructure-catalogue-spec-signals.json");
    let signals = fresh_catalogue_spec_signals(&catalogue, "yellow");
    fs::write(&signals_path, &signals).unwrap();

    let occurrences =
        SystemSignalReportSourceAdapter::read_catalog_spec(root.path(), &track).unwrap();

    assert!(matches!(
        occurrences.as_slice(),
        [SignalReportOccurrence {
            chain: SignalReportChain::CatalogSpec,
            level: SignalReportLevel::Yellow,
            ..
        }]
    ));
    assert_eq!(fs::read_to_string(signals_path).unwrap(), signals);
}

#[test]
fn test_signal_report_adapter_preserves_duplicate_catalogue_entry_contexts() {
    let root = tempfile::tempdir().unwrap();
    let track = TrackId::try_new("report-test").unwrap();
    let track_dir = root.path().join("track/items/report-test");
    let catalogue = prepare_catalogue_spec_source_with_catalogue(
        root.path(),
        &track_dir,
        &duplicate_bare_name_catalogue(),
    );
    fs::write(
        track_dir.join("infrastructure-catalogue-spec-signals.json"),
        fresh_catalogue_spec_signals(&catalogue, "yellow"),
    )
    .unwrap();

    let occurrences =
        SystemSignalReportSourceAdapter::read_catalog_spec(root.path(), &track).unwrap();

    assert!(occurrences.iter().any(|occurrence| {
        occurrence.entry_id.to_string() == "infrastructure:types:SystemSignalReportSourceAdapter"
            && occurrence.reference.to_string() == "track/items/report-test/spec.json#IN-04"
            && occurrence.reason.to_string() == "persisted catalogue-spec signal is non-blue"
    }));
    assert!(occurrences.iter().any(|occurrence| {
        occurrence.entry_id.to_string() == "infrastructure:traits:SystemSignalReportSourceAdapter"
            && occurrence.reference.to_string() == "no specification reference"
            && occurrence.reason.to_string()
                == "catalogue entry has neither specification references nor informal grounds"
    }));
}

#[test]
fn test_signal_report_adapter_rejects_stale_catalogue_declaration_hash() {
    let root = tempfile::tempdir().unwrap();
    let track = TrackId::try_new("report-test").unwrap();
    let track_dir = root.path().join("track/items/report-test");
    let catalogue = prepare_catalogue_spec_source(root.path(), &track_dir);
    let mut signals: serde_json::Value =
        serde_json::from_str(&fresh_catalogue_spec_signals(&catalogue, "yellow")).unwrap();
    signal_document_object(&mut signals)
        .insert("catalogue_declaration_hash".to_owned(), serde_json::json!("a".repeat(64)));
    fs::write(
        track_dir.join("infrastructure-catalogue-spec-signals.json"),
        serde_json::to_string(&signals).unwrap(),
    )
    .unwrap();

    let error = SystemSignalReportSourceAdapter::read_catalog_spec(root.path(), &track)
        .expect_err("a stale declaration hash must fail closed");

    assert!(matches!(error, SignalReportError::SourceUnavailable(SignalReportChain::CatalogSpec)));
}

#[test]
fn test_signal_report_adapter_rejects_missing_extra_or_mismatched_catalogue_signal_coverage() {
    let root = tempfile::tempdir().unwrap();
    let track = TrackId::try_new("report-test").unwrap();
    let track_dir = root.path().join("track/items/report-test");
    let catalogue = prepare_catalogue_spec_source(root.path(), &track_dir);
    let signals_path = track_dir.join("infrastructure-catalogue-spec-signals.json");
    let mut signals: serde_json::Value =
        serde_json::from_str(&fresh_catalogue_spec_signals(&catalogue, "yellow")).unwrap();

    signal_document_object(&mut signals).insert("signals".to_owned(), serde_json::json!([]));
    fs::write(&signals_path, serde_json::to_string(&signals).unwrap()).unwrap();
    let missing = SystemSignalReportSourceAdapter::read_catalog_spec(root.path(), &track)
        .expect_err("missing coverage must fail closed");
    assert!(matches!(
        missing,
        SignalReportError::SourceUnavailable(SignalReportChain::CatalogSpec)
    ));

    signal_document_object(&mut signals).insert(
        "signals".to_owned(),
        serde_json::json!([{
            "type_name": "UnexpectedEntry",
            "signal": "blue",
            "entry_hash": "a".repeat(64),
        }]),
    );
    fs::write(&signals_path, serde_json::to_string(&signals).unwrap()).unwrap();
    let mismatched = SystemSignalReportSourceAdapter::read_catalog_spec(root.path(), &track)
        .expect_err("mismatched identity must fail closed before blue signals are skipped");
    assert!(matches!(
        mismatched,
        SignalReportError::SourceUnavailable(SignalReportChain::CatalogSpec)
    ));

    signal_document_object(&mut signals).insert(
        "signals".to_owned(),
        serde_json::json!([
            {
                "type_name": "SystemSignalReportSourceAdapter",
                "signal": "yellow",
                "entry_hash": compute_catalogue_entry_hash(
                    &catalogue,
                    "types",
                    "SystemSignalReportSourceAdapter"
                )
                .unwrap(),
            },
            {
                "type_name": "UnexpectedEntry",
                "signal": "yellow",
                "entry_hash": "a".repeat(64),
            }
        ]),
    );
    fs::write(&signals_path, serde_json::to_string(&signals).unwrap()).unwrap();
    let extra = SystemSignalReportSourceAdapter::read_catalog_spec(root.path(), &track)
        .expect_err("extra coverage must fail closed");
    assert!(matches!(extra, SignalReportError::SourceUnavailable(SignalReportChain::CatalogSpec)));
}

#[test]
fn test_signal_report_adapter_rejects_stale_catalogue_entry_hash() {
    let root = tempfile::tempdir().unwrap();
    let track = TrackId::try_new("report-test").unwrap();
    let track_dir = root.path().join("track/items/report-test");
    let catalogue = prepare_catalogue_spec_source(root.path(), &track_dir);
    let mut signals: serde_json::Value =
        serde_json::from_str(&fresh_catalogue_spec_signals(&catalogue, "yellow")).unwrap();
    let first_signal = signal_document_object(&mut signals)
        .get_mut("signals")
        .and_then(serde_json::Value::as_array_mut)
        .and_then(|entries| entries.first_mut())
        .and_then(serde_json::Value::as_object_mut)
        .expect("fixture signals document must contain one signal object");
    first_signal.insert("entry_hash".to_owned(), serde_json::json!("a".repeat(64)));
    fs::write(
        track_dir.join("infrastructure-catalogue-spec-signals.json"),
        serde_json::to_string(&signals).unwrap(),
    )
    .unwrap();

    let error = SystemSignalReportSourceAdapter::read_catalog_spec(root.path(), &track)
        .expect_err("a stale entry hash must fail closed");

    assert!(matches!(error, SignalReportError::SourceUnavailable(SignalReportChain::CatalogSpec)));
}

#[test]
fn test_signal_report_adapter_reports_both_impl_catalogue_item_mismatches() {
    let root = tempfile::tempdir().unwrap();
    let track = TrackId::try_new("report-test").unwrap();
    let track_dir = root.path().join("track/items/report-test");
    let catalogue = prepare_catalogue_spec_source(root.path(), &track_dir);
    let mut signals: serde_json::Value =
        serde_json::from_str(&fresh_impl_catalog_signals(root.path(), &catalogue)).unwrap();
    signal_document_object(&mut signals).insert(
        "signals".to_owned(),
        serde_json::json!([{
            "type_name": "SystemSignalReportSourceAdapter",
            "namespace": "type",
            "kind_tag": "secondary_adapter",
            "signal": "yellow",
            "found_type": true,
            "missing_items": ["required_method"],
            "extra_items": ["unexpected_method"],
        }]),
    );
    fs::write(
        track_dir.join("infrastructure-type-signals.json"),
        serde_json::to_string(&signals).unwrap(),
    )
    .unwrap();

    let occurrences =
        SystemSignalReportSourceAdapter::read_impl_catalog(root.path(), &track).unwrap();

    assert!(matches!(
        occurrences.as_slice(),
        [SignalReportOccurrence { reason, .. }]
            if reason.to_string()
                == "missing implementation items: required_method; unexpected implementation items: unexpected_method"
    ));
}

#[test]
fn test_signal_report_adapter_same_name_distinct_kind_preserves_impl_occurrence_identity() {
    let root = tempfile::tempdir().unwrap();
    let track = TrackId::try_new("report-test").unwrap();
    let track_dir = root.path().join("track/items/report-test");
    let catalogue = prepare_catalogue_spec_source_with_catalogue(
        root.path(),
        &track_dir,
        &duplicate_bare_name_catalogue(),
    );
    let mut signals: serde_json::Value =
        serde_json::from_str(&fresh_impl_catalog_signals(root.path(), &catalogue)).unwrap();
    signal_document_object(&mut signals).insert(
        "signals".to_owned(),
        serde_json::json!([
            {
                "type_name": "SystemSignalReportSourceAdapter",
                "namespace": "type",
                "kind_tag": "secondary_adapter",
                "signal": "yellow",
                "found_type": true,
            },
            {
                "type_name": "SystemSignalReportSourceAdapter",
                "namespace": "trait",
                "kind_tag": "secondary_port",
                "signal": "red",
                "found_type": true,
            }
        ]),
    );
    fs::write(
        track_dir.join("infrastructure-type-signals.json"),
        serde_json::to_string(&signals).unwrap(),
    )
    .unwrap();

    let occurrences =
        SystemSignalReportSourceAdapter::read_impl_catalog(root.path(), &track).unwrap();

    assert!(occurrences.iter().any(|occurrence| {
            occurrence.entry_id.to_string()
                == "infrastructure:secondary_adapter:type:SystemSignalReportSourceAdapter"
                && occurrence.level == SignalReportLevel::Yellow
                && occurrence.reference.to_string()
                    == "infrastructure-types.json#secondary_adapter:type:SystemSignalReportSourceAdapter"
        }));
    assert!(occurrences.iter().any(|occurrence| {
        occurrence.entry_id.to_string()
            == "infrastructure:secondary_port:trait:SystemSignalReportSourceAdapter"
            && occurrence.level == SignalReportLevel::Red
            && occurrence.reference.to_string()
                == "infrastructure-types.json#secondary_port:trait:SystemSignalReportSourceAdapter"
    }));
}

#[test]
fn test_signal_report_adapter_rejects_missing_or_phantom_impl_catalogue_identity() {
    let root = tempfile::tempdir().unwrap();
    let track = TrackId::try_new("report-test").unwrap();
    let track_dir = root.path().join("track/items/report-test");
    let catalogue = prepare_catalogue_spec_source(root.path(), &track_dir);
    let signals_path = track_dir.join("infrastructure-type-signals.json");
    let mut signals: serde_json::Value =
        serde_json::from_str(&fresh_impl_catalog_signals(root.path(), &catalogue)).unwrap();

    signal_document_object(&mut signals).insert("signals".to_owned(), serde_json::json!([]));
    fs::write(&signals_path, serde_json::to_string(&signals).unwrap()).unwrap();
    let missing = SystemSignalReportSourceAdapter::read_impl_catalog(root.path(), &track)
        .expect_err("missing canonical implementation coverage must fail closed");
    assert!(matches!(
        missing,
        SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)
    ));

    signal_document_object(&mut signals).insert(
        "signals".to_owned(),
        serde_json::json!([{
            "type_name": "PhantomAdapter",
            "namespace": null,
            "kind_tag": "secondary_adapter",
            "signal": "yellow",
            "found_type": true,
        }]),
    );
    fs::write(&signals_path, serde_json::to_string(&signals).unwrap()).unwrap();
    let phantom = SystemSignalReportSourceAdapter::read_impl_catalog(root.path(), &track)
        .expect_err("a non-catalogue canonical identity must fail closed");
    assert!(matches!(
        phantom,
        SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)
    ));
}

#[test]
fn test_signal_report_adapter_reports_unique_unknown_impl_occurrence() {
    let root = tempfile::tempdir().unwrap();
    let track = TrackId::try_new("report-test").unwrap();
    let track_dir = root.path().join("track/items/report-test");
    let catalogue = prepare_catalogue_spec_source(root.path(), &track_dir);
    let mut signals: serde_json::Value =
        serde_json::from_str(&fresh_impl_catalog_signals(root.path(), &catalogue)).unwrap();
    signal_document_object(&mut signals).insert(
        "signals".to_owned(),
        serde_json::json!([
            {
                "type_name": "SystemSignalReportSourceAdapter",
                "namespace": "type",
                "kind_tag": "secondary_adapter",
                "signal": "blue",
                "found_type": true,
            },
            {
                "type_name": "std::sync::Arc<T>",
                "namespace": null,
                "kind_tag": "unknown",
                "signal": "red",
                "found_type": true,
            }
        ]),
    );
    fs::write(
        track_dir.join("infrastructure-type-signals.json"),
        serde_json::to_string(&signals).unwrap(),
    )
    .unwrap();

    let occurrences =
        SystemSignalReportSourceAdapter::read_impl_catalog(root.path(), &track).unwrap();

    assert!(matches!(
        occurrences.as_slice(),
        [SignalReportOccurrence {
            chain: SignalReportChain::ImplCatalog,
            level: SignalReportLevel::Red,
            entry_id,
            reference,
            ..
        }] if entry_id.to_string() == "infrastructure:unknown:std::sync::Arc<T>"
            && reference.to_string()
                == "infrastructure-types.json#unknown:std::sync::Arc<T>"
    ));
}

#[test]
fn test_signal_report_adapter_rejects_unsafe_impl_report_line_text() {
    let root = tempfile::tempdir().unwrap();
    let track = TrackId::try_new("report-test").unwrap();
    let track_dir = root.path().join("track/items/report-test");
    let catalogue = prepare_catalogue_spec_source(root.path(), &track_dir);
    let signals_path = track_dir.join("infrastructure-type-signals.json");
    let mut signals: serde_json::Value =
        serde_json::from_str(&fresh_impl_catalog_signals(root.path(), &catalogue)).unwrap();

    signal_document_object(&mut signals).insert(
        "signals".to_owned(),
        serde_json::json!([
            {
                "type_name": "SystemSignalReportSourceAdapter",
                "namespace": "type",
                "kind_tag": "secondary_adapter",
                "signal": "blue",
                "found_type": true,
            },
            {
                "type_name": "Forged\nOccurrence",
                "namespace": null,
                "kind_tag": "unknown",
                "signal": "red",
                "found_type": true,
            }
        ]),
    );
    fs::write(&signals_path, serde_json::to_string(&signals).unwrap()).unwrap();
    let unsafe_name = SystemSignalReportSourceAdapter::read_impl_catalog(root.path(), &track)
        .expect_err("an unsafe unknown item path must fail closed");
    assert!(matches!(
        unsafe_name,
        SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)
    ));

    signal_document_object(&mut signals).insert(
        "signals".to_owned(),
        serde_json::json!([{
            "type_name": "SystemSignalReportSourceAdapter",
            "namespace": "type",
            "kind_tag": "secondary_adapter",
            "signal": "yellow",
            "found_type": true,
            "missing_items": ["forged\u{001b}[31mitem"],
        }]),
    );
    fs::write(&signals_path, serde_json::to_string(&signals).unwrap()).unwrap();
    let unsafe_reason = SystemSignalReportSourceAdapter::read_impl_catalog(root.path(), &track)
        .expect_err("an unsafe mismatch item must fail closed");
    assert!(matches!(
        unsafe_reason,
        SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)
    ));
}

#[test]
fn test_signal_report_adapter_rejects_stale_type_baseline() {
    let root = tempfile::tempdir().unwrap();
    let track = TrackId::try_new("report-test").unwrap();
    let track_dir = root.path().join("track/items/report-test");
    let catalogue = prepare_catalogue_spec_source(root.path(), &track_dir);
    let signals_path = track_dir.join("infrastructure-type-signals.json");
    fs::write(&signals_path, fresh_impl_catalog_signals(root.path(), &catalogue)).unwrap();

    // A baseline recapture changes reverse filtering without touching the
    // catalogue or the implementation; the cached signals must go stale.
    fs::write(track_dir.join("infrastructure-types-baseline.json"), b"recaptured-baseline")
        .unwrap();
    let stale_baseline = SystemSignalReportSourceAdapter::read_impl_catalog(root.path(), &track)
        .expect_err("changed type baseline must stale the persisted artifact");
    assert!(matches!(
        stale_baseline,
        SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)
    ));
}

#[test]
fn test_signal_report_adapter_rejects_signal_from_implementation_only_commit() {
    let root = tempfile::tempdir().unwrap();
    let track = TrackId::try_new("report-test").unwrap();
    let track_dir = root.path().join("track/items/report-test");
    let catalogue = prepare_catalogue_spec_source(root.path(), &track_dir);
    fs::write(
        track_dir.join("infrastructure-type-signals.json"),
        fresh_impl_catalog_signals(root.path(), &catalogue),
    )
    .unwrap();

    fs::write(root.path().join("implementation-only.rs"), "changed\n").unwrap();
    run_git(root.path(), &["add", "implementation-only.rs"]);
    run_git(root.path(), &["commit", "--no-gpg-sign", "-m", "implementation-only"]);

    let error = SystemSignalReportSourceAdapter::read_impl_catalog(root.path(), &track)
        .expect_err("an implementation-only commit must stale the signal artifact");
    assert!(matches!(error, SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)));
}

#[test]
fn test_signal_report_adapter_accepts_signal_regenerated_in_all_in_one_commit() {
    let (root, track, signal_path, catalogue) = setup_committed_impl_signal_fixture();
    let regenerated =
        fresh_impl_catalog_signals_with_timestamp(root.path(), &catalogue, "2026-08-01T00:00:00Z");
    fs::write(&signal_path, regenerated).unwrap();
    fs::write(root.path().join("implementation.rs"), "changed\n").unwrap();
    let relative_signal_path = signal_path.strip_prefix(root.path()).unwrap().to_str().unwrap();
    run_git(root.path(), &["add", relative_signal_path, "implementation.rs"]);
    run_git(root.path(), &["commit", "--no-gpg-sign", "-m", "implementation and signals"]);

    let occurrences = SystemSignalReportSourceAdapter::read_impl_catalog(root.path(), &track)
        .expect("a regenerated signal in the guarded commit must be accepted");
    assert!(matches!(
        occurrences.as_slice(),
        [SignalReportOccurrence {
            chain: SignalReportChain::ImplCatalog,
            level: SignalReportLevel::Yellow,
            ..
        }]
    ));
}

#[test]
fn test_signal_report_adapter_rejects_stale_worktree_copy_over_fresh_commit() {
    let (root, track, signal_path, catalogue) = setup_committed_impl_signal_fixture();
    let stale =
        fresh_impl_catalog_signals_with_timestamp(root.path(), &catalogue, "2026-08-01T00:00:00Z");
    let fresh =
        fresh_impl_catalog_signals_with_timestamp(root.path(), &catalogue, "2026-08-02T00:00:00Z");
    fs::write(&signal_path, fresh).unwrap();
    fs::write(root.path().join("implementation.rs"), "changed\n").unwrap();
    let relative_signal_path = signal_path.strip_prefix(root.path()).unwrap().to_str().unwrap();
    run_git(root.path(), &["add", relative_signal_path, "implementation.rs"]);
    run_git(root.path(), &["commit", "--no-gpg-sign", "-m", "fresh signals"]);
    fs::write(&signal_path, stale).unwrap();

    let error = SystemSignalReportSourceAdapter::read_impl_catalog(root.path(), &track)
        .expect_err("a stale worktree copy must not ride on a fresh committed signal");
    assert!(matches!(error, SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)));
}

#[test]
fn test_signal_report_adapter_rejects_signal_deleted_at_tip() {
    let (root, track, signal_path, catalogue) = setup_committed_impl_signal_fixture();
    let worktree_copy =
        fresh_impl_catalog_signals_with_timestamp(root.path(), &catalogue, "2026-08-03T00:00:00Z");
    let relative_signal_path =
        signal_path.strip_prefix(root.path()).unwrap().to_str().unwrap().to_owned();
    fs::remove_file(&signal_path).unwrap();
    run_git(root.path(), &["add", "-u", &relative_signal_path]);
    run_git(root.path(), &["commit", "--no-gpg-sign", "-m", "delete signals"]);
    fs::write(&signal_path, worktree_copy).unwrap();

    let error = SystemSignalReportSourceAdapter::read_impl_catalog(root.path(), &track)
        .expect_err("a deleted committed signal must not be accepted from the worktree");
    assert!(matches!(error, SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)));
}

#[cfg(unix)]
#[test]
fn test_signal_report_freshness_rejects_mode_only_signal_change() {
    use std::os::unix::fs::PermissionsExt;

    let (root, _track, signal_path, _catalogue) = setup_committed_impl_signal_fixture();
    let worktree_bytes = fs::read(&signal_path).unwrap();
    let relative_signal_path =
        signal_path.strip_prefix(root.path()).unwrap().to_str().unwrap().to_owned();
    run_git(root.path(), &["config", "core.filemode", "true"]);
    let mut permissions = fs::metadata(&signal_path).unwrap().permissions();
    permissions.set_mode(permissions.mode() | 0o111);
    fs::set_permissions(&signal_path, permissions).unwrap();
    run_git(root.path(), &["add", &relative_signal_path]);
    run_git(root.path(), &["commit", "--no-gpg-sign", "-m", "mode-only signal change"]);

    let recorded_head =
        CommitHash::try_new(git_output(root.path(), &["rev-parse", "HEAD^1"]).trim().to_owned())
            .unwrap();
    let error =
        validate_impl_catalog_freshness(root.path(), &signal_path, &recorded_head, &worktree_bytes)
            .expect_err("a mode-only signal commit must not satisfy freshness");
    assert!(matches!(error, SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)));
}

#[test]
fn test_signal_report_adapter_accepts_type_baseline_above_capability_exec_limit() {
    let root = tempfile::tempdir().unwrap();
    let track = TrackId::try_new("report-test").unwrap();
    let track_dir = root.path().join("track/items/report-test");
    let catalogue = prepare_catalogue_spec_source(root.path(), &track_dir);
    let baseline = vec![b'b'; crate::capability_exec::MAX_CAPABILITY_EXEC_TEXT_BYTES as usize + 1];
    fs::write(track_dir.join("infrastructure-types-baseline.json"), &baseline).unwrap();

    let mut signals: serde_json::Value =
        serde_json::from_str(&fresh_impl_catalog_signals(root.path(), &catalogue)).unwrap();
    signal_document_object(&mut signals).insert(
        "baseline_hash".to_owned(),
        serde_json::json!(type_signals_codec::baseline_hash(&baseline).as_digest().as_str()),
    );
    fs::write(
        track_dir.join("infrastructure-type-signals.json"),
        serde_json::to_string(&signals).unwrap(),
    )
    .unwrap();

    let occurrences =
        SystemSignalReportSourceAdapter::read_impl_catalog(root.path(), &track).unwrap();

    assert!(matches!(
        occurrences.as_slice(),
        [SignalReportOccurrence {
            chain: SignalReportChain::ImplCatalog,
            level: SignalReportLevel::Yellow,
            ..
        }]
    ));
}

#[test]
fn test_relative_path_outside_root_reports_selected_non_adr_chain() {
    let root = tempfile::tempdir().unwrap();
    let outside = root.path().parent().unwrap().join("outside.json");

    let error = relative(root.path(), &outside, SignalReportChain::SpecAdr)
        .expect_err("an out-of-root path must retain the selected chain");

    assert!(matches!(error, SignalReportError::SourceUnavailable(SignalReportChain::SpecAdr)));
}

#[cfg(unix)]
#[test]
fn test_relative_non_utf8_path_reports_selected_non_adr_chain() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(OsString::from_vec(vec![0xff]));

    let error = relative(root.path(), &path, SignalReportChain::ImplCatalog)
        .expect_err("a non-UTF-8 path must retain the selected chain");

    assert!(matches!(error, SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)));
}

#[test]
fn test_signal_report_adapter_skips_catalogue_spec_layers_without_signal_activation() {
    let root = tempfile::tempdir().unwrap();
    let track = TrackId::try_new("report-test").unwrap();
    fs::write(
            root.path().join("architecture-rules.json"),
            r#"{"version":2,"layers":[{"crate":"infrastructure","path":"libs/infrastructure","may_depend_on":[],"tddd":{"enabled":true}}]}"#,
        )
        .unwrap();

    let occurrences =
        SystemSignalReportSourceAdapter::read_catalog_spec(root.path(), &track).unwrap();

    assert!(occurrences.is_empty());
}

#[test]
fn test_signal_report_adapter_rejects_excessive_adr_input_count() {
    let root = tempfile::tempdir().unwrap();
    let adr_dir = root.path().join("knowledge/adr");
    fs::create_dir_all(&adr_dir).unwrap();
    for index in 0..=MAX_ADR_FILES {
        fs::write(
            adr_dir.join(format!("{index}.md")),
            "---\nadr_id: report-source\ndecisions: []\n---\n",
        )
        .unwrap();
    }

    let error = SystemSignalReportSourceAdapter::derive_adr_user(root.path())
        .expect_err("ADR input count beyond the aggregate ceiling must be rejected");

    assert!(matches!(error, SignalReportError::SourceUnavailable(SignalReportChain::AdrUser)));
}

#[test]
fn test_bounded_adr_paths_rejects_excessive_junk_entries() {
    let root = tempfile::tempdir().expect("fixture root must be created");
    let adr_dir = root.path().join("knowledge/adr");
    fs::create_dir_all(&adr_dir).expect("ADR fixture directory must exist");
    for index in 0..=MAX_ADR_FILES {
        fs::write(adr_dir.join(format!("junk-{index}.txt")), "junk")
            .expect("junk fixture must be written");
    }

    let error = bounded_adr_paths(&adr_dir)
        .expect_err("directory entries beyond the ceiling must be rejected before filtering");

    assert!(matches!(error, SignalReportError::SourceUnavailable(SignalReportChain::AdrUser)));
}

#[test]
fn test_signal_report_adapter_rejects_excessive_adr_input_bytes() {
    let root = tempfile::tempdir().unwrap();
    let adr_dir = root.path().join("knowledge/adr");
    fs::create_dir_all(&adr_dir).unwrap();
    let content = format!(
        "---\nadr_id: report-source\ndecisions: []\n---\n{}",
        "x".repeat((MAX_ADR_TOTAL_BYTES / 2) + 1)
    );
    fs::write(adr_dir.join("one.md"), &content).unwrap();
    fs::write(adr_dir.join("two.md"), content).unwrap();

    let error = SystemSignalReportSourceAdapter::derive_adr_user(root.path())
        .expect_err("ADR input bytes beyond the aggregate ceiling must be rejected");

    assert!(matches!(error, SignalReportError::SourceUnavailable(SignalReportChain::AdrUser)));
}

#[test]
fn test_signal_report_source_port_load_reads_persisted_artifact_occurrences() {
    let fixture = seed_report_source_repo();
    let _lock = CWD_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let _cwd = CwdGuard::enter(fixture.path());
    let adapter = SystemSignalReportSourceAdapter::new();

    let catalogue = SignalReportSourcePort::load(&adapter, SignalReportChain::CatalogSpec)
        .expect("public port must read the persisted catalogue signal artifact");
    assert!(matches!(
        catalogue.as_slice(),
        [SignalReportOccurrence {
            chain: SignalReportChain::CatalogSpec,
            level: SignalReportLevel::Yellow,
            entry_id,
            reference,
            reason,
            location,
        }] if entry_id.to_string()
            == "infrastructure:types:SystemSignalReportSourceAdapter"
            && reference.to_string()
                == "track/items/signal-report-command-2026-07-31/spec.json#IN-04"
            && reason.to_string() == "persisted catalogue-spec signal is non-blue"
            && location.to_string() == "track/items/report-test/infrastructure-types.json"
    ));

    let implementation = SignalReportSourcePort::load(&adapter, SignalReportChain::ImplCatalog)
        .expect("public port must read the persisted implementation signal artifact");
    assert!(matches!(
        implementation.as_slice(),
        [SignalReportOccurrence {
            chain: SignalReportChain::ImplCatalog,
            level: SignalReportLevel::Yellow,
            entry_id,
            reference,
            reason,
            location,
        }] if entry_id.to_string()
            == "infrastructure:secondary_adapter:type:SystemSignalReportSourceAdapter"
            && reference.to_string()
                == "infrastructure-types.json#secondary_adapter:type:SystemSignalReportSourceAdapter"
            && reason.to_string() == "implementation does not conform to catalogue declaration"
            && location.to_string() == "track/items/report-test/infrastructure-type-signals.json"
    ));

    let signals_path =
        fixture.path().join("track/items/report-test/infrastructure-type-signals.json");
    let mut signals: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&signals_path).unwrap()).unwrap();
    signal_document_object(&mut signals)
        .insert("declaration_hash".to_owned(), serde_json::json!("a".repeat(64)));
    fs::write(&signals_path, serde_json::to_string(&signals).unwrap()).unwrap();

    let error = SignalReportSourcePort::load(&adapter, SignalReportChain::ImplCatalog)
        .expect_err("a stale implementation-catalogue declaration hash must fail closed");
    assert!(matches!(error, SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)));

    let catalogue_path = fixture.path().join("track/items/report-test/infrastructure-types.json");
    let malformed_catalogue = "{malformed catalogue";
    fs::write(&catalogue_path, malformed_catalogue).unwrap();
    fs::write(&signals_path, fresh_impl_catalog_signals(fixture.path(), malformed_catalogue))
        .unwrap();

    let error = SignalReportSourcePort::load(&adapter, SignalReportChain::ImplCatalog)
        .expect_err("a malformed current catalogue must fail closed even with a matching hash");
    assert!(matches!(error, SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)));
}

#[test]
fn test_signal_report_source_port_load_derives_nonpersisted_chain_occurrences() {
    let fixture = seed_report_source_repo();
    let _lock = CWD_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let _cwd = CwdGuard::enter(fixture.path());
    let adapter = SystemSignalReportSourceAdapter::new();

    let adr_user = SignalReportSourcePort::load(&adapter, SignalReportChain::AdrUser)
        .expect("public port must derive the ADR-user chain at report time");
    assert!(matches!(
        adr_user.as_slice(),
        [SignalReportOccurrence {
            chain: SignalReportChain::AdrUser,
            level: SignalReportLevel::Yellow,
            entry_id,
            reference,
            reason,
            location,
        }] if entry_id.to_string() == "report-source#D1"
            && reference.to_string() == "review:123"
            && reason.to_string() == "decision remains grounded by a review finding"
            && location.to_string() == "knowledge/adr/report-source.md"
    ));

    let spec_adr = SignalReportSourcePort::load(&adapter, SignalReportChain::SpecAdr)
        .expect("public port must derive the spec-ADR chain at report time");
    assert!(spec_adr.iter().any(|occurrence| {
        occurrence.chain == SignalReportChain::SpecAdr
            && occurrence.level == SignalReportLevel::Red
            && occurrence.entry_id.to_string() == "IN-01"
            && occurrence.reference.to_string() == "no ADR reference"
            && occurrence.reason.to_string()
                == "requirement has neither ADR references nor informal grounds"
            && occurrence.location.to_string() == "track/items/report-test/spec.json"
    }));
}

#[test]
fn test_signal_report_source_port_load_rejects_missing_impl_catalog_artifact() {
    let fixture = seed_report_source_repo();
    fs::remove_file(
        fixture.path().join("track/items/report-test/infrastructure-type-signals.json"),
    )
    .unwrap();
    let _lock = CWD_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let _cwd = CwdGuard::enter(fixture.path());
    let adapter = SystemSignalReportSourceAdapter::new();

    let error = SignalReportSourcePort::load(&adapter, SignalReportChain::ImplCatalog)
        .expect_err("missing signal artifact must fail the requested chain");

    assert!(matches!(error, SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)));
}

#[test]
fn test_signal_report_source_port_load_maps_context_error_to_requested_chain() {
    let fixture = tempfile::tempdir().unwrap();
    let _lock = CWD_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let _cwd = CwdGuard::enter(fixture.path());
    let adapter = SystemSignalReportSourceAdapter::new();

    let error = SignalReportSourcePort::load(&adapter, SignalReportChain::CatalogSpec)
        .expect_err("a missing repository context must fail the requested chain");

    assert!(matches!(error, SignalReportError::SourceUnavailable(SignalReportChain::CatalogSpec)));
}

#[test]
fn test_signal_report_source_port_load_is_read_only_across_all_chains() {
    let fixture = seed_report_source_repo();
    let before = snapshot_tree(fixture.path());
    let _lock = CWD_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let _cwd = CwdGuard::enter(fixture.path());
    let adapter = SystemSignalReportSourceAdapter::new();

    for chain in [
        SignalReportChain::AdrUser,
        SignalReportChain::SpecAdr,
        SignalReportChain::CatalogSpec,
        SignalReportChain::ImplCatalog,
    ] {
        SignalReportSourcePort::load(&adapter, chain)
            .expect("full public report load must succeed without writing artifacts");
    }

    assert_eq!(
        snapshot_tree(fixture.path()),
        before,
        "report loading must neither alter inputs nor create derived occurrence, signal, or aggregate artifacts"
    );
}
