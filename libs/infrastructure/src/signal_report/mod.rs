//! System-backed, read-only source for signal-report occurrences.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use domain::review_v2::types::FilePath;
use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::TdddLayerBindingsPort;
use domain::{
    AdrDecisionCommon, AdrDecisionEntry, ConfidenceSignal, DecisionGrounds, NonEmptyString,
    TrackId, evaluate_adr_decision,
};
use usecase::catalogue_traversal::iter_catalogue_entries;
use usecase::signal_report::{
    SignalReportChain, SignalReportEntryId, SignalReportError, SignalReportLevel,
    SignalReportLocation, SignalReportOccurrence, SignalReportReason, SignalReportReference,
    SignalReportSourcePort,
};
use usecase::tddd_feature_declaration::TdddActualFeatureDeclarationPort;

use crate::capability_exec::bounded_read_utf8_file;
use crate::git_cli::{SystemGitRepo, isolated_bounded_git_output};
use crate::tddd::{
    catalogue_document_codec::CatalogueDocumentCodec, catalogue_spec_signals_codec,
    feature_declaration_adapter::FsTdddFeatureDeclarationAdapter,
    tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter, type_signals_codec,
};
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::catalogue_spec_signals::{
    compute_catalogue_declaration_hash, compute_catalogue_entry_hash,
};
use crate::verify::tddd_layers;

/// Secondary adapter that reads persisted signal artifacts and derives the two
/// non-persisted occurrence chains in memory.
pub struct SystemSignalReportSourceAdapter;

const MAX_BRANCH_NAME_BYTES: usize = 4 * 1024;
const MAX_ADR_FILES: usize = 1_024;
const MAX_ADR_TOTAL_BYTES: usize = 8 * 1024 * 1024;
const MAX_TYPE_BASELINE_BYTES: u64 = 64 * 1024 * 1024;

impl SystemSignalReportSourceAdapter {
    /// Creates the system-backed signal report source.
    // The TDDD catalogue exposes this constructor but intentionally does not
    // declare `Default`; adding that impl would be an extra public contract.
    #[allow(clippy::new_without_default)]
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn context(chain: SignalReportChain) -> Result<(PathBuf, TrackId), SignalReportError> {
        let current_dir = std::env::current_dir()
            .and_then(|path| path.canonicalize())
            .map_err(|_| SignalReportError::SourceUnavailable(chain))?;
        let repo = SystemGitRepo::discover_from_isolated(&current_dir)
            .map_err(|_| SignalReportError::SourceUnavailable(chain))?;
        let root =
            repo.root().canonicalize().map_err(|_| SignalReportError::SourceUnavailable(chain))?;
        if !current_dir.starts_with(&root) {
            return Err(SignalReportError::SourceUnavailable(chain));
        }
        let branch_output = isolated_bounded_git_output(
            &root,
            &["rev-parse", "--abbrev-ref", "HEAD"],
            MAX_BRANCH_NAME_BYTES,
        )
        .map_err(|_| SignalReportError::SourceUnavailable(chain))?;
        if !branch_output.status.success() {
            return Err(SignalReportError::SourceUnavailable(chain));
        }
        let branch = String::from_utf8(branch_output.stdout)
            .map_err(|_| SignalReportError::SourceUnavailable(chain))?;
        let branch = branch.trim();
        let track =
            branch.strip_prefix("track/").ok_or(SignalReportError::SourceUnavailable(chain))?;
        let track_id =
            TrackId::try_new(track).map_err(|_| SignalReportError::SourceUnavailable(chain))?;
        Ok((root, track_id))
    }

    fn load_chain(
        workspace_root: &Path,
        track_id: &TrackId,
        chain: SignalReportChain,
    ) -> Result<Vec<SignalReportOccurrence>, SignalReportError> {
        match chain {
            SignalReportChain::AdrUser => Self::derive_adr_user(workspace_root),
            SignalReportChain::SpecAdr => Self::derive_spec_adr(workspace_root, track_id),
            SignalReportChain::CatalogSpec => Self::read_catalog_spec(workspace_root, track_id),
            SignalReportChain::ImplCatalog => Self::read_impl_catalog(workspace_root, track_id),
        }
    }

