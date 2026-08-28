use domain::ConfidenceSignal;
use domain::tddd::catalogue_v2::identifiers::CatalogueItemNamespace;
use domain::tddd::type_signals_doc::{
    TypeSignalsAuthorityStatus, TypeSignalsReuseDecision, TypeSignalsReuseInput,
    TypeSignalsWorktreeStatus, decide_type_signals_reuse,
};

use super::*;

const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn sample_doc() -> TypeSignalsDocument {
    let digest = Sha256Digest::try_new(DIGEST.to_owned()).unwrap();
    TypeSignalsDocument::new(
        Timestamp::new("2026-04-18T12:00:00Z").unwrap(),
        TypeSignalsCacheKey::new(
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
        TypeSignalsCacheKey::new(
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
        TypeSignalsCacheKey::new(
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
        TypeSignalsCacheKey::new(
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
fn test_decode_rejects_v5_signal_without_explicit_identity() {
    let payload = serde_json::json!({
        "schema_version": domain::TYPE_SIGNALS_SCHEMA_VERSION,
        "generated_at": "2026-04-18T12:00:00Z",
        "declaration_hash": DIGEST,
        "head_commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "baseline_hash": DIGEST,
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
        "v5 must not infer a label from an omitted catalogue identity discriminator: {result:?}"
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
    let recorded = TypeSignalsCacheKey::new(
        declaration.clone(),
        head_commit.clone(),
        recorded_baseline.clone(),
    );
    let current = TypeSignalsCacheKey::new(declaration, head_commit, current_baseline);

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
        TypeSignalsReuseDecision::ReevaluateWithoutExtraction,
        "a changed rustdoc baseline digest must invalidate reuse"
    );
}
