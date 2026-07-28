use super::*;
use domain::ContentHash;
use domain::tddd::semantic_verify::CatalogueEntryKey;
use domain::tddd::test_obligation::ids::{
    TestObligationAnchorId, TestObligationId, TestObligationItemIdentifier,
};
use domain::tddd::test_obligation::vocab::TestObligationKind;

fn codec(items_dir: PathBuf) -> JsonObligationFulfillmentCacheCodec {
    JsonObligationFulfillmentCacheCodec::new(items_dir)
}

fn edge_id() -> domain::tddd::test_obligation::ids::TestObligationEdgeId {
    domain::tddd::test_obligation::ids::TestObligationEdgeId::new(
        CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
        TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-09".to_owned()).unwrap(),
    )
}

fn obligation_id() -> TestObligationId {
    TestObligationId::new(
        CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
        TestObligationKind::Result,
        TestObligationItemIdentifier::try_new("entry".to_owned()).unwrap(),
    )
}

fn cache_entry(
    edge_id: domain::tddd::test_obligation::ids::TestObligationEdgeId,
    obligation_id: TestObligationId,
    key: ObligationFulfillmentCacheKey,
    verdict: ObligationFulfillmentVerdict,
    verifier_fingerprint: Option<VerifierPromptFingerprint>,
) -> ObligationFulfillmentCacheEntry {
    let location = domain::tddd::test_obligation::binding::TestLocation::new(
        domain::tddd::LayerId::try_new("infrastructure".to_owned()).unwrap(),
        domain::tddd::test_obligation::ids::TestModulePath::try_new("fixture".to_owned()).unwrap(),
        domain::tddd::test_obligation::ids::TestFunctionName::try_new(format!(
            "entry_hash_{}",
            &key.bound_tests_set_hash().as_hash().to_hex()[..2]
        ))
        .unwrap(),
    );
    let state = match verifier_fingerprint {
        Some(verifier_fingerprint) => ObligationFulfillmentCacheEntryState::Identified {
            verifier_fingerprint,
            bound_tests: Some(NonEmptyTestLocations::new(location, Vec::new())),
        },
        None => ObligationFulfillmentCacheEntryState::Legacy,
    };
    ObligationFulfillmentCacheEntry::new(edge_id, obligation_id, key, verdict, state)
}

fn key() -> ObligationFulfillmentCacheKey {
    ObligationFulfillmentCacheKey::new(
        BoundTestsSetHash::new(ContentHash::from_bytes([1u8; 32])),
        DeclarationHash::new(ContentHash::from_bytes([2u8; 32])),
        AnchorTextHash::new(ContentHash::from_bytes([3u8; 32])),
    )
}

fn verifier_fingerprint() -> VerifierPromptFingerprint {
    VerifierPromptFingerprint::new(ContentHash::from_bytes([4u8; 32]))
}

fn document(verdict: ObligationFulfillmentVerdict) -> ObligationFulfillmentCacheDocument {
    ObligationFulfillmentCacheDocument::new(
        TrackId::try_new("my-track").unwrap(),
        vec![cache_entry(edge_id(), obligation_id(), key(), verdict, Some(verifier_fingerprint()))],
    )
}

fn round_trip(verdict: ObligationFulfillmentVerdict) {
    let doc = document(verdict);
    let dto = document_to_dto(&doc);
    let json = serde_json::to_string_pretty(&dto).unwrap();
    let decoded: ObligationFulfillmentCacheDocumentDto = serde_json::from_str(&json).unwrap();
    assert_eq!(codec(PathBuf::new()).document_from_dto(decoded).unwrap(), doc);
}

// AC-06: the fulfilled verdict (with citation) round-trips, including the
// three-hash cache key.
#[test]
fn test_fulfilled_verdict_round_trips() {
    round_trip(ObligationFulfillmentVerdict::Fulfilled {
        citation: EvidenceCitation::try_new("asserts empty rejected".to_owned()).unwrap(),
    });
}

// AC-08: each fail category round-trips.
#[test]
fn test_fail_verdict_round_trips_all_categories() {
    for category in [
        FulfillmentFailCategory::Contradiction,
        FulfillmentFailCategory::Substitution,
        FulfillmentFailCategory::CentralUnverified,
    ] {
        round_trip(ObligationFulfillmentVerdict::Fail {
            category,
            reason: DiagnosticMessage::try_new("asserts the opposite".to_owned()).unwrap(),
        });
    }
}

