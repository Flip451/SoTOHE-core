//! Accumulation helpers for converting report signals into persisted rows.

use std::collections::{BTreeMap, HashMap};

use domain::tddd::catalogue_v2::identifiers::CatalogueItemNamespace;
use domain::tddd::signal_evaluator::ThreeWaySignalIdentity;
use domain::{ConfidenceSignal, FreeText};

use super::super::signal_tags::kind_tag_namespace;
use super::identity_index::TypeSignalIdentityIndex;
use crate::tddd::ThreeWaySignalKind;

/// `(signal, found_type, found_items, missing_items, extra_items)`.
pub(crate) type AccEntry = (ConfidenceSignal, bool, Vec<String>, Vec<String>, Vec<String>);
/// Persisted entry key plus optional namespace; shared identities are isolated.
pub(crate) type AccKey = (String, Option<CatalogueItemNamespace>);

pub(crate) fn signal_identity(
    name: impl Into<String>,
    namespace: Option<CatalogueItemNamespace>,
) -> ThreeWaySignalIdentity {
    match namespace {
        Some(namespace) => {
            ThreeWaySignalIdentity::CatalogueItem { item_name: FreeText::new(name), namespace }
        }
        None => ThreeWaySignalIdentity::Label { label: FreeText::new(name) },
    }
}

pub(crate) fn entry_namespace_presence(
    entry_key: &str,
    kind_tag_map: &BTreeMap<String, Vec<&'static str>>,
    identity_index: &TypeSignalIdentityIndex,
) -> (bool, bool, bool) {
    let tags = kind_tags_for_entry(entry_key, kind_tag_map, identity_index);
    let has_type = identity_index.key_has_namespace(entry_key, CatalogueItemNamespace::Type)
        || tags.iter().any(|tag| kind_tag_namespace(tag) == Some(CatalogueItemNamespace::Type));
    let has_trait = identity_index.key_has_namespace(entry_key, CatalogueItemNamespace::Trait)
        || tags.iter().any(|tag| kind_tag_namespace(tag) == Some(CatalogueItemNamespace::Trait));
    let has_namespace_less = tags.iter().any(|tag| kind_tag_namespace(tag).is_none());
    (has_type, has_trait, has_namespace_less)
}

/// Returns every accumulator namespace represented by an entry. Keeping the
/// namespace-less bucket in this set is what prevents a function label from
/// joining a type or trait that happens to use the same spelling.
pub(crate) fn accumulator_namespaces(
    entry_key: &str,
    kind_tag_map: &BTreeMap<String, Vec<&'static str>>,
    identity_index: &TypeSignalIdentityIndex,
) -> Vec<Option<CatalogueItemNamespace>> {
    let (has_type, has_trait, has_namespace_less) =
        entry_namespace_presence(entry_key, kind_tag_map, identity_index);
    let mut namespaces = Vec::new();
    if has_type {
        namespaces.push(Some(CatalogueItemNamespace::Type));
    }
    if has_trait {
        namespaces.push(Some(CatalogueItemNamespace::Trait));
    }
    if has_namespace_less || namespaces.is_empty() {
        namespaces.push(None);
    }
    namespaces
}

/// The type namespace is used for a local impl owner; an owner without a
/// catalogue type remains an external report label.
pub(crate) fn impl_owner_namespace(
    owner: &str,
    kind_tag_map: &BTreeMap<String, Vec<&'static str>>,
    identity_index: &TypeSignalIdentityIndex,
) -> Option<CatalogueItemNamespace> {
    if identity_index.key_has_namespace(owner, CatalogueItemNamespace::Type)
        || kind_tags_for_entry(owner, kind_tag_map, identity_index)
            .iter()
            .any(|tag| kind_tag_namespace(tag) == Some(CatalogueItemNamespace::Type))
    {
        Some(CatalogueItemNamespace::Type)
    } else {
        None
    }
}

