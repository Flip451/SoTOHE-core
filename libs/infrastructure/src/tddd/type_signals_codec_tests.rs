use domain::ConfidenceSignal;
use domain::tddd::catalogue_v2::identifiers::CatalogueItemNamespace;
use domain::tddd::type_signals_doc::{
    TypeSignalsAuthorityStatus, TypeSignalsReuseDecision, TypeSignalsReuseInput,
    TypeSignalsWorktreeStatus, decide_type_signals_reuse,
};

use super::*;

const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

struct PropertyGenerator {
    state: u64,
}

impl PropertyGenerator {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        (self.state >> 32) as u32
    }
}

#[derive(Debug, Clone, Copy)]
enum InvalidExecutionIdentityComponent {
    TargetDirectory,
    ExpectedJsonPath,
    CrateName,
    Feature,
    Profile,
}

impl InvalidExecutionIdentityComponent {
    fn from_case(case: u32) -> Self {
        match case % 5 {
            0 => Self::TargetDirectory,
            1 => Self::ExpectedJsonPath,
            2 => Self::CrateName,
            3 => Self::Feature,
            _ => Self::Profile,
        }
    }
}

fn valid_type_signals_doc_dto() -> TypeSignalsDocDto {
    TypeSignalsDocDto {
        schema_version: domain::TYPE_SIGNALS_SCHEMA_VERSION,
        generated_at: "2026-04-18T12:00:00Z".to_owned(),
        declaration_hash: DIGEST.to_owned(),
        head_commit: "a".repeat(40),
        baseline_hash: DIGEST.to_owned(),
        implementation_fingerprint: DIGEST.to_owned(),
        resolution_fingerprint: DIGEST.to_owned(),
        rustdoc_execution_identity: test_execution_identity_dto(),
        signals: vec![],
    }
}

fn set_generated_invalid_execution_identity_component(
    payload: &mut serde_json::Value,
    component: InvalidExecutionIdentityComponent,
    case: u32,
    variation: u32,
) {
    let identity = payload
        .get_mut("rustdoc_execution_identity")
        .and_then(serde_json::Value::as_object_mut)
        .expect("the valid fixture must contain an execution identity object");
    let value = match component {
        InvalidExecutionIdentityComponent::TargetDirectory => match variation % 3 {
            0 => format!("/tmp/other-target-{case}"),
            1 => format!("../other-target-{case}"),
            _ => format!("/tmp/sotohe-codec-test-target/../other-target-{case}"),
        },
        InvalidExecutionIdentityComponent::ExpectedJsonPath => {
            if variation % 2 == 0 {
                format!("/tmp/other-target-{case}/domain.json")
            } else {
                format!("/tmp/sotohe-codec-test-target/../other-{case}.json")
            }
        }
        InvalidExecutionIdentityComponent::CrateName => match variation % 3 {
            0 => String::new(),
            1 => format!("crate-name-{case}"),
            _ => format!("{case}crate"),
        },
        InvalidExecutionIdentityComponent::Feature => match variation % 3 {
            0 => String::new(),
            1 => format!("-feature-{case}"),
            _ => format!("feature/{case}"),
        },
        InvalidExecutionIdentityComponent::Profile => match variation % 3 {
            0 => String::new(),
            1 => " ".repeat((case % 3 + 1) as usize),
            _ => "\t".repeat((case % 3 + 1) as usize),
        },
    };
    match component {
        InvalidExecutionIdentityComponent::TargetDirectory => {
            identity.insert("target_directory".to_owned(), serde_json::Value::String(value));
        }
        InvalidExecutionIdentityComponent::ExpectedJsonPath => {
            identity.insert("expected_json_path".to_owned(), serde_json::Value::String(value));
        }
        InvalidExecutionIdentityComponent::CrateName => {
            identity.insert("crate_name".to_owned(), serde_json::Value::String(value));
        }
        InvalidExecutionIdentityComponent::Feature => {
            identity.insert(
                "features".to_owned(),
                serde_json::Value::Array(vec![serde_json::Value::String(value)]),
            );
        }
        InvalidExecutionIdentityComponent::Profile => {
            identity.insert("profile".to_owned(), serde_json::Value::String(value));
        }
    }
}