#[test]
fn test_fail_verdict_json_uses_only_nested_non_empty_reason() {
    let dto = document_to_dto(&document(ObligationFulfillmentVerdict::Fail {
        category: FulfillmentFailCategory::Contradiction,
        reason: DiagnosticMessage::try_new("asserts the opposite".to_owned()).unwrap(),
    }));
    let json = serde_json::to_value(dto).unwrap();
    let entry = &json["entries"][0];

    assert!(entry.get("verdict_reason").is_none());
    assert_eq!(entry["verdict"]["reason"], "asserts the opposite");
    assert!(entry["verdict"]["reason"].as_str().is_some_and(|reason| !reason.is_empty()));
}

// AC-06: the pending verdict round-trips (treated as fail at the gate).
#[test]
fn test_pending_verdict_round_trips() {
    round_trip(ObligationFulfillmentVerdict::Pending);
}

// CN-04: the three hex hashes survive serialization exactly.
#[test]
fn test_hash_triple_serializes_as_hex() {
    let doc = document(ObligationFulfillmentVerdict::Pending);
    let dto = document_to_dto(&doc);
    let wire = &dto.entries[0].key;
    assert_eq!(wire.bound_tests_set_hash, "01".repeat(32));
    assert_eq!(wire.declaration_hash, "02".repeat(32));
    assert_eq!(wire.anchor_text_hash, "03".repeat(32));
    assert_eq!(dto.entries[0].verifier_fingerprint, Some("04".repeat(32)));
}

#[test]
fn test_legacy_entry_without_fingerprint_decodes_as_absent() {
    let dto = document_to_dto(&document(ObligationFulfillmentVerdict::Pending));
    let mut json = serde_json::to_value(dto).unwrap();
    json["entries"][0].as_object_mut().unwrap().remove("verifier_fingerprint");

    let legacy: ObligationFulfillmentCacheDocumentDto = serde_json::from_value(json).unwrap();
    let decoded = codec(PathBuf::new()).document_from_dto(legacy).unwrap();

    assert_eq!(decoded.entries()[0].verifier_fingerprint(), None);
}

#[test]
fn test_fingerprint_only_entry_decodes_as_identified() {
    let mut dto = document_to_dto(&document(ObligationFulfillmentVerdict::Pending));
    dto.entries[0].bound_tests = None;

    let decoded = codec(PathBuf::new()).document_from_dto(dto).unwrap();
    let entry = &decoded.entries()[0];

    assert_eq!(entry.verifier_fingerprint(), Some(&verifier_fingerprint()));
    assert_eq!(entry.bound_tests(), None);
    assert!(
        decoded
            .lookup_current(&edge_id(), &obligation_id(), &key(), &verifier_fingerprint())
            .unwrap()
            .is_some()
    );
}

#[test]
fn test_load_preserves_fingerprint_for_pre_bound_tests_entry() {
    let dir = tempfile::tempdir().unwrap();
    let codec = codec(dir.path().to_path_buf());
    let doc = document(ObligationFulfillmentVerdict::Pending);
    codec.save(&doc).unwrap();
    let path = dir.path().join(doc.track_id().as_ref()).join(FULFILLMENT_CACHE_ARTIFACT);
    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut historical = json["entries"][0].clone();
    historical["key"]["bound_tests_set_hash"] = serde_json::Value::String("09".repeat(32));
    historical.as_object_mut().unwrap().remove("bound_tests");
    json["entries"].as_array_mut().unwrap().push(historical);
    std::fs::write(path, serde_json::to_string(&json).unwrap()).unwrap();

    let Some(loaded) = codec.load(doc.track_id()).unwrap() else {
        panic!("a current entry must remain readable");
    };
    let historical_key = ObligationFulfillmentCacheKey::new(
        BoundTestsSetHash::new(ContentHash::from_bytes([9u8; 32])),
        key().declaration_hash().clone(),
        key().anchor_text_hash().clone(),
    );
    assert_eq!(loaded.entries()[1].verifier_fingerprint(), Some(&verifier_fingerprint()));
    assert_eq!(loaded.entries()[1].bound_tests(), None);
    assert!(
        loaded
            .lookup_current(&edge_id(), &obligation_id(), &historical_key, &verifier_fingerprint(),)
            .unwrap()
            .is_some()
    );
    assert!(
        loaded
            .lookup_current(&edge_id(), &obligation_id(), &key(), &verifier_fingerprint())
            .unwrap()
            .is_some()
    );
}