    fn derive_adr_user(root: &Path) -> Result<Vec<SignalReportOccurrence>, SignalReportError> {
        let adr_dir = root.join("knowledge/adr");
        reject_symlinks_below(&adr_dir, root)
            .map_err(|_| SignalReportError::SourceUnavailable(SignalReportChain::AdrUser))?;
        let paths = bounded_adr_paths(&adr_dir)?;
        let mut occurrences = Vec::new();
        let mut total_bytes = 0_usize;
        for path in paths {
            let text = read_text(root, &path, SignalReportChain::AdrUser)?;
            total_bytes = total_bytes
                .checked_add(text.len())
                .ok_or(SignalReportError::SourceUnavailable(SignalReportChain::AdrUser))?;
            if total_bytes > MAX_ADR_TOTAL_BYTES {
                return Err(SignalReportError::SourceUnavailable(SignalReportChain::AdrUser));
            }
            let document = crate::adr_decision::parse_adr_frontmatter(&text)
                .map_err(|_| SignalReportError::SourceUnavailable(SignalReportChain::AdrUser))?;
            let location = relative(root, &path, SignalReportChain::AdrUser)?;
            for entry in document.decisions() {
                let grounds = evaluate_adr_decision(entry.clone());
                let Some((level, reference, reason)) = adr_occurrence(entry, grounds) else {
                    continue;
                };
                occurrences.push(occurrence(
                    SignalReportChain::AdrUser,
                    level,
                    format!("{}#{}", document.adr_id(), common(entry).id()),
                    reference,
                    reason,
                    location.clone(),
                )?);
            }
        }
        Ok(occurrences)
    }

    fn derive_spec_adr(
        root: &Path,
        track_id: &TrackId,
    ) -> Result<Vec<SignalReportOccurrence>, SignalReportError> {
        let path = track_file(root, track_id, "spec.json", SignalReportChain::SpecAdr)?;
        let document =
            crate::spec::codec::decode(&read_text(root, &path, SignalReportChain::SpecAdr)?)
                .map_err(|_| SignalReportError::SourceUnavailable(SignalReportChain::SpecAdr))?;
        let location = relative(root, &path, SignalReportChain::SpecAdr)?;
        let requirements = document
            .goal()
            .iter()
            .chain(document.scope().in_scope())
            .chain(document.scope().out_of_scope())
            .chain(document.constraints())
            .chain(document.acceptance_criteria());
        let mut occurrences = Vec::new();
        for requirement in requirements {
            let (level, reference, reason) = match requirement.signal() {
                ConfidenceSignal::Yellow => {
                    let ground = requirement
                        .informal_grounds()
                        .first()
                        .ok_or(SignalReportError::SourceUnavailable(SignalReportChain::SpecAdr))?;
                    (
                        SignalReportLevel::Yellow,
                        format!("{}:{}", ground.kind, ground.summary),
                        "unpromoted informal ground".to_owned(),
                    )
                }
                ConfidenceSignal::Red => (
                    SignalReportLevel::Red,
                    "no ADR reference".to_owned(),
                    "requirement has neither ADR references nor informal grounds".to_owned(),
                ),
                ConfidenceSignal::Blue => continue,
                _ => (
                    SignalReportLevel::Red,
                    "unknown signal level".to_owned(),
                    "requirement evaluated to an unsupported non-blue signal level".to_owned(),
                ),
            };
            occurrences.push(occurrence(
                SignalReportChain::SpecAdr,
                level,
                requirement.id().as_ref().to_owned(),
                reference,
                reason,
                location.clone(),
            )?);
        }
        Ok(occurrences)
    }

