//! Conversion of three-way evaluator output into persisted type signals.

use std::collections::{BTreeMap, HashMap};

use domain::tddd::NewTypeGraphCodecError;
use domain::tddd::catalogue_v2::roles::ItemAction;
use domain::tddd::catalogue_v2::{
    CatalogueDocument, CrateName, DeletionRecord, FullyQualifiedItemPath, ParamName, TypeRef,
};
use domain::{ConfidenceSignal, TypeSignal};
use rustdoc_types::{Id, ItemSummary};

use crate::tddd::canonical_type_identity::canonicalize_catalogue_type_ref;
use crate::tddd::{ThreeWaySignal, ThreeWaySignalKind};

/// Intermediate accumulator entry for a single top-level item.
///
/// Fields: `(signal, found_type, found_items, missing_items, extra_items)`.
type AccEntry = (ConfidenceSignal, bool, Vec<String>, Vec<String>, Vec<String>);

/// Canonical catalogue-key aliases used while collapsing evaluator output.
///
/// The evaluator deliberately keeps short labels for human-readable reports.
/// The persisted type-signals document is consumed as an entry-key index,
/// however, so an impl such as `ShortOwner: Trait` must join the catalogue
/// entry that owns the fully qualified identity.  The aliases in this index
/// are derived from the catalogue declarations and the T004 canonicalizer;
/// no renderer-local path spelling is treated as an identity.
#[derive(Debug, Default)]
pub(super) struct TypeSignalIdentityIndex {
    canonical_to_keys: BTreeMap<String, Vec<String>>,
    declaration_aliases_to_keys: BTreeMap<String, Vec<String>>,
    impl_aliases_to_keys: BTreeMap<String, Vec<String>>,
}

impl TypeSignalIdentityIndex {
    fn add_canonical(&mut self, canonical: &str, key: &str) {
        add_unique(&mut self.canonical_to_keys, canonical, key);
    }

    fn add_alias(&mut self, alias: &str, key: &str) {
        add_unique(&mut self.declaration_aliases_to_keys, alias, key);
    }

    fn add_impl_alias(&mut self, alias: &str, key: &str) {
        add_unique(&mut self.impl_aliases_to_keys, alias, key);
    }

    fn declaration_candidates(&self, raw: &str) -> Option<Vec<String>> {
        let exact =
            self.declaration_aliases_to_keys.get(raw).or_else(|| self.canonical_to_keys.get(raw));
        if exact.is_some() || raw.contains("::") {
            return exact.cloned();
        }
        self.declaration_aliases_to_keys.get(short_name(raw)).cloned()
    }

    fn owner_candidates(&self, raw: &str) -> Option<Vec<String>> {
        if let Some(keys) = self.impl_aliases_to_keys.get(raw) {
            return Some(keys.clone());
        }

        let mut candidates = Vec::new();
        add_candidates(&mut candidates, self.declaration_aliases_to_keys.get(raw));
        if let Some(keys) = self.canonical_to_keys.get(raw) {
            add_candidates(&mut candidates, Some(keys));
        }
        if !raw.contains("::") {
            let short = short_name(raw);
            if short != raw {
                if let Some(keys) = self.impl_aliases_to_keys.get(short) {
                    return Some(keys.clone());
                }
                add_candidates(&mut candidates, self.declaration_aliases_to_keys.get(short));
            }
        }
        (!candidates.is_empty()).then_some(candidates)
    }

    fn aliases_for_entry_key<'a>(&'a self, key: &str) -> Vec<&'a str> {
        let mut aliases = Vec::new();
        for (alias, keys) in
            self.canonical_to_keys.iter().chain(self.declaration_aliases_to_keys.iter())
        {
            if keys.len() == 1
                && keys.first().is_some_and(|candidate| candidate.as_str() == key)
                && !aliases.contains(&alias.as_str())
            {
                aliases.push(alias.as_str());
            }
        }
        aliases
    }
}