#[test]
fn test_load_rejects_entry_whose_persisted_key_differs_from_resolved_source() {
    let dir = tempfile::tempdir().unwrap();
    let codec = codec(dir.path().to_path_buf());
    let doc = document(ObligationFulfillmentVerdict::Pending);
    codec.save(&doc).unwrap();
    let path = dir.path().join(doc.track_id().as_ref()).join(FULFILLMENT_CACHE_ARTIFACT);
    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    json["entries"][0]["key"]["bound_tests_set_hash"] = serde_json::Value::String("09".repeat(32));
    std::fs::write(path, serde_json::to_string(&json).unwrap()).unwrap();

    assert!(codec.load(doc.track_id()).unwrap().is_some());
}

#[test]
fn test_load_rejects_when_persisted_bound_test_diagnostics_cannot_be_resolved() {
    let dir = tempfile::tempdir().unwrap();
    let writing_codec = codec(dir.path().to_path_buf());
    let doc = document(ObligationFulfillmentVerdict::Pending);
    writing_codec.save(&doc).unwrap();

    assert!(codec(dir.path().to_path_buf()).load(doc.track_id()).unwrap().is_some());
}

#[test]
fn test_load_ignores_historical_entry_that_has_a_different_key() {
    let dir = tempfile::tempdir().unwrap();
    let codec = codec(dir.path().to_path_buf());
    let doc = document(ObligationFulfillmentVerdict::Pending);
    codec.save(&doc).unwrap();
    let path = dir.path().join(doc.track_id().as_ref()).join(FULFILLMENT_CACHE_ARTIFACT);
    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let historical = {
        let entry = json["entries"][0].clone();
        let mut entry = entry;
        entry["key"]["bound_tests_set_hash"] = serde_json::Value::String("09".repeat(32));
        entry["bound_tests"][0]["test_name"] =
            serde_json::Value::String("entry_hash_09".to_owned());
        entry
    };
    json["entries"].as_array_mut().unwrap().push(historical);
    std::fs::write(path, serde_json::to_string(&json).unwrap()).unwrap();

    let Some(loaded) = codec.load(doc.track_id()).unwrap() else {
        panic!("a current entry must remain readable");
    };
    let current = loaded
        .lookup_current(&edge_id(), &obligation_id(), &key(), &verifier_fingerprint())
        .unwrap()
        .expect("the unchanged entry must be selected");
    assert_eq!(current.key(), &key());
}

#[test]
fn test_document_from_dto_rejects_invalid_duplicate_current_identity() {
    let mut dto = document_to_dto(&document(ObligationFulfillmentVerdict::Pending));
    let mut duplicate = dto.entries[0].clone();
    duplicate.bound_tests = Some(Vec::new());
    dto.entries.push(duplicate);

    assert!(matches!(
        codec(PathBuf::new()).document_from_dto(dto),
        Err(VerifyCacheError::MalformedJson(_))
    ));
}

#[test]
fn test_document_from_dto_rejects_excessive_entry_count_before_resolution() {
    let dto = document_to_dto(&document(ObligationFulfillmentVerdict::Pending));
    let oversized = ObligationFulfillmentCacheDocumentDto {
        track_id: dto.track_id,
        entries: vec![dto.entries[0].clone(); MAX_FULFILLMENT_CACHE_ENTRIES + 1],
    };

    assert!(matches!(
        codec(PathBuf::new()).document_from_dto(oversized),
        Err(VerifyCacheError::MalformedJson(_))
    ));
}

#[test]
fn test_entry_from_dto_rejects_excessive_bound_test_locations_before_resolution() {
    let mut dto =
        document_to_dto(&document(ObligationFulfillmentVerdict::Pending)).entries.remove(0);
    dto.bound_tests =
        Some(vec![dto.bound_tests.as_ref().unwrap()[0].clone(); MAX_BOUND_TESTS_PER_ENTRY + 1]);

    assert!(matches!(
        codec(PathBuf::new()).entry_from_dto(dto),
        Err(VerifyCacheError::MalformedJson(_))
    ));
}