    fn read_catalog_spec(
        root: &Path,
        track_id: &TrackId,
    ) -> Result<Vec<SignalReportOccurrence>, SignalReportError> {
        let bindings = tddd_layers::load_tddd_layers(&root.join("architecture-rules.json"), root)
            .map_err(|_| {
            SignalReportError::SourceUnavailable(SignalReportChain::CatalogSpec)
        })?;
        let mut occurrences = Vec::new();
        for binding in bindings {
            if !binding.catalogue_spec_signal_enabled() {
                continue;
            }
            let catalogue_path = track_file(
                root,
                track_id,
                binding.catalogue_file(),
                SignalReportChain::CatalogSpec,
            )?;
            let catalogue_text = read_text(root, &catalogue_path, SignalReportChain::CatalogSpec)?;
            let catalogue = CatalogueDocumentCodec::decode(&catalogue_text, binding.layer_id())
                .map_err(|_| {
                    SignalReportError::SourceUnavailable(SignalReportChain::CatalogSpec)
                })?;
            let signals_path = track_file(
                root,
                track_id,
                &binding.catalogue_spec_signal_file(),
                SignalReportChain::CatalogSpec,
            )?;
            let signals = catalogue_spec_signals_codec::decode(&read_text(
                root,
                &signals_path,
                SignalReportChain::CatalogSpec,
            )?)
            .map_err(|_| SignalReportError::SourceUnavailable(SignalReportChain::CatalogSpec))?;
            validate_catalogue_spec_freshness(&catalogue_text, &catalogue, &signals)?;
            for (entry, signal) in iter_catalogue_entries(&catalogue).zip(signals.signals) {
                let level = report_level(signal.signal);
                let Some(level) = level else { continue };
                let (reference, reason) =
                    catalogue_context(entry.spec_refs, entry.informal_grounds);
                occurrences.push(occurrence(
                    SignalReportChain::CatalogSpec,
                    level,
                    format!("{}:{}", binding.layer_id(), entry.section_key),
                    reference,
                    reason,
                    relative(root, &catalogue_path, SignalReportChain::CatalogSpec)?,
                )?);
            }
        }
        Ok(occurrences)
    }

    fn read_impl_catalog(
        root: &Path,
        track_id: &TrackId,
    ) -> Result<Vec<SignalReportOccurrence>, SignalReportError> {
        let bindings = FsTdddLayerBindingsAdapter::new()
            .load(root, None)
            .map_err(|_| SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog))?;
        let track_dir = root.join("track/items").join(track_id.as_ref());
        let feature_declaration = FsTdddFeatureDeclarationAdapter::new()
            .load_for_actual(&track_dir, root, &bindings)
            .map_err(|_| SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog))?;
        let mut occurrences = Vec::new();
        for binding in bindings {
            let layer_id = LayerId::try_new(binding.layer_id.clone()).map_err(|_| {
                SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)
            })?;
            let features = feature_declaration.features_for(&layer_id).map_err(|_| {
                SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)
            })?;
            let target_crate = match binding.targets.as_slice() {
                [target] => target.as_str(),
                _ => {
                    return Err(SignalReportError::SourceUnavailable(
                        SignalReportChain::ImplCatalog,
                    ));
                }
            };
            let signals_path =
                track_file(root, track_id, &binding.signal_file(), SignalReportChain::ImplCatalog)?;
            let document = type_signals_codec::decode(&read_text(
                root,
                &signals_path,
                SignalReportChain::ImplCatalog,
            )?)
            .map_err(|_| SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog))?;
            let catalogue_path = track_file(
                root,
                track_id,
                &binding.catalogue_file,
                SignalReportChain::ImplCatalog,
            )?;
            let catalogue_text = read_text(root, &catalogue_path, SignalReportChain::ImplCatalog)?;
            let catalogue = CatalogueDocumentCodec::decode(&catalogue_text, &binding.layer_id)
                .map_err(|_| {
                    SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)
                })?;
            if *document.cache_key().declaration_hash()
                != type_signals_codec::declaration_hash(catalogue_text.as_bytes())
            {
                return Err(SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog));
            }
            let implementation_input_hash =
                crate::tddd::type_signals_evaluator::inputs::hash_workspace_inputs(
                    root,
                    target_crate,
                    features,
                )
                .map_err(|_| {
                    SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)
                })?;
            if *document.cache_key().implementation_input_hash() != implementation_input_hash {
                return Err(SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog));
            }
            // A baseline recapture can change reverse filtering without touching
            // the catalogue or the implementation, so the cached signals are only
            // current when all three cache-key hashes match the current inputs.
            let baseline_text = crate::track_artifact::read_track_artifact(
                &root.join("track/items"),
                track_id,
                &binding.baseline_file,
                MAX_TYPE_BASELINE_BYTES,
            )
            .map_err(|_| SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog))?;
            if *document.cache_key().baseline_hash()
                != type_signals_codec::baseline_hash(baseline_text.as_bytes())
            {
                return Err(SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog));
            }
            validate_impl_catalog_coverage(&catalogue, &document)?;
            let location = relative(root, &signals_path, SignalReportChain::ImplCatalog)?;
            for signal in document.signals() {
                let Some(level) = report_level(signal.signal()) else { continue };
                let reason = impl_catalog_reason(signal);
                occurrences.push(occurrence(
                    SignalReportChain::ImplCatalog,
                    level,
                    format!("{}:{}:{}", binding.layer_id, signal.kind_tag(), signal.type_name()),
                    format!(
                        "{}#{}:{}",
                        binding.catalogue_file,
                        signal.kind_tag(),
                        signal.type_name()
                    ),
                    reason,
                    location.clone(),
                )?);
            }
        }
        Ok(occurrences)
    }
}