/// Builds the identity join used by the type-signal producer.
///
/// Catalogue entry keys remain the persisted spelling.  Their canonical
/// identities are computed through the T004 choke point, and impl owners are
/// then joined to the matching declared entry (or to their own fully
/// qualified `for_type` key when the owner is external, such as `Arc<T>`).
///
/// # Errors
///
/// Returns a diagnostic when a catalogue declaration cannot be reconciled
/// against the authoritative rustdoc path universe.
pub(super) fn build_type_signal_identity_index(
    catalogue: &CatalogueDocument,
    rustdoc_paths: &HashMap<Id, ItemSummary>,
) -> Result<TypeSignalIdentityIndex, String> {
    let mut index = TypeSignalIdentityIndex::default();
    let catalogue_crate = catalogue.crate_name();

    for (key, entry) in catalogue.types() {
        add_entry_identity(
            &mut index,
            catalogue_crate,
            key.as_str(),
            entry.module_path(),
            entry.action(),
            rustdoc_paths,
        )?;
    }
    for (key, entry) in catalogue.traits() {
        add_entry_identity(
            &mut index,
            catalogue_crate,
            key.as_str(),
            entry.module_path(),
            entry.action(),
            rustdoc_paths,
        )?;
    }

    for deletion in catalogue.deletions() {
        let key = match deletion {
            DeletionRecord::Type { name, .. } | DeletionRecord::Trait { name, .. } => name,
            DeletionRecord::Function { .. } => continue,
        };
        add_deletion_identity(&mut index, catalogue_crate, key.as_str(), rustdoc_paths)?;
    }

    for trait_impl in catalogue.trait_impls() {
        let generic_params = trait_impl
            .impl_generics()
            .iter()
            .map(|param| param.name.clone())
            .collect::<Vec<ParamName>>();
        let canonical = match canonicalize_catalogue_type_ref(
            trait_impl.for_type(),
            catalogue_crate,
            rustdoc_paths,
            &generic_params,
        ) {
            Ok(canonical) => Some(canonical),
            Err(NewTypeGraphCodecError::UnresolvedIdentifier(_))
                if matches!(trait_impl.action(), ItemAction::Add | ItemAction::Delete) =>
            {
                None
            }
            Err(error) => {
                return Err(format!(
                    "cannot canonicalize type-signal impl owner '{}': {error}",
                    trait_impl.for_type()
                ));
            }
        };

        // A local type entry owns its impl rows.  External/self types that are
        // not declared in this catalogue retain the fully qualified for_type
        // spelling as their signal entry key.
        let owner_key = match canonical.as_ref() {
            Some(canonical) => declaration_key_for_canonical(&index, canonical.as_str()),
            None => declaration_key_for_raw_owner(&index, trait_impl.for_type().as_str()),
        }
        .unwrap_or_else(|| trait_impl.for_type().as_str().to_owned());
        if let Some(canonical) = canonical {
            index.add_impl_alias(canonical.as_str(), &owner_key);
        }
        index.add_impl_alias(trait_impl.for_type().as_str(), &owner_key);
        index.add_impl_alias(strip_generic_suffix(trait_impl.for_type().as_str()), &owner_key);
        index.add_impl_alias(short_name(trait_impl.for_type().as_str()), &owner_key);
    }

    Ok(index)
}

fn add_entry_identity(
    index: &mut TypeSignalIdentityIndex,
    catalogue_crate: &CrateName,
    key: &str,
    module_path: &domain::tddd::catalogue_v2::ModulePath,
    action: ItemAction,
    rustdoc_paths: &HashMap<Id, ItemSummary>,
) -> Result<(), String> {
    let entry_key = domain::tddd::catalogue_v2::CatalogueEntryKey::try_new(key.to_owned())
        .map_err(|error| format!("invalid catalogue entry key '{key}': {error}"))?;
    let identity =
        FullyQualifiedItemPath::from_catalogue_entry_key(catalogue_crate, &entry_key, module_path)
            .map_err(|error| format!("cannot construct catalogue identity for '{key}': {error}"))?;
    let identity_text = identity.to_string();
    index.add_alias(key, key);
    index.add_alias(&identity_text, key);
    index.add_alias(short_name(key), key);
    let identity_ref = TypeRef::new(identity.to_string())
        .map_err(|error| format!("cannot construct TypeRef for '{key}': {error}"))?;
    match canonicalize_catalogue_type_ref(&identity_ref, catalogue_crate, rustdoc_paths, &[]) {
        Ok(canonical) => index.add_canonical(canonical.as_str(), key),
        Err(NewTypeGraphCodecError::UnresolvedIdentifier(_)) if action == ItemAction::Add => {}
        Err(error) => {
            return Err(format!("cannot canonicalize catalogue entry '{key}': {error}"));
        }
    }
    Ok(())
}