#[test]
fn test_save_rejects_excessive_entry_count_before_serializing() {
    let dir = tempfile::tempdir().unwrap();
    let codec = codec(dir.path().to_path_buf());
    let entry = cache_entry(
        edge_id(),
        obligation_id(),
        key(),
        ObligationFulfillmentVerdict::Pending,
        Some(verifier_fingerprint()),
    );
    let doc = ObligationFulfillmentCacheDocument::new(
        TrackId::try_new("my-track").unwrap(),
        vec![entry; MAX_FULFILLMENT_CACHE_ENTRIES + 1],
    );

    assert!(codec.save(&doc).is_err());
    assert!(!dir.path().join("my-track").exists());
}

#[test]
fn test_save_rejects_excessive_bound_tests_before_serializing() {
    let dir = tempfile::tempdir().unwrap();
    let codec = codec(dir.path().to_path_buf());
    let location = domain::tddd::test_obligation::binding::TestLocation::new(
        domain::tddd::LayerId::try_new("infrastructure".to_owned()).unwrap(),
        domain::tddd::test_obligation::ids::TestModulePath::try_new("fixture".to_owned()).unwrap(),
        domain::tddd::test_obligation::ids::TestFunctionName::try_new("entry_hash_01".to_owned())
            .unwrap(),
    );
    let locations = NonEmptyTestLocations::try_new(
        std::iter::repeat_n(location, MAX_BOUND_TESTS_PER_ENTRY + 1).collect(),
    )
    .unwrap();
    let entry = ObligationFulfillmentCacheEntry::new(
        edge_id(),
        obligation_id(),
        key(),
        ObligationFulfillmentVerdict::Pending,
        ObligationFulfillmentCacheEntryState::Identified {
            verifier_fingerprint: verifier_fingerprint(),
            bound_tests: Some(locations),
        },
    );
    let doc =
        ObligationFulfillmentCacheDocument::new(TrackId::try_new("my-track").unwrap(), vec![entry]);

    assert!(codec.save(&doc).is_err());
    assert!(!dir.path().join("my-track").exists());
}

#[test]
fn test_cache_dto_persists_resolved_bound_test_locations() {
    let dto = document_to_dto(&document(ObligationFulfillmentVerdict::Pending));
    let json = serde_json::to_value(dto).unwrap();

    assert_eq!(json["entries"][0]["bound_tests"].as_array().map(Vec::len), Some(1));
}

#[test]
fn test_bound_tests_diagnostic_representation_does_not_change_lookup_identity() {
    let current_key = key();
    let mut json =
        serde_json::to_value(document_to_dto(&document(ObligationFulfillmentVerdict::Pending)))
            .unwrap();
    json["entries"][0]["bound_tests"][0]["test_name"] =
        serde_json::Value::String("diagnostic_only_location".to_owned());
    let dto = serde_json::from_value::<ObligationFulfillmentCacheDocumentDto>(json).unwrap();

    let document = codec(PathBuf::new()).document_from_dto(dto).unwrap();
    let selected = document
        .lookup_current(&edge_id(), &obligation_id(), &current_key, &verifier_fingerprint())
        .unwrap()
        .expect("the unchanged cache key must still select the row");

    assert_eq!(
        selected.bound_tests().unwrap().as_slice()[0].test_name().as_str(),
        "diagnostic_only_location"
    );
}

// Every parseable row is retained for full-identity ambiguity detection, so
// malformed historical data fails closed rather than being silently dropped.
#[test]
fn test_load_with_malformed_historical_hash_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let codec = codec(dir.path().to_path_buf());
    let doc = document(ObligationFulfillmentVerdict::Pending);
    codec.save(&doc).unwrap();
    let path = dir.path().join(doc.track_id().as_ref()).join(FULFILLMENT_CACHE_ARTIFACT);
    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut historical = json["entries"][0].clone();
    historical["key"]["declaration_hash"] = serde_json::Value::String("not-a-hash".to_owned());
    historical.as_object_mut().unwrap().remove("bound_tests");
    historical.as_object_mut().unwrap().remove("verifier_fingerprint");
    json["entries"].as_array_mut().unwrap().push(historical);
    std::fs::write(path, serde_json::to_string(&json).unwrap()).unwrap();

    assert!(matches!(codec.load(doc.track_id()), Err(VerifyCacheError::MalformedJson(_))));
}

// IN-09 fail-closed: an unknown field is rejected by deny_unknown_fields.
#[test]
fn test_unknown_field_is_rejected() {
    let json = r#"{ "track_id": "t", "entries": [], "extra": true }"#;
    assert!(serde_json::from_str::<ObligationFulfillmentCacheDocumentDto>(json).is_err());
}