fn validate_impl_catalog_coverage(
    catalogue: &domain::tddd::catalogue_v2::CatalogueDocument,
    document: &domain::TypeSignalsDocument,
) -> Result<(), SignalReportError> {
    use crate::tddd::type_signals_evaluator::signal_tags::{
        contract_role_kind_tag, data_role_kind_tag, function_role_kind_tag,
    };

    let unavailable = || SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog);
    let mut expected = BTreeSet::new();
    for (name, entry) in catalogue.types() {
        expected.insert((
            name.as_str().to_owned(),
            data_role_kind_tag(entry.role(), entry.kind()).to_owned(),
        ));
    }
    for (name, entry) in catalogue.traits() {
        expected
            .insert((name.as_str().to_owned(), contract_role_kind_tag(entry.role()).to_owned()));
    }
    for (path, entry) in catalogue.functions() {
        expected.insert((path.to_string(), function_role_kind_tag(entry.role()).to_owned()));
    }

    let expected_names = expected.iter().map(|(name, _)| name.clone()).collect::<BTreeSet<_>>();
    let mut unknown_names = BTreeSet::new();
    for signal in document.signals() {
        if signal
            .missing_items()
            .iter()
            .chain(signal.extra_items())
            .any(|item| !is_safe_signal_line_text(item))
        {
            return Err(unavailable());
        }
        if signal.is_unknown_kind() {
            if !is_rust_path(signal.type_name())
                || expected_names.contains(signal.type_name())
                || !unknown_names.insert(signal.type_name().to_owned())
            {
                return Err(unavailable());
            }
            continue;
        }
        if !expected.remove(&(signal.type_name().to_owned(), signal.kind_tag().to_owned())) {
            return Err(unavailable());
        }
    }
    if !expected.is_empty() {
        return Err(unavailable());
    }
    Ok(())
}

fn is_safe_signal_line_text(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && !value
            .chars()
            .any(|character| character.is_control() || matches!(character, '\u{2028}' | '\u{2029}'))
}

fn is_rust_path(value: &str) -> bool {
    is_safe_signal_line_text(value)
        && value.split("::").all(|segment| {
            domain::tddd::catalogue_v2::identifiers::Identifier::new(segment).is_ok()
        })
}