fn legacy_cache_key(
    declaration_hash: domain::CatalogueDeclarationHash,
    head_commit: domain::CommitHash,
    baseline_hash: domain::BaselineHash,
) -> domain::TypeSignalsCacheKey {
    let target = domain::ResolvedCargoTargetDirectory::try_new(std::path::PathBuf::from(
        "/tmp/sotohe-codec-test-target",
    ))
    .unwrap();
    let expected =
        domain::ExpectedRustdocJsonPath::try_new(target.as_path().join("doc/legacy.json"), &target)
            .unwrap();
    let identity = domain::RustdocExecutionIdentity::new(
        target,
        domain::tddd::catalogue_v2::CrateName::new("legacy").unwrap(),
        vec![],
        domain::CargoProfileName::try_new("dev".to_owned()).unwrap(),
        expected,
    )
    .unwrap();
    let zero = domain::Sha256Digest::try_new("0".repeat(64)).unwrap();
    domain::TypeSignalsCacheKey::new(
        declaration_hash,
        head_commit,
        baseline_hash,
        domain::ImplementationFingerprint::new(zero.clone()),
        domain::ResolutionFingerprint::new(zero),
        identity,
    )
}

fn sample_doc() -> TypeSignalsDocument {
    let digest = Sha256Digest::try_new(DIGEST.to_owned()).unwrap();
    TypeSignalsDocument::new(
        Timestamp::new("2026-04-18T12:00:00Z").unwrap(),
        legacy_cache_key(
            CatalogueDeclarationHash::new(digest.clone()),
            CommitHash::try_new("a".repeat(40)).unwrap(),
            BaselineHash::new(digest),
        ),
        vec![TypeSignal::new(
            ThreeWaySignalIdentity::Label { label: FreeText::new("Example") },
            "free_function".to_owned(),
            ConfidenceSignal::Blue,
            true,
            vec!["field".to_owned()],
            vec![],
            vec!["unexpected".to_owned()],
        )],
    )
}

fn sample_catalogue_item_doc() -> TypeSignalsDocument {
    let digest = Sha256Digest::try_new(DIGEST.to_owned()).unwrap();
    TypeSignalsDocument::new(
        Timestamp::new("2026-04-18T12:00:00Z").unwrap(),
        legacy_cache_key(
            CatalogueDeclarationHash::new(digest.clone()),
            CommitHash::try_new("a".repeat(40)).unwrap(),
            BaselineHash::new(digest),
        ),
        vec![
            TypeSignal::new(
                ThreeWaySignalIdentity::CatalogueItem {
                    item_name: FreeText::new("Shared"),
                    namespace: CatalogueItemNamespace::Type,
                },
                "value_object".to_owned(),
                ConfidenceSignal::Blue,
                true,
                vec![],
                vec![],
                vec![],
            ),
            TypeSignal::new(
                ThreeWaySignalIdentity::CatalogueItem {
                    item_name: FreeText::new("Shared"),
                    namespace: CatalogueItemNamespace::Trait,
                },
                "secondary_port".to_owned(),
                ConfidenceSignal::Yellow,
                false,
                vec![],
                vec!["method".to_owned()],
                vec![],
            ),
        ],
    )
}

fn sample_identity_collision_doc() -> TypeSignalsDocument {
    let digest = Sha256Digest::try_new(DIGEST.to_owned()).unwrap();
    TypeSignalsDocument::new(
        Timestamp::new("2026-04-18T12:00:00Z").unwrap(),
        legacy_cache_key(
            CatalogueDeclarationHash::new(digest.clone()),
            CommitHash::try_new("a".repeat(40)).unwrap(),
            BaselineHash::new(digest),
        ),
        vec![
            TypeSignal::new(
                ThreeWaySignalIdentity::CatalogueItem {
                    item_name: FreeText::new("Shared"),
                    namespace: CatalogueItemNamespace::Type,
                },
                "value_object".to_owned(),
                ConfidenceSignal::Blue,
                true,
                vec![],
                vec![],
                vec![],
            ),
            TypeSignal::new(
                ThreeWaySignalIdentity::CatalogueItem {
                    item_name: FreeText::new("Shared"),
                    namespace: CatalogueItemNamespace::Trait,
                },
                "secondary_port".to_owned(),
                ConfidenceSignal::Red,
                false,
                vec![],
                vec!["required_method".to_owned()],
                vec![],
            ),
            TypeSignal::new(
                ThreeWaySignalIdentity::Label { label: FreeText::new("Shared") },
                "free_function".to_owned(),
                ConfidenceSignal::Yellow,
                true,
                vec![],
                vec![],
                vec!["report-only".to_owned()],
            ),
        ],
    )
}