fn add_deletion_identity(
    index: &mut TypeSignalIdentityIndex,
    catalogue_crate: &CrateName,
    key: &str,
    rustdoc_paths: &HashMap<Id, ItemSummary>,
) -> Result<(), String> {
    let _entry_key = domain::tddd::catalogue_v2::CatalogueEntryKey::try_new(key.to_owned())
        .map_err(|error| format!("invalid catalogue deletion key '{key}': {error}"))?;
    let identity_ref = TypeRef::new(key.to_owned())
        .map_err(|error| format!("cannot construct TypeRef for '{key}': {error}"))?;
    index.add_alias(key, key);
    index.add_alias(short_name(key), key);
    match canonicalize_catalogue_type_ref(&identity_ref, catalogue_crate, rustdoc_paths, &[]) {
        Ok(canonical) => index.add_canonical(canonical.as_str(), key),
        Err(NewTypeGraphCodecError::UnresolvedIdentifier(_)) => {
            index.add_canonical(key, key);
        }
        Err(error) => {
            return Err(format!("cannot canonicalize catalogue deletion '{key}': {error}"));
        }
    }
    Ok(())
}

fn add_unique(map: &mut BTreeMap<String, Vec<String>>, alias: &str, key: &str) {
    let keys = map.entry(alias.to_owned()).or_default();
    if !keys.iter().any(|existing| existing == key) {
        keys.push(key.to_owned());
    }
}

fn unique_key(keys: Option<&Vec<String>>) -> Option<&str> {
    let keys = keys?;
    match keys.as_slice() {
        [key] => Some(key.as_str()),
        _ => None,
    }
}

fn declaration_key_for_canonical(
    index: &TypeSignalIdentityIndex,
    canonical: &str,
) -> Option<String> {
    let owner_identity = strip_generic_suffix(canonical);
    unique_key(index.canonical_to_keys.get(canonical))
        .or_else(|| unique_key(index.canonical_to_keys.get(owner_identity)))
        .map(ToOwned::to_owned)
}

fn declaration_key_for_raw_owner(index: &TypeSignalIdentityIndex, raw: &str) -> Option<String> {
    let outer = strip_generic_suffix(raw);
    [raw, outer].into_iter().find_map(|candidate| {
        index
            .declaration_candidates(candidate)
            .and_then(|keys| (keys.len() == 1).then(|| keys.into_iter().next()).flatten())
    })
}

fn add_candidates(candidates: &mut Vec<String>, keys: Option<&Vec<String>>) {
    for key in keys.into_iter().flatten() {
        if !candidates.iter().any(|candidate| candidate == key) {
            candidates.push(key.clone());
        }
    }
}

fn strip_generic_suffix(raw: &str) -> &str {
    raw.split_once('<').map_or(raw, |(head, _)| head)
}

fn short_name(raw: &str) -> &str {
    strip_generic_suffix(raw).rsplit("::").next().unwrap_or(raw)
}