fn validate_catalogue_spec_freshness(
    catalogue_text: &str,
    catalogue: &domain::tddd::catalogue_v2::CatalogueDocument,
    signals: &domain::CatalogueSpecSignalsDocument,
) -> Result<(), SignalReportError> {
    let unavailable = || SignalReportError::SourceUnavailable(SignalReportChain::CatalogSpec);
    if signals.catalogue_declaration_hash.to_hex()
        != compute_catalogue_declaration_hash(catalogue_text.as_bytes()).as_digest().as_str()
    {
        return Err(unavailable());
    }

    let entries: Vec<_> = iter_catalogue_entries(catalogue).collect();
    if entries.len() != signals.signals.len() {
        return Err(unavailable());
    }

    for (entry, signal) in entries.iter().zip(&signals.signals) {
        if entry.key != signal.type_name {
            return Err(unavailable());
        }
        let (section, entry_key) = entry.section_key.split_once(':').ok_or_else(unavailable)?;
        let current_entry_hash = compute_catalogue_entry_hash(catalogue_text, section, entry_key)
            .map_err(|_| unavailable())?;
        if signal.entry_hash().to_hex() != current_entry_hash {
            return Err(unavailable());
        }
    }

    Ok(())
}

fn impl_catalog_reason(signal: &domain::TypeSignal) -> String {
    if !signal.found_type() {
        return "catalogue type was not found in implementation".to_owned();
    }

    let mut fragments = Vec::new();
    if !signal.missing_items().is_empty() {
        fragments
            .push(format!("missing implementation items: {}", signal.missing_items().join(", ")));
    }
    if !signal.extra_items().is_empty() {
        fragments
            .push(format!("unexpected implementation items: {}", signal.extra_items().join(", ")));
    }
    if fragments.is_empty() {
        "implementation does not conform to catalogue declaration".to_owned()
    } else {
        fragments.join("; ")
    }
}

impl SignalReportSourcePort for SystemSignalReportSourceAdapter {
    fn load(
        &self,
        chain: SignalReportChain,
    ) -> Result<Vec<SignalReportOccurrence>, SignalReportError> {
        let (root, track_id) = Self::context(chain)?;
        Self::load_chain(&root, &track_id, chain)
    }
}

fn bounded_adr_paths(adr_dir: &Path) -> Result<Vec<PathBuf>, SignalReportError> {
    let entries = std::fs::read_dir(adr_dir)
        .map_err(|_| SignalReportError::SourceUnavailable(SignalReportChain::AdrUser))?;
    let mut paths = Vec::new();
    let mut examined_entries = 0_usize;
    for entry in entries {
        let entry =
            entry.map_err(|_| SignalReportError::SourceUnavailable(SignalReportChain::AdrUser))?;
        examined_entries = examined_entries
            .checked_add(1)
            .ok_or(SignalReportError::SourceUnavailable(SignalReportChain::AdrUser))?;
        if examined_entries > MAX_ADR_FILES {
            return Err(SignalReportError::SourceUnavailable(SignalReportChain::AdrUser));
        }
        let file_type = entry
            .file_type()
            .map_err(|_| SignalReportError::SourceUnavailable(SignalReportChain::AdrUser))?;
        if !file_type.is_file()
            || !entry
                .path()
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        {
            continue;
        }
        paths.push(entry.path());
    }
    paths.sort();
    Ok(paths)
}

fn common(entry: &AdrDecisionEntry) -> &AdrDecisionCommon {
    match entry {
        AdrDecisionEntry::ProposedDecision(value) => &value.common,
        AdrDecisionEntry::AcceptedDecision(value) => &value.common,
        AdrDecisionEntry::ImplementedDecision(value) => &value.common,
        AdrDecisionEntry::SupersededDecision(value) => &value.common,
        AdrDecisionEntry::DeprecatedDecision(value) => &value.common,
    }
}