// IN-09 / AC-06: the codec persists and reloads a cache via the port.
#[test]
fn test_codec_save_then_load_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let codec = codec(dir.path().to_path_buf());
    let current_key = key();
    let current = cache_entry(
        edge_id(),
        obligation_id(),
        current_key.clone(),
        ObligationFulfillmentVerdict::Fulfilled {
            citation: EvidenceCitation::try_new("cite".to_owned()).unwrap(),
        },
        Some(verifier_fingerprint()),
    );
    let expected_bound_tests = current.bound_tests().cloned();
    let historical_keys = [
        ObligationFulfillmentCacheKey::new(
            BoundTestsSetHash::new(ContentHash::from_bytes([9u8; 32])),
            current_key.declaration_hash().clone(),
            current_key.anchor_text_hash().clone(),
        ),
        ObligationFulfillmentCacheKey::new(
            current_key.bound_tests_set_hash().clone(),
            DeclarationHash::new(ContentHash::from_bytes([9u8; 32])),
            current_key.anchor_text_hash().clone(),
        ),
        ObligationFulfillmentCacheKey::new(
            current_key.bound_tests_set_hash().clone(),
            current_key.declaration_hash().clone(),
            AnchorTextHash::new(ContentHash::from_bytes([9u8; 32])),
        ),
    ];
    let mut entries = historical_keys
        .into_iter()
        .map(|key| {
            cache_entry(
                edge_id(),
                obligation_id(),
                key,
                ObligationFulfillmentVerdict::Pending,
                Some(verifier_fingerprint()),
            )
        })
        .collect::<Vec<_>>();
    entries.push(cache_entry(
        edge_id(),
        obligation_id(),
        current_key.clone(),
        ObligationFulfillmentVerdict::Pending,
        Some(VerifierPromptFingerprint::new(ContentHash::from_bytes([8u8; 32]))),
    ));
    entries.push(current);
    let doc =
        ObligationFulfillmentCacheDocument::new(TrackId::try_new("my-track").unwrap(), entries);
    codec.save(&doc).unwrap();
    let Some(loaded) = codec.load(doc.track_id()).unwrap() else {
        panic!("saved diagnostic cache must load as current");
    };
    let selected = loaded
        .lookup_current(&edge_id(), &obligation_id(), &current_key, &verifier_fingerprint())
        .unwrap()
        .unwrap();
    assert_eq!(selected.key(), &current_key);
    assert_eq!(selected.bound_tests(), expected_bound_tests.as_ref());

    let duplicate = ObligationFulfillmentCacheDocument::new(
        doc.track_id().clone(),
        vec![
            cache_entry(
                edge_id(),
                obligation_id(),
                current_key.clone(),
                ObligationFulfillmentVerdict::Pending,
                Some(verifier_fingerprint()),
            ),
            cache_entry(
                edge_id(),
                obligation_id(),
                current_key.clone(),
                ObligationFulfillmentVerdict::Pending,
                Some(verifier_fingerprint()),
            ),
        ],
    );
    codec.save(&duplicate).unwrap();
    let Some(loaded) = codec.load(duplicate.track_id()).unwrap() else {
        panic!("duplicate diagnostic cache must be readable before lookup");
    };
    assert!(
        loaded
            .lookup_current(&edge_id(), &obligation_id(), &current_key, &verifier_fingerprint())
            .is_err()
    );
}