#[test]
fn test_encode_decode_roundtrip_preserves_document() {
    let document = sample_doc();
    assert_eq!(decode(&encode(&document).unwrap()).unwrap(), document);
}

#[test]
fn test_workspace_relative_identity_roundtrip_uses_active_workspace() {
    let workspace = std::path::Path::new("/tmp/sotohe-codec-workspace");
    let target = domain::ResolvedCargoTargetDirectory::try_new(workspace.join("target")).unwrap();
    let expected =
        domain::ExpectedRustdocJsonPath::try_new(target.as_path().join("doc/domain.json"), &target)
            .unwrap();
    let identity = domain::RustdocExecutionIdentity::new(
        target,
        domain::tddd::catalogue_v2::CrateName::new("domain").unwrap(),
        vec![],
        domain::CargoProfileName::try_new("dev".to_owned()).unwrap(),
        expected,
    )
    .unwrap();
    let digest = Sha256Digest::try_new(DIGEST.to_owned()).unwrap();
    let document = TypeSignalsDocument::new(
        Timestamp::new("2026-04-18T12:00:00Z").unwrap(),
        domain::TypeSignalsCacheKey::new(
            CatalogueDeclarationHash::new(digest.clone()),
            CommitHash::try_new("a".repeat(40)).unwrap(),
            BaselineHash::new(digest),
            domain::ImplementationFingerprint::new(
                Sha256Digest::try_new(DIGEST.to_owned()).unwrap(),
            ),
            domain::ResolutionFingerprint::new(Sha256Digest::try_new(DIGEST.to_owned()).unwrap()),
            identity,
        ),
        vec![],
    );

    let encoded = encode_with_workspace(&document, Some(workspace)).unwrap();
    assert!(encoded.contains("\"target_directory\": \"target\""));
    assert!(encoded.contains("\"expected_json_path\": \"target/doc/domain.json\""));
    assert_eq!(
        decode_with_workspace_for_current(
            &encoded,
            workspace,
            document.cache_key().rustdoc_execution_identity(),
        )
        .unwrap(),
        document
    );
}

#[test]
fn test_external_identity_roundtrip_uses_portable_marker_and_current_target() {
    let workspace = std::path::Path::new("/tmp/sotohe-codec-workspace");
    let target = domain::ResolvedCargoTargetDirectory::try_new(std::path::PathBuf::from(
        "/home/alice/.cache/cargo-target",
    ))
    .unwrap();
    let expected =
        domain::ExpectedRustdocJsonPath::try_new(target.as_path().join("doc/domain.json"), &target)
            .unwrap();
    let identity = domain::RustdocExecutionIdentity::new(
        target,
        domain::tddd::catalogue_v2::CrateName::new("domain").unwrap(),
        vec![],
        domain::CargoProfileName::try_new("dev".to_owned()).unwrap(),
        expected,
    )
    .unwrap();
    let digest = Sha256Digest::try_new(DIGEST.to_owned()).unwrap();
    let document = TypeSignalsDocument::new(
        Timestamp::new("2026-04-18T12:00:00Z").unwrap(),
        domain::TypeSignalsCacheKey::new(
            CatalogueDeclarationHash::new(digest.clone()),
            CommitHash::try_new("a".repeat(40)).unwrap(),
            BaselineHash::new(digest),
            domain::ImplementationFingerprint::new(
                Sha256Digest::try_new(DIGEST.to_owned()).unwrap(),
            ),
            domain::ResolutionFingerprint::new(Sha256Digest::try_new(DIGEST.to_owned()).unwrap()),
            identity,
        ),
        vec![],
    );

    let encoded = encode_with_workspace(&document, Some(workspace)).unwrap();
    assert!(!encoded.contains("/home/alice/"));
    assert!(encoded.contains(EXTERNAL_TARGET_IDENTITY_ROOT));
    assert_eq!(
        decode_with_workspace_for_current(
            &encoded,
            workspace,
            document.cache_key().rustdoc_execution_identity(),
        )
        .unwrap(),
        document
    );
}