fn adr_occurrence(
    entry: &AdrDecisionEntry,
    grounds: DecisionGrounds,
) -> Option<(SignalReportLevel, String, String)> {
    match grounds {
        DecisionGrounds::ReviewFindingRef => Some((
            SignalReportLevel::Yellow,
            common(entry).review_finding_ref()?.as_str().to_owned(),
            "decision remains grounded by a review finding".to_owned(),
        )),
        DecisionGrounds::NoGrounds => Some((
            SignalReportLevel::Red,
            "no decision ground reference".to_owned(),
            "decision has neither user nor review-finding ground".to_owned(),
        )),
        DecisionGrounds::UserDecisionRef | DecisionGrounds::Grandfathered => None,
    }
}

fn catalogue_context(
    spec_refs: &[domain::SpecRef],
    informal_grounds: &[domain::InformalGroundRef],
) -> (String, String) {
    if let Some(ground) = informal_grounds.first() {
        return (
            format!("{}:{}", ground.kind, ground.summary),
            "catalogue entry has an unpromoted informal ground".to_owned(),
        );
    }
    if let Some(reference) = spec_refs.first() {
        return (
            format!("{}#{}", reference.file.display(), reference.anchor),
            "persisted catalogue-spec signal is non-blue".to_owned(),
        );
    }
    (
        "no specification reference".to_owned(),
        "catalogue entry has neither specification references nor informal grounds".to_owned(),
    )
}

fn report_level(signal: ConfidenceSignal) -> Option<SignalReportLevel> {
    match signal {
        ConfidenceSignal::Yellow => Some(SignalReportLevel::Yellow),
        ConfidenceSignal::Red => Some(SignalReportLevel::Red),
        ConfidenceSignal::Blue => None,
        _ => Some(SignalReportLevel::Red),
    }
}

fn track_file(
    root: &Path,
    track_id: &TrackId,
    file: &str,
    chain: SignalReportChain,
) -> Result<PathBuf, SignalReportError> {
    let path = root.join("track/items").join(track_id.as_ref()).join(file);
    reject_symlinks_below(&path, root).map_err(|_| SignalReportError::SourceUnavailable(chain))?;
    Ok(path)
}

fn read_text(
    root: &Path,
    path: &Path,
    chain: SignalReportChain,
) -> Result<String, SignalReportError> {
    reject_symlinks_below(path, root).map_err(|_| SignalReportError::SourceUnavailable(chain))?;
    bounded_read_utf8_file(path).map_err(|_| SignalReportError::SourceUnavailable(chain))
}

fn relative(
    root: &Path,
    path: &Path,
    chain: SignalReportChain,
) -> Result<String, SignalReportError> {
    path.strip_prefix(root)
        .map_err(|_| SignalReportError::SourceUnavailable(chain))?
        .to_str()
        .map(str::to_owned)
        .ok_or(SignalReportError::SourceUnavailable(chain))
}

