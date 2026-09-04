//! System-backed, read-only source for signal-report occurrences.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use domain::review_v2::types::FilePath;
use domain::tddd::catalogue_v2::{CatalogueItemNamespace, DeletionRecord, TdddLayerBindingsPort};
use domain::tddd::signal_evaluator::ThreeWaySignalIdentity;
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

use crate::capability_exec::bounded_read_utf8_file;
use crate::git_cli::{SystemGitRepo, isolated_bounded_git_output};
use crate::tddd::{
    catalogue_document_codec::CatalogueDocumentCodec, catalogue_spec_signals_codec,
    tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter, type_signals_codec,
};
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::catalogue_spec_signals::{
    compute_catalogue_declaration_hash, compute_catalogue_entry_hash,
};
use crate::verify::tddd_layers;

mod coverage;
mod freshness;
use coverage::{impl_catalog_identity, is_safe_signal_line_text, validate_impl_catalog_coverage};
use freshness::validate_impl_catalog_freshness;

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
        let mut occurrences = Vec::new();
        for binding in bindings {
            let signals_path =
                track_file(root, track_id, &binding.signal_file(), SignalReportChain::ImplCatalog)?;
            let signal_text = read_text(root, &signals_path, SignalReportChain::ImplCatalog)?;
            let document = type_signals_codec::decode(&signal_text).map_err(|_| {
                SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)
            })?;
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
            // A baseline recapture can change reverse filtering without touching
            // the catalogue, so the cached signals must still match the local baseline.
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
            validate_impl_catalog_freshness(
                root,
                &signals_path,
                document.cache_key().head_commit(),
                signal_text.as_bytes(),
            )?;
            validate_impl_catalog_coverage(&catalogue, &document)?;
            let location = relative(root, &signals_path, SignalReportChain::ImplCatalog)?;
            for signal in document.signals() {
                let Some(level) = report_level(signal.signal()) else { continue };
                let reason = impl_catalog_reason(signal);
                let identity = impl_catalog_identity(signal);
                occurrences.push(occurrence(
                    SignalReportChain::ImplCatalog,
                    level,
                    format!("{}:{}:{}", binding.layer_id, signal.kind_tag(), identity),
                    format!("{}#{}:{}", binding.catalogue_file, signal.kind_tag(), identity),
                    reason,
                    location.clone(),
                )?);
            }
        }
        Ok(occurrences)
    }
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
#[path = "mod_tests.rs"]
mod tests;