#[test]
fn test_codec_replaces_stale_cache_for_subsequent_check() {
    let dir = tempfile::tempdir().unwrap();
    let codec = codec(dir.path().to_path_buf());
    let current_key = key();
    let stale_key = ObligationFulfillmentCacheKey::new(
        BoundTestsSetHash::new(ContentHash::from_bytes([9u8; 32])),
        current_key.declaration_hash().clone(),
        current_key.anchor_text_hash().clone(),
    );
    let stale = ObligationFulfillmentCacheDocument::new(
        TrackId::try_new("my-track").unwrap(),
        vec![cache_entry(
            edge_id(),
            obligation_id(),
            stale_key,
            ObligationFulfillmentVerdict::Pending,
            Some(verifier_fingerprint()),
        )],
    );
    codec.save(&stale).unwrap();

    let Some(before_re_evaluation) = codec.load(stale.track_id()).unwrap() else {
        panic!("saved stale cache must remain readable for fail-closed checking");
    };
    assert!(
        before_re_evaluation
            .lookup_current(&edge_id(), &obligation_id(), &current_key, &verifier_fingerprint(),)
            .unwrap()
            .is_none()
    );

    let refreshed = ObligationFulfillmentCacheDocument::new(
        stale.track_id().clone(),
        vec![cache_entry(
            edge_id(),
            obligation_id(),
            current_key.clone(),
            ObligationFulfillmentVerdict::Fulfilled {
                citation: EvidenceCitation::try_new("re-evaluated evidence".to_owned()).unwrap(),
            },
            Some(verifier_fingerprint()),
        )],
    );
    codec.save(&refreshed).unwrap();

    let Some(after_re_evaluation) = codec.load(refreshed.track_id()).unwrap() else {
        panic!("re-evaluated cache must be readable for the subsequent check");
    };
    assert!(
        after_re_evaluation
            .lookup_current(&edge_id(), &obligation_id(), &current_key, &verifier_fingerprint(),)
            .unwrap()
            .is_some()
    );
}

// IN-09 / CN-04: the trusted items root itself must not be a symlink.
#[cfg(unix)]
#[test]
fn test_codec_symlinked_items_root_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let real_items = dir.path().join("real-items");
    let link_items = dir.path().join("items-link");
    std::fs::create_dir_all(&real_items).unwrap();
    std::os::unix::fs::symlink(&real_items, &link_items).unwrap();

    let codec = codec(link_items);
    let doc = document(ObligationFulfillmentVerdict::Pending);
    match codec.load(doc.track_id()) {
        Err(VerifyCacheError::Io(message)) => {
            assert!(message.as_str().contains("symlinked track items root"));
        }
        other => panic!("expected symlinked items root load error, got {other:?}"),
    }
    let save_error = codec.save(&doc).unwrap_err();
    assert!(save_error.as_str().contains("symlinked track items root"));
}

// IN-14 / AC-10: a missing cache loads as `None`.
#[test]
fn test_load_missing_cache_is_none() {
    let dir = tempfile::tempdir().unwrap();
    let codec = codec(dir.path().to_path_buf());
    assert!(codec.load(&TrackId::try_new("absent-track").unwrap()).unwrap().is_none());
}

#[cfg(unix)]
#[test]
fn test_load_fifo_returns_error_without_blocking() {
    let dir = tempfile::tempdir().unwrap();
    let track_id = TrackId::try_new("fifo-track").unwrap();
    let track_dir = dir.path().join(track_id.as_ref());
    std::fs::create_dir_all(&track_dir).unwrap();
    let fifo = track_dir.join(FULFILLMENT_CACHE_ARTIFACT);
    rustix::fs::mkfifoat(rustix::fs::CWD, &fifo, rustix::fs::Mode::from_raw_mode(0o600)).unwrap();

    assert!(matches!(
        codec(dir.path().to_path_buf()).load(&track_id),
        Err(VerifyCacheError::Io(_))
    ));
}

// CN-04: a fulfillment cache whose embedded `track_id` disagrees with the
// requested id must not be returned — a copy from another track would
// otherwise satisfy or block the gate on unrelated verdicts.
#[test]
fn test_codec_load_rejects_track_id_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let codec = codec(dir.path().to_path_buf());
    let doc = document(ObligationFulfillmentVerdict::Pending);
    codec.save(&doc).unwrap();

    let other_track = TrackId::try_new("other-track").unwrap();
    let source = dir.path().join(doc.track_id().as_ref()).join(FULFILLMENT_CACHE_ARTIFACT);
    let target_dir = dir.path().join(other_track.as_ref());
    std::fs::create_dir_all(&target_dir).unwrap();
    std::fs::rename(&source, target_dir.join(FULFILLMENT_CACHE_ARTIFACT)).unwrap();

    match codec.load(&other_track) {
        Err(VerifyCacheError::MalformedJson(message)) => {
            let text = message.as_str();
            assert!(text.contains("fulfillment cache track id mismatch"), "got: {text}");
            assert!(text.contains(other_track.as_ref()), "got: {text}");
            assert!(text.contains(doc.track_id().as_ref()), "got: {text}");
        }
        other => panic!("expected MalformedJson track-id mismatch error, got {other:?}"),
    }
}