#[test]
fn test_encode_decode_roundtrip_preserves_catalogue_item_namespaces() {
    let document = sample_catalogue_item_doc();

    let encoded = encode(&document).unwrap();
    assert!(encoded.contains("\"namespace\": \"type\""));
    assert!(encoded.contains("\"namespace\": \"trait\""));

    let decoded = decode(&encoded).unwrap();
    assert_eq!(decoded, document);
    let namespaces =
        decoded.signals().iter().map(|signal| signal.identity().namespace()).collect::<Vec<_>>();
    assert_eq!(
        namespaces,
        vec![Some(CatalogueItemNamespace::Type), Some(CatalogueItemNamespace::Trait)]
    );
    let signals = decoded.signals().iter().map(TypeSignal::signal).collect::<Vec<_>>();
    assert_eq!(signals, vec![ConfidenceSignal::Blue, ConfidenceSignal::Yellow]);
}

#[test]
fn test_encode_decode_roundtrip_keeps_same_named_type_trait_and_label_statuses() {
    let document = sample_identity_collision_doc();

    let encoded = encode(&document).unwrap();
    let payload: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    let rows = payload["signals"].as_array().unwrap();
    assert_eq!(rows[0]["namespace"], serde_json::json!("type"));
    assert_eq!(rows[1]["namespace"], serde_json::json!("trait"));
    assert!(rows[2]["namespace"].is_null(), "report labels must persist null namespace");

    let decoded = decode(&encoded).unwrap();
    assert_eq!(decoded, document);
    assert_eq!(
        decoded.signals().iter().map(TypeSignal::signal).collect::<Vec<_>>(),
        vec![ConfidenceSignal::Blue, ConfidenceSignal::Red, ConfidenceSignal::Yellow]
    );
    assert_eq!(
        decoded.signals().iter().map(|signal| signal.identity().namespace()).collect::<Vec<_>>(),
        vec![Some(CatalogueItemNamespace::Type), Some(CatalogueItemNamespace::Trait), None,]
    );
}

#[test]
fn test_encode_rejects_catalogue_kind_without_namespace() {
    let digest = Sha256Digest::try_new(DIGEST.to_owned()).unwrap();
    let document = TypeSignalsDocument::new(
        Timestamp::new("2026-04-18T12:00:00Z").unwrap(),
        legacy_cache_key(
            CatalogueDeclarationHash::new(digest.clone()),
            CommitHash::try_new("a".repeat(40)).unwrap(),
            BaselineHash::new(digest),
        ),
        vec![TypeSignal::new(
            ThreeWaySignalIdentity::Label { label: FreeText::new("Shared") },
            "value_object".to_owned(),
            ConfidenceSignal::Blue,
            true,
            vec![],
            vec![],
            vec![],
        )],
    );

    assert!(matches!(encode(&document), Err(TypeSignalsCodecError::InvalidNamespace(_))));
}

#[test]
fn test_encode_canonicalizes_json_keys_and_is_byte_stable() {
    let document = sample_doc();

    let first = encode(&document).unwrap();
    let second = encode(&document).unwrap();

    assert_eq!(first, second, "type-signal encoding must not churn JSON bytes");
    assert!(
        first.starts_with("{\n  \"baseline_hash\":"),
        "type-signal keys must be canonicalized: {first}"
    );
    let signal = &first[first.find("\"extra_items\"").unwrap()..];
    let extra_items = signal.find("\"extra_items\"").unwrap();
    let found_items = signal.find("\"found_items\"").unwrap();
    let found_type = signal.find("\"found_type\"").unwrap();
    let kind_tag = signal.find("\"kind_tag\"").unwrap();
    let namespace = signal.find("\"namespace\"").unwrap();
    let signal_key = signal.find("\"signal\"").unwrap();
    let type_name = signal.find("\"type_name\"").unwrap();
    assert!(
        extra_items < found_items
            && found_items < found_type
            && found_type < kind_tag
            && kind_tag < namespace
            && kind_tag < signal_key
            && namespace < signal_key
            && signal_key < type_name,
        "type-signal objects must be recursively canonicalized: {signal}"
    );
}

#[test]
fn test_declaration_hash_changes_when_declaration_bytes_change() {
    assert_eq!(
        declaration_hash(b"type A = u8;").as_digest().as_str(),
        "bdc16928bf7d4bbd73c69b65d1d3e4e644225f9c3322d589f1a63b5f37af0592"
    );
    assert_ne!(declaration_hash(b"type A = u8;"), declaration_hash(b"type A = u16;"));
}