pub(super) fn build_type_signals_from_report<'a>(
    signals: impl Iterator<Item = &'a ThreeWaySignal>,
    kind_tag_map: &BTreeMap<String, Vec<&'static str>>,
    identity_index: &TypeSignalIdentityIndex,
) -> Vec<TypeSignal> {
    use domain::tddd::signal_evaluator::region::SignalRegion;

    let mut acc: HashMap<String, AccEntry> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    for signal in signals {
        let name = signal.item_name();
        let confidence = signal_kind_to_confidence(signal.signal());
        let found_in_c = !matches!(
            signal.region(),
            SignalRegion::SMinusC_Add
                | SignalRegion::SMinusC_Modify
                | SignalRegion::SMinusC_Reference
                | SignalRegion::DMinusC
        );

        if let Some(sep) = name.find(": ") {
            let raw_owner = &name[..sep];
            let trait_part = &name[sep + 2..];
            let candidates = identity_index.owner_candidates(raw_owner);
            let ambiguous = candidates.as_ref().is_some_and(|keys| keys.len() > 1);
            let owners = candidates.map_or_else(|| vec![raw_owner.to_owned()], |keys| keys.clone());

            for owner in owners {
                let entry = acc.entry(owner.clone()).or_insert_with(|| {
                    order.push(owner.clone());
                    (
                        if ambiguous { ConfidenceSignal::Yellow } else { ConfidenceSignal::Blue },
                        true,
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                    )
                });
                if ambiguous {
                    entry.0 = worse_signal(entry.0, ConfidenceSignal::Yellow);
                }

                if signal.region() != SignalRegion::DMinusC {
                    entry.0 = worse_signal(entry.0, confidence);
                    match signal.region() {
                        SignalRegion::SIntersectC_Match_Add
                        | SignalRegion::SIntersectC_Match_Modify => {
                            entry.2.push(trait_part.to_owned());
                        }
                        SignalRegion::CMinusSUnionD => {
                            entry.4.push(trait_part.to_owned());
                        }
                        _ => {
                            entry.3.push(trait_part.to_owned());
                        }
                    }
                }
            }
        } else {
            if kind_tag_map.contains_key(name) {
                record_plain_signal(
                    &mut acc,
                    &mut order,
                    name.to_owned(),
                    confidence,
                    found_in_c,
                    false,
                );
            } else {
                match identity_index.declaration_candidates(name) {
                    Some(keys) if keys.len() > 1 => {
                        for key in keys.clone() {
                            record_plain_signal(
                                &mut acc, &mut order, key, confidence, found_in_c, true,
                            );
                        }
                    }
                    Some(keys) if keys.len() == 1 => {
                        if let Some(key) = keys.first() {
                            record_plain_signal(
                                &mut acc,
                                &mut order,
                                key.clone(),
                                confidence,
                                found_in_c,
                                false,
                            );
                        }
                    }
                    _ => record_plain_signal(
                        &mut acc,
                        &mut order,
                        name.to_owned(),
                        confidence,
                        found_in_c,
                        false,
                    ),
                }
            }
        }
    }

    for name in kind_tag_map.keys() {
        let entry_name =
            stored_entry_key_for_kind_name(name, identity_index).unwrap_or_else(|| name.clone());
        acc.entry(entry_name.clone()).or_insert_with(|| {
            order.push(entry_name);
            (ConfidenceSignal::Blue, true, Vec::new(), Vec::new(), Vec::new())
        });
    }

    order
        .into_iter()
        .flat_map(|name| {
            let Some((sig, found_type, found_items, missing_items, extra_items)) =
                acc.remove(&name)
            else {
                return Vec::new();
            };
            let kind_tags = kind_tags_for_entry(name.as_str(), kind_tag_map, identity_index);
            if kind_tags.is_empty() {
                return vec![TypeSignal::new(
                    name,
                    "unknown",
                    sig,
                    found_type,
                    found_items,
                    missing_items,
                    extra_items,
                )];
            }
            let is_collision = kind_tags.len() > 1;
            kind_tags
                .iter()
                .map(|&kind_tag| {
                    let effective_signal = if is_collision {
                        worse_signal(sig, ConfidenceSignal::Yellow)
                    } else {
                        sig
                    };
                    TypeSignal::new(
                        name.clone(),
                        kind_tag,
                        effective_signal,
                        found_type,
                        found_items.clone(),
                        missing_items.clone(),
                        extra_items.clone(),
                    )
                })
                .collect()
        })
        .collect()
}

fn record_plain_signal(
    acc: &mut HashMap<String, AccEntry>,
    order: &mut Vec<String>,
    name: String,
    confidence: ConfidenceSignal,
    found_in_c: bool,
    ambiguous: bool,
) {
    let initial_signal = if ambiguous { ConfidenceSignal::Yellow } else { confidence };
    let entry = acc.entry(name.clone()).or_insert_with(|| {
        order.push(name);
        (initial_signal, found_in_c, Vec::new(), Vec::new(), Vec::new())
    });
    entry.0 = worse_signal(entry.0, confidence);
    entry.1 = entry.1 || found_in_c;
}

