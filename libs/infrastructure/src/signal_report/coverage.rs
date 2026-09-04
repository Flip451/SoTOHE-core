//! Impl-catalogue coverage validation: persisted row identities versus the catalogue
//! entries a track declares. Split from `signal_report/mod.rs` for module size.

use super::*;

pub(super) fn validate_impl_catalog_coverage(
    catalogue: &domain::tddd::catalogue_v2::CatalogueDocument,
    document: &domain::TypeSignalsDocument,
) -> Result<(), SignalReportError> {
    use crate::tddd::type_signals_evaluator::signal_tags::{
        contract_role_kind_tag, data_role_kind_tag, function_role_kind_tag,
    };

    let unavailable = || SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog);
    let mut expected = BTreeSet::new();
    for (name, entry) in catalogue.types() {
        expected.insert(SignalCoverageKey::catalogue_item(
            name.as_str(),
            CatalogueItemNamespace::Type,
            data_role_kind_tag(entry.role(), entry.kind()),
        ));
    }
    for (name, entry) in catalogue.traits() {
        expected.insert(SignalCoverageKey::catalogue_item(
            name.as_str(),
            CatalogueItemNamespace::Trait,
            contract_role_kind_tag(entry.role()),
        ));
    }
    for (path, entry) in catalogue.functions() {
        expected.insert(SignalCoverageKey::label(
            path.to_string(),
            function_role_kind_tag(entry.role()),
        ));
    }

    // Deletions have no live role/kind, so the builder persists their typed
    // catalogue identities (or function labels) with the synthetic `unknown`
    // tag. They are still required coverage, not arbitrary report rows.
    for deletion in catalogue.deletions() {
        match deletion {
            DeletionRecord::Type { name, .. } => {
                expected.insert(SignalCoverageKey::catalogue_item(
                    name.as_str(),
                    CatalogueItemNamespace::Type,
                    "unknown",
                ));
            }
            DeletionRecord::Trait { name, .. } => {
                expected.insert(SignalCoverageKey::catalogue_item(
                    name.as_str(),
                    CatalogueItemNamespace::Trait,
                    "unknown",
                ));
            }
            DeletionRecord::Function { path, .. } => {
                expected.insert(SignalCoverageKey::label(path.to_string(), "unknown"));
            }
        }
    }

    let expected_identities =
        expected.iter().map(|key| key.identity.clone()).collect::<BTreeSet<_>>();
    let mut observed = BTreeSet::new();
    for signal in document.signals() {
        if signal
            .missing_items()
            .iter()
            .chain(signal.extra_items())
            .any(|item| !is_safe_signal_line_text(item))
        {
            return Err(unavailable());
        }
        let key = SignalCoverageKey::from_signal(signal);
        if !observed.insert(key.clone()) {
            return Err(unavailable());
        }
        if signal.is_unknown_kind() {
            let identity_is_valid = match &key.identity {
                SignalCoverageIdentity::CatalogueItem { item_name, .. } => is_rust_path(item_name),
                SignalCoverageIdentity::Label { label } => is_safe_signal_line_text(label),
            };
            if !identity_is_valid {
                return Err(unavailable());
            }
            if expected.remove(&key) {
                continue;
            }
            // A known identity with the wrong `unknown` tag must not satisfy
            // the concrete live row. Only genuinely undeclared unknown
            // identities are admitted as additional report coverage.
            if expected_identities.contains(&key.identity) {
                return Err(unavailable());
            }
            continue;
        }
        if !expected.remove(&key) {
            return Err(unavailable());
        }
    }
    if !expected.is_empty() {
        return Err(unavailable());
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum SignalCoverageIdentity {
    CatalogueItem { item_name: String, namespace: CatalogueItemNamespace },
    Label { label: String },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct SignalCoverageKey {
    identity: SignalCoverageIdentity,
    kind_tag: String,
}

impl SignalCoverageKey {
    fn catalogue_item(
        item_name: impl Into<String>,
        namespace: CatalogueItemNamespace,
        kind_tag: impl Into<String>,
    ) -> Self {
        Self {
            identity: SignalCoverageIdentity::CatalogueItem {
                item_name: item_name.into(),
                namespace,
            },
            kind_tag: kind_tag.into(),
        }
    }

    fn label(label: impl Into<String>, kind_tag: impl Into<String>) -> Self {
        Self {
            identity: SignalCoverageIdentity::Label { label: label.into() },
            kind_tag: kind_tag.into(),
        }
    }

    fn from_signal(signal: &domain::TypeSignal) -> Self {
        let kind_tag = signal.kind_tag().to_owned();
        match signal.identity() {
            ThreeWaySignalIdentity::CatalogueItem { item_name, namespace } => {
                Self::catalogue_item(item_name.as_str(), *namespace, kind_tag)
            }
            ThreeWaySignalIdentity::Label { label } => Self::label(label.as_str(), kind_tag),
        }
    }
}

pub(super) fn impl_catalog_identity(signal: &domain::TypeSignal) -> String {
    match signal.identity() {
        ThreeWaySignalIdentity::CatalogueItem { namespace, .. } => {
            format!("{}:{}", catalogue_namespace_label(*namespace), signal.type_name())
        }
        ThreeWaySignalIdentity::Label { .. } => signal.type_name().to_owned(),
    }
}

pub(super) fn catalogue_namespace_label(namespace: CatalogueItemNamespace) -> &'static str {
    match namespace {
        CatalogueItemNamespace::Type => "type",
        CatalogueItemNamespace::Trait => "trait",
    }
}

pub(super) fn is_safe_signal_line_text(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && !value
            .chars()
            .any(|character| character.is_control() || matches!(character, '\u{2028}' | '\u{2029}'))
}

pub(super) fn is_rust_path(value: &str) -> bool {
    is_safe_signal_line_text(value)
        && value.split("::").all(|segment| {
            domain::tddd::catalogue_v2::identifiers::Identifier::new(segment).is_ok()
        })
}