#[test]
fn test_decode_rejects_unknown_fields() {
    let mut payload = serde_json::to_value(&TypeSignalsDocDto {
        schema_version: domain::TYPE_SIGNALS_SCHEMA_VERSION,
        generated_at: "2026-04-18T12:00:00Z".to_owned(),
        declaration_hash: DIGEST.to_owned(),
        head_commit: "a".repeat(40),
        baseline_hash: DIGEST.to_owned(),
        implementation_fingerprint: DIGEST.to_owned(),
        resolution_fingerprint: DIGEST.to_owned(),
        rustdoc_execution_identity: test_execution_identity_dto(),
        signals: vec![],
    })
    .unwrap();
    payload
        .as_object_mut()
        .unwrap()
        .insert("unknown_field".to_owned(), serde_json::Value::String(DIGEST.to_owned()));
    assert!(matches!(decode(&payload.to_string()), Err(TypeSignalsCodecError::Json(_))));
}

#[test]
fn test_decode_requires_head_commit() {
    let mut payload = serde_json::to_value(&TypeSignalsDocDto {
        schema_version: domain::TYPE_SIGNALS_SCHEMA_VERSION,
        generated_at: "2026-04-18T12:00:00Z".to_owned(),
        declaration_hash: DIGEST.to_owned(),
        head_commit: "a".repeat(40),
        baseline_hash: DIGEST.to_owned(),
        implementation_fingerprint: DIGEST.to_owned(),
        resolution_fingerprint: DIGEST.to_owned(),
        rustdoc_execution_identity: test_execution_identity_dto(),
        signals: vec![],
    })
    .unwrap();
    payload.as_object_mut().unwrap().remove("head_commit");
    assert!(matches!(decode(&payload.to_string()), Err(TypeSignalsCodecError::Json(_))));
}

#[test]
fn test_decode_requires_baseline_hash() {
    let mut payload = serde_json::to_value(&TypeSignalsDocDto {
        schema_version: domain::TYPE_SIGNALS_SCHEMA_VERSION,
        generated_at: "2026-04-18T12:00:00Z".to_owned(),
        declaration_hash: DIGEST.to_owned(),
        head_commit: "a".repeat(40),
        baseline_hash: DIGEST.to_owned(),
        implementation_fingerprint: DIGEST.to_owned(),
        resolution_fingerprint: DIGEST.to_owned(),
        rustdoc_execution_identity: test_execution_identity_dto(),
        signals: vec![],
    })
    .unwrap();
    payload.as_object_mut().unwrap().remove("baseline_hash");
    assert!(matches!(decode(&payload.to_string()), Err(TypeSignalsCodecError::Json(_))));
}

#[test]
fn test_decode_rejects_unknown_confidence_signal_value() {
    let mut payload = serde_json::to_value(&TypeSignalsDocDto {
        schema_version: domain::TYPE_SIGNALS_SCHEMA_VERSION,
        generated_at: "2026-04-18T12:00:00Z".to_owned(),
        declaration_hash: DIGEST.to_owned(),
        head_commit: "a".repeat(40),
        baseline_hash: DIGEST.to_owned(),
        implementation_fingerprint: DIGEST.to_owned(),
        resolution_fingerprint: DIGEST.to_owned(),
        rustdoc_execution_identity: test_execution_identity_dto(),
        signals: vec![signal_to_dto(&TypeSignal::new(
            ThreeWaySignalIdentity::Label { label: FreeText::new("Example") },
            "free_function".to_owned(),
            ConfidenceSignal::Blue,
            true,
            vec![],
            vec![],
            vec![],
        ))],
    })
    .unwrap();
    *payload.pointer_mut("/signals/0/signal").unwrap() =
        serde_json::Value::String("bogus".to_owned());
    assert!(
        matches!(decode(&payload.to_string()), Err(TypeSignalsCodecError::InvalidSignal(_))),
        "an unknown signal value must fail the decode instead of defaulting"
    );
}

#[test]
fn test_decode_rejects_unknown_catalogue_item_namespace() {
    let mut payload = serde_json::to_value(&TypeSignalsDocDto {
        schema_version: domain::TYPE_SIGNALS_SCHEMA_VERSION,
        generated_at: "2026-04-18T12:00:00Z".to_owned(),
        declaration_hash: DIGEST.to_owned(),
        head_commit: "a".repeat(40),
        baseline_hash: DIGEST.to_owned(),
        implementation_fingerprint: DIGEST.to_owned(),
        resolution_fingerprint: DIGEST.to_owned(),
        rustdoc_execution_identity: test_execution_identity_dto(),
        signals: vec![signal_to_dto(&TypeSignal::new(
            ThreeWaySignalIdentity::CatalogueItem {
                item_name: FreeText::new("Shared"),
                namespace: CatalogueItemNamespace::Type,
            },
            "value_object".to_owned(),
            ConfidenceSignal::Blue,
            true,
            vec![],
            vec![],
            vec![],
        ))],
    })
    .unwrap();
    *payload.pointer_mut("/signals/0/namespace").unwrap() =
        serde_json::Value::String("module".to_owned());

    assert!(matches!(decode(&payload.to_string()), Err(TypeSignalsCodecError::Json(_))));
}