fn kind_tags_for_entry(
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

fn stored_entry_key_for_kind_name(
    kind_name: &str,
    identity_index: &TypeSignalIdentityIndex,
) -> Option<String> {
    match identity_index.declaration_candidates(kind_name)?.as_slice() {
        [entry_key] => Some(entry_key.clone()),
        _ => None,
    }
}

fn signal_kind_to_confidence(kind: ThreeWaySignalKind) -> ConfidenceSignal {
    match kind {
        ThreeWaySignalKind::Blue => ConfidenceSignal::Blue,
        ThreeWaySignalKind::Yellow => ConfidenceSignal::Yellow,
        ThreeWaySignalKind::Red => ConfidenceSignal::Red,
        ThreeWaySignalKind::Skip => ConfidenceSignal::Yellow,
    }
}

fn worse_signal(a: ConfidenceSignal, b: ConfidenceSignal) -> ConfidenceSignal {
    match (a, b) {
        (ConfidenceSignal::Red, _) | (_, ConfidenceSignal::Red) => ConfidenceSignal::Red,
        (ConfidenceSignal::Yellow, _) | (_, ConfidenceSignal::Yellow) => ConfidenceSignal::Yellow,
        _ => ConfidenceSignal::Blue,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic, clippy::useless_vec)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use super::{
        TypeSignalIdentityIndex, add_deletion_identity, add_entry_identity,
        build_type_signal_identity_index, build_type_signals_from_report,
    };
    use domain::tddd::LayerId;
    use domain::tddd::catalogue_v2::roles::ItemAction;
    use domain::tddd::catalogue_v2::{
        CatalogueDocument, CatalogueEntryKey, CrateName, DeletionRecord, ModulePath,
    };
    use domain::tddd::signal_evaluator::region::{SignalRegion, ThreeWaySignal};

    #[test]
    fn test_build_type_signals_joins_short_impl_owner_to_qualified_entry() {
        let mut index = TypeSignalIdentityIndex::default();
        index.add_alias(
            "CatalogueLinterError",
            "domain::tddd::catalogue_linter::CatalogueLinterError",
        );

        let mut kinds = std::collections::BTreeMap::new();
        kinds.insert(
            "domain::tddd::catalogue_linter::CatalogueLinterError".to_owned(),
            vec!["error_type"],
        );
        let signals = vec![ThreeWaySignal::new(
            "CatalogueLinterError: From<TypeRefPathExtractionError>".to_owned(),
            SignalRegion::SIntersectC_Match_Add,
        )];

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        assert_eq!(built.len(), 1);
        assert_eq!(built[0].type_name(), "domain::tddd::catalogue_linter::CatalogueLinterError");
        assert_eq!(built[0].found_items(), &["From<TypeRefPathExtractionError>"]);
    }

    #[test]
    fn test_build_type_signals_joins_external_impl_owner_to_for_type_key() {
        let mut index = TypeSignalIdentityIndex::default();
        index.add_impl_alias("Arc", "std::sync::Arc<T>");

        let signals = vec![ThreeWaySignal::new(
            "Arc: TypeRefPathExtractorPort".to_owned(),
            SignalRegion::SIntersectC_Match_Add,
        )];

        let built = build_type_signals_from_report(
            signals.iter(),
            &std::collections::BTreeMap::new(),
            &index,
        );

        assert_eq!(built.len(), 1);
        assert_eq!(built[0].type_name(), "std::sync::Arc<T>");
    }

    #[test]
    fn test_build_identity_index_joins_generic_impl_owner_to_declaration_identity() {
        use domain::tddd::LayerId;
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
        use domain::tddd::catalogue_v2::entries::TypeEntry;
        use domain::tddd::catalogue_v2::methods::MethodGenericParam;
        use domain::tddd::catalogue_v2::roles::DataRole;
        use domain::tddd::catalogue_v2::{ParamName, TraitImplDeclV2, TypeRef};
        use rustdoc_types::{Id, ItemKind, ItemSummary};

        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").expect("valid crate name"),
            LayerId::try_new("domain").expect("valid layer"),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("domain::alpha::Wrapper".to_owned())
                .expect("valid declaration key"),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                ModulePath::from_segments(vec!["alpha".to_owned()]).expect("valid module path"),
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.push_trait_impl(TraitImplDeclV2::from_parts(
            ItemAction::Add,
            TypeRef::new("domain::ports::Port".to_owned()).expect("valid trait ref"),
            TypeRef::new("domain::alpha::Wrapper<T>".to_owned()).expect("valid impl owner"),
            vec![MethodGenericParam {
                name: ParamName::new("T").expect("valid generic parameter"),
                bounds: vec![],
            }],
            vec![],
        ));

        let rustdoc_paths = HashMap::from([(
            Id(1),
            ItemSummary {
                crate_id: 0,
                path: vec!["domain".to_owned(), "alpha".to_owned(), "Wrapper".to_owned()],
                kind: ItemKind::Struct,
            },
        )]);
        for rustdoc_paths in [rustdoc_paths, HashMap::new()] {
            let index = build_type_signal_identity_index(&catalogue, &rustdoc_paths)
                .expect("generic impl owner identity indexes successfully");

            let signals = vec![ThreeWaySignal::new(
                "domain::alpha::Wrapper<T>: domain::ports::Port".to_owned(),
                SignalRegion::SIntersectC_Match_Add,
            )];
            let kinds = BTreeMap::from([("domain::alpha::Wrapper".to_owned(), vec!["struct"])]);
            let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

            assert_eq!(built.len(), 1);
            assert_eq!(built[0].type_name(), "domain::alpha::Wrapper");
            assert_eq!(built[0].kind_tag(), "struct");
            assert_eq!(built[0].found_items(), &["domain::ports::Port"]);
        }
    }

    #[test]
    fn test_build_type_signals_duplicate_module_impl_owners_join_independently() {
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
        use domain::tddd::catalogue_v2::entries::{TraitEntry, TypeEntry};
        use domain::tddd::catalogue_v2::methods::MethodGenericParam;
        use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole};
        use domain::tddd::catalogue_v2::traits::TraitImplDeclV2;
        use domain::tddd::catalogue_v2::{ParamName, TypeRef};
        use rustdoc_types::{Id, ItemKind, ItemSummary};

        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").expect("valid crate name"),
            LayerId::try_new("domain").expect("valid layer"),
        );
        for module in ["alpha", "beta"] {
            let module_path =
                ModulePath::from_segments(vec![module.to_owned()]).expect("valid module path");
            catalogue.insert_type(
                CatalogueEntryKey::try_new(format!("{module}::Input")).expect("valid type key"),
                TypeEntry::new(
                    ItemAction::Add,
                    DataRole::value_object(),
                    TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                    vec![],
                    vec![],
                    vec![],
                    module_path.clone(),
                    None,
                    vec![],
                    vec![],
                ),
            );
            catalogue.insert_trait(
                CatalogueEntryKey::try_new(format!("{module}::Port")).expect("valid trait key"),
                TraitEntry::new(
                    ItemAction::Add,
                    ContractRole::SpecificationPort,
                    vec![],
                    vec![],
                    vec![],
                    vec![],
                    vec![],
                    vec![],
                    module_path,
                    None,
                    vec![],
                    vec![],
                ),
            );
            catalogue.push_trait_impl(TraitImplDeclV2::from_parts(
                ItemAction::Add,
                TypeRef::new(format!("domain::{module}::Port<domain::{module}::Input>"))
                    .expect("valid qualified trait ref"),
                TypeRef::new(format!("domain::{module}::Input<T>")).expect("valid generic owner"),
                vec![MethodGenericParam {
                    name: ParamName::new("T").expect("valid generic name"),
                    bounds: vec![],
                }],
                vec![],
            ));
        }

        let rustdoc_paths = HashMap::from([
            (
                Id(1),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "alpha".to_owned(), "Input".to_owned()],
                    kind: ItemKind::Struct,
                },
            ),
            (
                Id(2),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "beta".to_owned(), "Input".to_owned()],
                    kind: ItemKind::Struct,
                },
            ),
            (
                Id(3),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "alpha".to_owned(), "Port".to_owned()],
                    kind: ItemKind::Trait,
                },
            ),
            (
                Id(4),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "beta".to_owned(), "Port".to_owned()],
                    kind: ItemKind::Trait,
                },
            ),
        ]);
        let identity_index = build_type_signal_identity_index(&catalogue, &rustdoc_paths)
            .expect("duplicate module identities must be indexable");
        let signals = [
            ThreeWaySignal::new(
                "domain::alpha::Input: domain::alpha::Port<domain::alpha::Input>".to_owned(),
                SignalRegion::SIntersectC_Match_Add,
            ),
            ThreeWaySignal::new(
                "domain::beta::Input: domain::beta::Port<domain::beta::Input>".to_owned(),
                SignalRegion::SIntersectC_Match_Add,
            ),
        ];
        let kinds = BTreeMap::from([
            ("alpha::Input".to_owned(), vec!["struct"]),
            ("beta::Input".to_owned(), vec!["struct"]),
        ]);

        let built = build_type_signals_from_report(signals.iter(), &kinds, &identity_index);
        assert_eq!(built.len(), 2, "each qualified owner must produce one signal");
        for (module, other_module) in [("alpha", "beta"), ("beta", "alpha")] {
            let signal = built
                .iter()
                .find(|signal| signal.type_name() == format!("{module}::Input"))
                .expect("qualified owner must remain a separate entry");
            assert_eq!(signal.signal(), domain::ConfidenceSignal::Blue);
            assert_eq!(
                signal.found_items(),
                &[format!("domain::{module}::Port<domain::{module}::Input>")]
            );
            assert!(
                signal
                    .found_items()
                    .iter()
                    .all(|item| !item.contains(&format!("domain::{other_module}::"))),
                "{module} owner must not receive {other_module} impl data: {:?}",
                signal.found_items()
            );
        }
    }

    #[test]
    fn test_build_type_signals_plain_declaration_ignores_external_impl_alias() {
        let mut index = TypeSignalIdentityIndex::default();
        index.add_alias("Shared", "domain::alpha::Shared");
        index.add_impl_alias("Shared", "external::Shared");
        let mut kinds = std::collections::BTreeMap::new();
        kinds.insert("domain::alpha::Shared".to_owned(), vec!["struct"]);
        let signals =
            vec![ThreeWaySignal::new("Shared".to_owned(), SignalRegion::SIntersectC_Match_Add)];

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        assert_eq!(built.len(), 1);
        assert_eq!(built[0].type_name(), "domain::alpha::Shared");
        assert_eq!(built[0].signal(), domain::ConfidenceSignal::Blue);
    }

    #[test]
    fn test_build_type_signals_prefers_exact_impl_owner_over_declaration_alias() {
        let mut index = TypeSignalIdentityIndex::default();
        index.add_alias("other::Thing", "domain::other::Thing");
        index.add_impl_alias("other::Thing", "other::Thing");

        let mut kinds = std::collections::BTreeMap::new();
        kinds.insert("domain::other::Thing".to_owned(), vec!["struct"]);
        let signals = vec![ThreeWaySignal::new(
            "other::Thing: LocalTrait".to_owned(),
            SignalRegion::SIntersectC_Match_Add,
        )];

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        let local = built
            .iter()
            .find(|signal| signal.type_name() == "domain::other::Thing")
            .expect("declared local entry remains in the output");
        assert!(local.found_items().is_empty());
        let external = built
            .iter()
            .find(|signal| signal.type_name() == "other::Thing")
            .expect("exact impl owner receives the impl signal");
        assert_eq!(external.found_items(), ["LocalTrait"]);
    }

    #[test]
    fn test_build_type_signals_marks_ambiguous_short_impl_owner_yellow_for_each_candidate() {
        let mut index = TypeSignalIdentityIndex::default();
        index.add_alias("Shared", "domain::alpha::Shared");
        index.add_alias("Shared", "domain::beta::Shared");

        let mut kinds = std::collections::BTreeMap::new();
        kinds.insert("domain::alpha::Shared".to_owned(), vec!["struct"]);
        kinds.insert("domain::beta::Shared".to_owned(), vec!["struct"]);
        let signals = vec![
            ThreeWaySignal::new(
                "domain::alpha::Shared".to_owned(),
                SignalRegion::SIntersectC_Match_Add,
            ),
            ThreeWaySignal::new(
                "domain::beta::Shared".to_owned(),
                SignalRegion::SIntersectC_Match_Add,
            ),
            ThreeWaySignal::new("Shared: Clone".to_owned(), SignalRegion::SIntersectC_Match_Add),
        ];

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        assert_eq!(built.len(), 2);
        assert!(built.iter().all(|signal| signal.signal() == domain::ConfidenceSignal::Yellow));
        assert!(built.iter().all(|signal| signal.found_items() == ["Clone"]));
    }

    #[test]
    fn test_build_type_signals_does_not_short_fallback_for_unknown_qualified_owner() {
        let mut index = TypeSignalIdentityIndex::default();
        index.add_alias("Shared", "domain::alpha::Shared");
        index.add_alias("Shared", "domain::beta::Shared");

        let signals = vec![ThreeWaySignal::new(
            "domain::gamma::Shared: Clone".to_owned(),
            SignalRegion::SIntersectC_Match_Add,
        )];
        let mut kinds = std::collections::BTreeMap::new();
        kinds.insert("domain::alpha::Shared".to_owned(), vec!["struct"]);
        kinds.insert("domain::beta::Shared".to_owned(), vec!["struct"]);

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        let gamma = built
            .iter()
            .find(|signal| signal.type_name() == "domain::gamma::Shared")
            .expect("unknown qualified owner remains a distinct signal");
        assert_eq!(gamma.found_items(), ["Clone"]);
        assert!(
            built
                .iter()
                .filter(|signal| {
                    matches!(signal.type_name(), "domain::alpha::Shared" | "domain::beta::Shared")
                })
                .all(|signal| signal.found_items().is_empty())
        );
    }

    #[test]
    fn test_build_type_signals_nested_supertrait_joins_qualified_kind_alias_to_stored_entry() {
        let mut index = TypeSignalIdentityIndex::default();
        index.add_alias("usecase::chain::traits::SoTChain", "SoTChain");
        let kinds = BTreeMap::from([(
            "usecase::chain::traits::SoTChain".to_owned(),
            vec!["secondary_port"],
        )]);
        let signals = vec![ThreeWaySignal::new(
            "usecase::chain::traits::SoTChain: ChainIdentity".to_owned(),
            SignalRegion::SIntersectC_Match_Add,
        )];

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);

        assert_eq!(built.len(), 1);
        assert_eq!(built[0].type_name(), "SoTChain");
        assert_eq!(built[0].kind_tag(), "secondary_port");
        assert_eq!(built[0].signal(), domain::ConfidenceSignal::Blue);
        assert_eq!(built[0].found_items(), &["ChainIdentity"]);
    }

    #[test]
    fn test_build_identity_index_preserves_unresolved_add_entry_alias() {
        let mut index = TypeSignalIdentityIndex::default();
        let crate_name = CrateName::new("domain").expect("valid crate name");

        add_entry_identity(
            &mut index,
            &crate_name,
            "domain::new::Added",
            &ModulePath::root(),
            ItemAction::Add,
            &HashMap::new(),
        )
        .expect("unimplemented add entry remains indexable");

        assert_eq!(
            index.declaration_candidates("domain::new::Added"),
            Some(vec!["domain::new::Added".to_owned()])
        );
        assert_eq!(
            index.declaration_candidates("Added"),
            Some(vec!["domain::new::Added".to_owned()])
        );
    }

    #[test]
    fn test_build_identity_index_includes_type_and_trait_deletion_aliases() {
        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").expect("valid crate name"),
            LayerId::try_new("domain").expect("valid layer"),
        );
        catalogue.push_deletion(DeletionRecord::Type {
            name: CatalogueEntryKey::try_new("RemovedType".to_owned()).expect("valid key"),
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        catalogue.push_deletion(DeletionRecord::Trait {
            name: CatalogueEntryKey::try_new("domain::old::RemovedTrait".to_owned())
                .expect("valid key"),
            spec_refs: vec![],
            informal_grounds: vec![],
        });

        let index = build_type_signal_identity_index(&catalogue, &HashMap::new())
            .expect("deletion identities remain indexable");

        assert_eq!(
            index.declaration_candidates("RemovedType"),
            Some(vec!["RemovedType".to_owned()])
        );
        assert_eq!(
            index.declaration_candidates("domain::old::RemovedTrait"),
            Some(vec!["domain::old::RemovedTrait".to_owned()])
        );
    }

    #[test]
    fn test_add_deletion_identity_preserves_bare_catalogue_key() {
        let mut index = TypeSignalIdentityIndex::default();
        let crate_name = CrateName::new("infrastructure").expect("valid crate name");

        add_deletion_identity(
            &mut index,
            &crate_name,
            "CatalogueToExtendedCrateCodecError",
            &HashMap::new(),
        )
        .expect("legacy bare deletion key remains indexable");

        assert_eq!(
            index.declaration_candidates("CatalogueToExtendedCrateCodecError"),
            Some(vec!["CatalogueToExtendedCrateCodecError".to_owned()])
        );
    }
}