pub(crate) fn record_plain_signal(
    acc: &mut HashMap<AccKey, AccEntry>,
    order: &mut Vec<AccKey>,
    acc_key: AccKey,
    confidence: ConfidenceSignal,
    found_in_c: bool,
    ambiguous: bool,
) {
    let initial_signal = if ambiguous { ConfidenceSignal::Yellow } else { confidence };
    let entry = acc.entry(acc_key.clone()).or_insert_with(|| {
        order.push(acc_key);
        (initial_signal, found_in_c, Vec::new(), Vec::new(), Vec::new())
    });
    entry.0 = worse_signal(entry.0, confidence);
    entry.1 = entry.1 || found_in_c;
    if ambiguous {
        entry.0 = worse_signal(entry.0, ConfidenceSignal::Yellow);
    }
}

pub(crate) fn kind_tags_for_entry(
    entry_key: &str,
    kind_tag_map: &BTreeMap<String, Vec<&'static str>>,
    identity_index: &TypeSignalIdentityIndex,
) -> Vec<&'static str> {
    if let Some(kind_tags) = kind_tag_map.get(entry_key) {
        return kind_tags.clone();
    }

    let mut kind_tags = Vec::new();
    for alias in identity_index.aliases_for_entry_key(entry_key) {
        if let Some(tags) = kind_tag_map.get(alias) {
            for &tag in tags {
                if !kind_tags.contains(&tag) {
                    kind_tags.push(tag);
                }
            }
        }
    }
    kind_tags
}

/// Selects only tags belonging to the accumulator identity. Namespace-less
/// rows retain report-only tags; when an external owner has no such tag, the
/// historical tag is retained on its label rather than silently dropped.
pub(crate) fn kind_tags_for_accumulator(
    namespace: Option<CatalogueItemNamespace>,
    all_kind_tags: &[&'static str],
    allow_unscoped_kind_tags: bool,
) -> Vec<&'static str> {
    match namespace {
        Some(namespace) => all_kind_tags
            .iter()
            .copied()
            .filter(|kind_tag| kind_tag_namespace(kind_tag) == Some(namespace))
            .collect(),
        None => {
            let label_tags = all_kind_tags
                .iter()
                .copied()
                .filter(|kind_tag| kind_tag_namespace(kind_tag).is_none())
                .collect::<Vec<_>>();
            if label_tags.is_empty() && allow_unscoped_kind_tags {
                all_kind_tags.to_vec()
            } else {
                label_tags
            }
        }
    }
}

pub(crate) fn stored_entry_key_for_kind_name(
    kind_name: &str,
    namespace: CatalogueItemNamespace,
    identity_index: &TypeSignalIdentityIndex,
) -> String {
    if let Some(candidates) =
        identity_index.declaration_candidates_in_namespace(kind_name, namespace)
    {
        if let [candidate] = candidates.as_slice() {
            return candidate.clone();
        }
    }
    match identity_index.declaration_candidates(kind_name) {
        Some(candidates) => match candidates.as_slice() {
            [candidate] if !identity_index.has_known_namespace(candidate) => candidate.clone(),
            _ => kind_name.to_owned(),
        },
        _ => kind_name.to_owned(),
    }
}

pub(crate) fn signal_kind_to_confidence(kind: ThreeWaySignalKind) -> ConfidenceSignal {
    match kind {
        ThreeWaySignalKind::Blue => ConfidenceSignal::Blue,
        ThreeWaySignalKind::Yellow => ConfidenceSignal::Yellow,
        ThreeWaySignalKind::Red => ConfidenceSignal::Red,
        ThreeWaySignalKind::Skip => ConfidenceSignal::Yellow,
    }
}

pub(crate) fn worse_signal(a: ConfidenceSignal, b: ConfidenceSignal) -> ConfidenceSignal {
    match (a, b) {
        (ConfidenceSignal::Red, _) | (_, ConfidenceSignal::Red) => ConfidenceSignal::Red,
        (ConfidenceSignal::Yellow, _) | (_, ConfidenceSignal::Yellow) => ConfidenceSignal::Yellow,
        _ => ConfidenceSignal::Blue,
    }
}