#[test]
fn test_decode_rejects_current_signal_without_explicit_identity() {
    let payload = serde_json::json!({
        "schema_version": domain::TYPE_SIGNALS_SCHEMA_VERSION,
        "generated_at": "2026-04-18T12:00:00Z",
        "declaration_hash": DIGEST,
        "head_commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "baseline_hash": DIGEST,
        "implementation_fingerprint": DIGEST,
        "resolution_fingerprint": DIGEST,
        "rustdoc_execution_identity": test_execution_identity_dto(),
        "signals": [{
            "type_name": "Shared",
            "kind_tag": "value_object",
            "signal": "blue",
            "found_type": true
        }]
    });

    let result = decode(&payload.to_string());
    assert!(
        matches!(result, Err(TypeSignalsCodecError::InvalidNamespace(_))),
        "the current schema must not infer a label from an omitted catalogue identity discriminator: {result:?}"
    );
}

#[test]
fn test_decode_v5_document_without_new_fields_returns_unsupported_schema_error() {
    let payload = serde_json::json!({
        "schema_version": 5,
        "generated_at": "2026-04-18T12:00:00Z",
        "declaration_hash": DIGEST,
        "head_commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "baseline_hash": DIGEST,
        "signals": []
    });

    let result = decode(&payload.to_string());
    assert!(
        matches!(
            result,
            Err(TypeSignalsCodecError::UnsupportedSchemaVersion(version)) if version.value() == 5
        ),
        "a v5 document must be a cache miss before v6 DTO decoding: {result:?}"
    );
}

#[test]
fn test_decode_accepts_missing_namespace_for_function_label() {
    let payload = serde_json::json!({
        "schema_version": domain::TYPE_SIGNALS_SCHEMA_VERSION,
        "generated_at": "2026-04-18T12:00:00Z",
        "declaration_hash": DIGEST,
        "head_commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "baseline_hash": DIGEST,
        "implementation_fingerprint": DIGEST,
        "resolution_fingerprint": DIGEST,
        "rustdoc_execution_identity": test_execution_identity_dto(),
        "signals": [{
            "type_name": "compute",
            "kind_tag": "free_function",
            "signal": "blue",
            "found_type": true
        }]
    });

    let document =
        decode(&payload.to_string()).expect("an omitted namespace identifies a label row");
    assert!(matches!(
        document.signals()[0].identity(),
        ThreeWaySignalIdentity::Label { label } if label.as_str() == "compute"
    ));
}

#[test]
fn test_decode_accepts_missing_namespace_for_unknown_label() {
    let payload = serde_json::json!({
        "schema_version": domain::TYPE_SIGNALS_SCHEMA_VERSION,
        "generated_at": "2026-04-18T12:00:00Z",
        "declaration_hash": DIGEST,
        "head_commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "baseline_hash": DIGEST,
        "implementation_fingerprint": DIGEST,
        "resolution_fingerprint": DIGEST,
        "rustdoc_execution_identity": test_execution_identity_dto(),
        "signals": [{
            "type_name": "report::label",
            "kind_tag": "unknown",
            "signal": "red",
            "found_type": true
        }]
    });

    let document =
        decode(&payload.to_string()).expect("an omitted namespace identifies a label row");
    assert!(matches!(
        document.signals()[0].identity(),
        ThreeWaySignalIdentity::Label { label } if label.as_str() == "report::label"
    ));
}

#[test]
fn test_decode_accepts_explicit_null_label_identity() {
    let payload = serde_json::json!({
        "schema_version": domain::TYPE_SIGNALS_SCHEMA_VERSION,
        "generated_at": "2026-04-18T12:00:00Z",
        "declaration_hash": DIGEST,
        "head_commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "baseline_hash": DIGEST,
        "implementation_fingerprint": DIGEST,
        "resolution_fingerprint": DIGEST,
        "rustdoc_execution_identity": test_execution_identity_dto(),
        "signals": [{
            "type_name": "Shared",
            "namespace": null,
            "kind_tag": "free_function",
            "signal": "blue",
            "found_type": true
        }]
    });

    let document = decode(&payload.to_string()).expect("explicit null must identify a label");
    assert!(matches!(
        document.signals()[0].identity(),
        ThreeWaySignalIdentity::Label { label } if label.as_str() == "Shared"
    ));
}