fn occurrence(
    chain: SignalReportChain,
    level: SignalReportLevel,
    entry_id: String,
    reference: String,
    reason: String,
    location: String,
) -> Result<SignalReportOccurrence, SignalReportError> {
    if [&entry_id, &reference, &reason, &location]
        .into_iter()
        .any(|value| !is_safe_signal_line_text(value))
    {
        return Err(SignalReportError::SourceUnavailable(chain));
    }
    let entry_id = NonEmptyString::try_new(entry_id)
        .map_err(|_| SignalReportError::SourceUnavailable(chain))?;
    let reference = NonEmptyString::try_new(reference)
        .map_err(|_| SignalReportError::SourceUnavailable(chain))?;
    let reason =
        NonEmptyString::try_new(reason).map_err(|_| SignalReportError::SourceUnavailable(chain))?;
    let location =
        FilePath::new(location).map_err(|_| SignalReportError::SourceUnavailable(chain))?;
    Ok(SignalReportOccurrence {
        chain,
        level,
        entry_id: SignalReportEntryId::new(entry_id),
        reference: SignalReportReference::new(reference),
        reason: SignalReportReason::new(reason),
        location: SignalReportLocation::new(location),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::fs::File;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::Mutex;

    use super::*;

    static CWD_LOCK: Mutex<()> = Mutex::new(());

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
            assert!(matches!(
                error,
                SignalReportError::SourceUnavailable(SignalReportChain::AdrUser)
            ));
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

    fn fresh_impl_catalog_signals(root: &Path, catalogue_text: &str) -> String {
        let implementation_input_hash =
            crate::tddd::type_signals_evaluator::inputs::hash_workspace_inputs(
                root,
                "infrastructure",
                &[],
            )
            .expect("fixture implementation inputs must hash");
        serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": 4,
            "generated_at": "2026-07-31T00:00:00Z",
            "declaration_hash": type_signals_codec::declaration_hash(catalogue_text.as_bytes())
                .as_digest()
                .as_str(),
            "implementation_input_hash": implementation_input_hash.as_digest().as_str(),
            "baseline_hash": type_signals_codec::baseline_hash(FIXTURE_BASELINE_BYTES)
                .as_digest()
                .as_str(),
            "signals": [{
                "type_name": "SystemSignalReportSourceAdapter",
                "kind_tag": "secondary_adapter",
                "signal": "yellow",
                "found_type": true,
            }],
        }))
        .expect("fixture signals must serialize")
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
        fs::write(root.join("Cargo.lock"), "version = 4\n")
            .expect("fixture lockfile must be written");
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
            .expect("fixture must include the first in-scope ADR reference") =
            serde_json::json!([]);
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
                let metadata =
                    fs::symlink_metadata(&path).expect("fixture metadata must be readable");
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
            .expect("fixture must include the first in-scope ADR reference") =
            serde_json::json!([]);
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
            occurrence.entry_id.to_string()
                == "infrastructure:types:SystemSignalReportSourceAdapter"
                && occurrence.reference.to_string() == "track/items/report-test/spec.json#IN-04"
                && occurrence.reason.to_string() == "persisted catalogue-spec signal is non-blue"
        }));
        assert!(occurrences.iter().any(|occurrence| {
            occurrence.entry_id.to_string()
                == "infrastructure:traits:SystemSignalReportSourceAdapter"
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

        assert!(matches!(
            error,
            SignalReportError::SourceUnavailable(SignalReportChain::CatalogSpec)
        ));
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
        assert!(matches!(
            extra,
            SignalReportError::SourceUnavailable(SignalReportChain::CatalogSpec)
        ));
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

        assert!(matches!(
            error,
            SignalReportError::SourceUnavailable(SignalReportChain::CatalogSpec)
        ));
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
                    "kind_tag": "secondary_adapter",
                    "signal": "yellow",
                    "found_type": true,
                },
                {
                    "type_name": "SystemSignalReportSourceAdapter",
                    "kind_tag": "secondary_port",
                    "signal": "yellow",
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
                == "infrastructure:secondary_adapter:SystemSignalReportSourceAdapter"
                && occurrence.reference.to_string()
                    == "infrastructure-types.json#secondary_adapter:SystemSignalReportSourceAdapter"
        }));
        assert!(occurrences.iter().any(|occurrence| {
            occurrence.entry_id.to_string()
                == "infrastructure:secondary_port:SystemSignalReportSourceAdapter"
                && occurrence.reference.to_string()
                    == "infrastructure-types.json#secondary_port:SystemSignalReportSourceAdapter"
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
                    "kind_tag": "secondary_adapter",
                    "signal": "blue",
                    "found_type": true,
                },
                {
                    "type_name": "ImplementationOnlyType",
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
            }] if entry_id.to_string() == "infrastructure:unknown:ImplementationOnlyType"
                && reference.to_string()
                    == "infrastructure-types.json#unknown:ImplementationOnlyType"
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
                    "kind_tag": "secondary_adapter",
                    "signal": "blue",
                    "found_type": true,
                },
                {
                    "type_name": "Forged\nOccurrence",
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
        let stale_baseline =
            SystemSignalReportSourceAdapter::read_impl_catalog(root.path(), &track)
                .expect_err("changed type baseline must stale the persisted artifact");
        assert!(matches!(
            stale_baseline,
            SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)
        ));
    }

    #[test]
    fn test_signal_report_adapter_accepts_type_baseline_above_capability_exec_limit() {
        let root = tempfile::tempdir().unwrap();
        let track = TrackId::try_new("report-test").unwrap();
        let track_dir = root.path().join("track/items/report-test");
        let catalogue = prepare_catalogue_spec_source(root.path(), &track_dir);
        let baseline =
            vec![b'b'; crate::capability_exec::MAX_CAPABILITY_EXEC_TEXT_BYTES as usize + 1];
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
    fn test_signal_report_adapter_rejects_stale_impl_source_and_feature_inputs() {
        let root = tempfile::tempdir().unwrap();
        let track = TrackId::try_new("report-test").unwrap();
        let track_dir = root.path().join("track/items/report-test");
        let catalogue = prepare_catalogue_spec_source(root.path(), &track_dir);
        let signals_path = track_dir.join("infrastructure-type-signals.json");
        fs::write(&signals_path, fresh_impl_catalog_signals(root.path(), &catalogue)).unwrap();

        let source_path = root.path().join("libs/infrastructure/src/lib.rs");
        fs::write(&source_path, "pub struct ChangedFixture;\n").unwrap();
        let stale_source = SystemSignalReportSourceAdapter::read_impl_catalog(root.path(), &track)
            .expect_err("changed implementation source must stale the persisted artifact");
        assert!(matches!(
            stale_source,
            SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)
        ));

        fs::write(&source_path, "pub struct Fixture;\n").unwrap();
        let selected_feature = "{\n  \"schema_version\": 1,\n  \"layers\": {\n    \"infrastructure\": [\"freshness\"]\n  }\n}\n";
        fs::write(track_dir.join("tddd-features.json"), selected_feature).unwrap();
        fs::write(track_dir.join("tddd-features-baseline.json"), selected_feature).unwrap();
        let stale_features =
            SystemSignalReportSourceAdapter::read_impl_catalog(root.path(), &track)
                .expect_err("changed feature selection must stale the persisted artifact");
        assert!(matches!(
            stale_features,
            SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)
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

        assert!(matches!(
            error,
            SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)
        ));
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
                == "infrastructure:secondary_adapter:SystemSignalReportSourceAdapter"
                && reference.to_string()
                    == "infrastructure-types.json#secondary_adapter:SystemSignalReportSourceAdapter"
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
        assert!(matches!(
            error,
            SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)
        ));

        let catalogue_path =
            fixture.path().join("track/items/report-test/infrastructure-types.json");
        let malformed_catalogue = "{malformed catalogue";
        fs::write(&catalogue_path, malformed_catalogue).unwrap();
        fs::write(&signals_path, fresh_impl_catalog_signals(fixture.path(), malformed_catalogue))
            .unwrap();

        let error = SignalReportSourcePort::load(&adapter, SignalReportChain::ImplCatalog)
            .expect_err("a malformed current catalogue must fail closed even with a matching hash");
        assert!(matches!(
            error,
            SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)
        ));
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

        assert!(matches!(
            error,
            SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)
        ));
    }

    #[test]
    fn test_signal_report_source_port_load_maps_context_error_to_requested_chain() {
        let fixture = tempfile::tempdir().unwrap();
        let _lock = CWD_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let _cwd = CwdGuard::enter(fixture.path());
        let adapter = SystemSignalReportSourceAdapter::new();

        let error = SignalReportSourcePort::load(&adapter, SignalReportChain::CatalogSpec)
            .expect_err("a missing repository context must fail the requested chain");

        assert!(matches!(
            error,
            SignalReportError::SourceUnavailable(SignalReportChain::CatalogSpec)
        ));
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
}