#[test]
fn test_decode_rejects_null_namespace_for_catalogue_kind_tag() {
    let payload = serde_json::json!({
        "schema_version": domain::TYPE_SIGNALS_SCHEMA_VERSION,
        "generated_at": "2026-04-18T12:00:00Z",
        "declaration_hash": DIGEST,
        "head_commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "baseline_hash": DIGEST,
        "implementation_fingerprint": DIGEST,
        "resolution_fingerprint": DIGEST,
        "rustdoc_execution_identity": test_execution_identity_dto(),
        "signals": [{
            "type_name": "Shared",
            "namespace": null,
            "kind_tag": "value_object",
            "signal": "blue",
            "found_type": true
        }]
    });

    assert!(matches!(
        decode(&payload.to_string()),
        Err(TypeSignalsCodecError::InvalidNamespace(_))
    ));
}

#[test]
fn test_decode_rejects_catalogue_namespace_for_function_label() {
    let payload = serde_json::json!({
        "schema_version": domain::TYPE_SIGNALS_SCHEMA_VERSION,
        "generated_at": "2026-04-18T12:00:00Z",
        "declaration_hash": DIGEST,
        "head_commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "baseline_hash": DIGEST,
        "implementation_fingerprint": DIGEST,
        "resolution_fingerprint": DIGEST,
        "rustdoc_execution_identity": test_execution_identity_dto(),
        "signals": [{
            "type_name": "compute",
            "namespace": "type",
            "kind_tag": "free_function",
            "signal": "blue",
            "found_type": true
        }]
    });

    assert!(matches!(
        decode(&payload.to_string()),
        Err(TypeSignalsCodecError::InvalidNamespace(_))
    ));
}

#[test]
fn test_decode_requires_declaration_hash() {
    let mut payload = serde_json::to_value(&TypeSignalsDocDto {
        schema_version: domain::TYPE_SIGNALS_SCHEMA_VERSION,
        generated_at: "2026-04-18T12:00:00Z".to_owned(),
        declaration_hash: DIGEST.to_owned(),
        head_commit: "a".repeat(40),
        baseline_hash: DIGEST.to_owned(),
        implementation_fingerprint: DIGEST.to_owned(),
        resolution_fingerprint: DIGEST.to_owned(),
        rustdoc_execution_identity: test_execution_identity_dto(),
        signals: vec![],
    })
    .unwrap();
    payload.as_object_mut().unwrap().remove("declaration_hash");
    assert!(matches!(decode(&payload.to_string()), Err(TypeSignalsCodecError::Json(_))));
}

#[test]
fn test_decode_rejects_each_typed_codec_error_variant() {
    let dto = TypeSignalsDocDto {
        schema_version: domain::TYPE_SIGNALS_SCHEMA_VERSION,
        generated_at: "2026-04-18T12:00:00Z".to_owned(),
        declaration_hash: DIGEST.to_owned(),
        head_commit: "a".repeat(40),
        baseline_hash: DIGEST.to_owned(),
        implementation_fingerprint: DIGEST.to_owned(),
        resolution_fingerprint: DIGEST.to_owned(),
        rustdoc_execution_identity: test_execution_identity_dto(),
        signals: vec![],
    };

    let zero_schema = TypeSignalsDocDto { schema_version: 0, ..dto.clone() };
    assert!(matches!(
        decode(&serde_json::to_string(&zero_schema).unwrap()),
        Err(TypeSignalsCodecError::InvalidSchemaVersion(TypeSignalsSchemaVersionError::Zero))
    ));

    // v4 predates the namespace-bearing TypeSignal identity and must be
    // rejected so the evaluator treats legacy rows as a cache miss.
    let unsupported_schema = TypeSignalsDocDto { schema_version: 4, ..dto.clone() };
    assert!(matches!(
        decode(&serde_json::to_string(&unsupported_schema).unwrap()),
        Err(TypeSignalsCodecError::UnsupportedSchemaVersion(_))
    ));

    let invalid_timestamp =
        TypeSignalsDocDto { generated_at: "2026-04-18T12:00:00+09:00".to_owned(), ..dto.clone() };
    assert!(matches!(
        decode(&serde_json::to_string(&invalid_timestamp).unwrap()),
        Err(TypeSignalsCodecError::InvalidTimestamp(_))
    ));

    let invalid_head = TypeSignalsDocDto { head_commit: "g".repeat(40), ..dto };
    assert!(matches!(
        decode(&serde_json::to_string(&invalid_head).unwrap()),
        Err(TypeSignalsCodecError::InvalidTimestamp(_))
    ));
}

#[test]
fn test_decode_with_invalid_rustdoc_execution_identity_returns_invalid_execution_identity() {
    let dto = valid_type_signals_doc_dto();

    let mut outside_target = serde_json::to_value(&dto).unwrap();
    outside_target["rustdoc_execution_identity"]["expected_json_path"] =
        serde_json::Value::String("/tmp/other-target/domain.json".to_owned());
    assert!(matches!(
        decode(&outside_target.to_string()),
        Err(TypeSignalsCodecError::InvalidExecutionIdentity(_))
    ));

    let mut empty_profile = serde_json::to_value(&dto).unwrap();
    empty_profile["rustdoc_execution_identity"]["profile"] =
        serde_json::Value::String("   ".to_owned());
    assert!(matches!(
        decode(&empty_profile.to_string()),
        Err(TypeSignalsCodecError::InvalidExecutionIdentity(_))
    ));
}

#[test]
fn test_decode_generated_invalid_execution_identity_components_returns_invalid_execution_identity()
{
    // This deterministic generator provides reproducible property coverage
    // without adding a test-only generator dependency to the production crate.
    let mut generator = PropertyGenerator::new(0x5eed_2026);
    for case in 0..64_u32 {
        let component = InvalidExecutionIdentityComponent::from_case(case);
        let mut payload = serde_json::to_value(valid_type_signals_doc_dto()).unwrap();
        set_generated_invalid_execution_identity_component(
            &mut payload,
            component,
            case,
            generator.next_u32(),
        );

        assert!(
            matches!(
                decode(&payload.to_string()),
                Err(TypeSignalsCodecError::InvalidExecutionIdentity(_))
            ),
            "generated invalid {component:?} case {case} was accepted"
        );
    }
}

#[test]
fn test_decode_rejects_legacy_schema_with_catalogue_row_as_cache_miss() {
    let payload = serde_json::json!({
        "schema_version": 4,
        "generated_at": "2026-04-18T12:00:00Z",
        "declaration_hash": DIGEST,
        "head_commit": "a".repeat(40),
        "baseline_hash": DIGEST,
        "signals": [{
            "type_name": "Shared",
            "kind_tag": "value_object",
            "signal": "blue",
            "found_type": true
        }]
    });

    assert!(matches!(
        decode(&payload.to_string()),
        Err(TypeSignalsCodecError::UnsupportedSchemaVersion(version))
            if version.value() == 4
    ));
}

#[test]
fn test_baseline_hash_changes_when_baseline_bytes_change() {
    assert_eq!(
        baseline_hash(b"baseline A").as_digest().as_str(),
        "7061fe86b948cf084b16235a204ce4a357f6b38f637f28edad27213428fda3d6"
    );
    assert_ne!(baseline_hash(b"baseline A"), baseline_hash(b"baseline B"));
}

#[test]
fn test_baseline_hash_cache_key_tracks_baseline_bytes_and_mismatch_requires_recomparison() {
    let digest = Sha256Digest::try_new(DIGEST.to_owned()).unwrap();
    let declaration = CatalogueDeclarationHash::new(digest.clone());
    let head_commit = CommitHash::try_new("a".repeat(40)).unwrap();
    let recorded_baseline = baseline_hash(b"baseline A");
    let current_baseline = baseline_hash(b"baseline B");
    let recorded =
        legacy_cache_key(declaration.clone(), head_commit.clone(), recorded_baseline.clone());
    let current = legacy_cache_key(declaration, head_commit, current_baseline);

    assert_eq!(recorded.baseline_hash(), &baseline_hash(b"baseline A"));
    let input = TypeSignalsReuseInput::verify(
        recorded,
        current,
        TypeSignalsWorktreeStatus::Clean,
        TypeSignalsAuthorityStatus::Readable,
    )
    .unwrap();
    assert_eq!(
        decide_type_signals_reuse(&input),
        TypeSignalsReuseDecision::ReextractAndEvaluate,
        "a changed rustdoc baseline digest must invalidate reuse"
    );
}
