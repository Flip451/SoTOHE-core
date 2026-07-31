//! System-backed, read-only source for signal-report occurrences.

use std::path::{Path, PathBuf};

use domain::review_v2::types::FilePath;
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
    type_signals_codec,
};
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::tddd_layers;

/// Secondary adapter that reads persisted signal artifacts and derives the two
/// non-persisted occurrence chains in memory.
pub struct SystemSignalReportSourceAdapter;

const MAX_BRANCH_NAME_BYTES: usize = 4 * 1024;
const MAX_ADR_FILES: usize = 1_024;
const MAX_ADR_TOTAL_BYTES: usize = 8 * 1024 * 1024;

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
            let location = relative(root, &path)?;
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
        let location = relative(root, &path)?;
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
            for signal in signals.signals {
                let level = report_level(signal.signal);
                let Some(level) = level else { continue };
                let entry = iter_catalogue_entries(&catalogue)
                    .find(|entry| entry.key == signal.type_name)
                    .ok_or(SignalReportError::SourceUnavailable(SignalReportChain::CatalogSpec))?;
                let (reference, reason) =
                    catalogue_context(entry.spec_refs, entry.informal_grounds);
                occurrences.push(occurrence(
                    SignalReportChain::CatalogSpec,
                    level,
                    format!("{}:{}", binding.layer_id(), signal.type_name),
                    reference,
                    reason,
                    relative(root, &catalogue_path)?,
                )?);
            }
        }
        Ok(occurrences)
    }

    fn read_impl_catalog(
        root: &Path,
        track_id: &TrackId,
    ) -> Result<Vec<SignalReportOccurrence>, SignalReportError> {
        let bindings = tddd_layers::load_tddd_layers(&root.join("architecture-rules.json"), root)
            .map_err(|_| {
            SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog)
        })?;
        let mut occurrences = Vec::new();
        for binding in bindings {
            let signals_path =
                track_file(root, track_id, &binding.signal_file(), SignalReportChain::ImplCatalog)?;
            let document = type_signals_codec::decode(&read_text(
                root,
                &signals_path,
                SignalReportChain::ImplCatalog,
            )?)
            .map_err(|_| SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog))?;
            let location = relative(root, &signals_path)?;
            for signal in document.signals() {
                let Some(level) = report_level(signal.signal()) else { continue };
                let reason = if !signal.found_type() {
                    "catalogue type was not found in implementation".to_owned()
                } else if !signal.missing_items().is_empty() {
                    format!("missing implementation items: {}", signal.missing_items().join(", "))
                } else if !signal.extra_items().is_empty() {
                    format!("unexpected implementation items: {}", signal.extra_items().join(", "))
                } else {
                    "implementation does not conform to catalogue declaration".to_owned()
                };
                occurrences.push(occurrence(
                    SignalReportChain::ImplCatalog,
                    level,
                    format!("{}:{}", binding.layer_id(), signal.type_name()),
                    format!("{}#{}", binding.catalogue_file(), signal.type_name()),
                    reason,
                    location.clone(),
                )?);
            }
        }
        Ok(occurrences)
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

fn relative(root: &Path, path: &Path) -> Result<String, SignalReportError> {
    path.strip_prefix(root)
        .map_err(|_| SignalReportError::SourceUnavailable(SignalReportChain::AdrUser))?
        .to_str()
        .map(str::to_owned)
        .ok_or(SignalReportError::SourceUnavailable(SignalReportChain::AdrUser))
}

fn occurrence(
    chain: SignalReportChain,
    level: SignalReportLevel,
    entry_id: String,
    reference: String,
    reason: String,
    location: String,
) -> Result<SignalReportOccurrence, SignalReportError> {
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

    const CATALOGUE_SPEC_SIGNALS: &str = r#"{
  "schema_version": 1,
  "catalogue_declaration_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "signals": [{
    "type_name": "SystemSignalReportSourceAdapter",
    "signal": "yellow",
    "entry_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
  }]
}"#;

    const IMPL_CATALOG_SIGNALS: &str = r#"{
  "schema_version": 3,
  "generated_at": "2026-07-31T00:00:00Z",
  "declaration_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "implementation_input_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "signals": [{
    "type_name": "SystemSignalReportSourceAdapter",
    "kind_tag": "struct",
    "signal": "yellow",
    "found_type": true
  }]
}"#;

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
        fs::create_dir_all(&track_dir).expect("track fixture directory must exist");
        fs::write(
            root.join("architecture-rules.json"),
            r#"{"version":2,"layers":[{"crate":"infrastructure","tddd":{"enabled":true,"catalogue_file":"infrastructure-types.json","catalogue_spec_signal":{"enabled":true}}}]}"#,
        )
        .expect("architecture rules fixture must be written");
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
            track_dir.join("infrastructure-types.json"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../track/items/signal-report-command-2026-07-31/infrastructure-types.json"
            )),
        )
        .expect("catalogue fixture must be written");
        fs::write(
            track_dir.join("infrastructure-catalogue-spec-signals.json"),
            CATALOGUE_SPEC_SIGNALS,
        )
        .expect("catalogue signals fixture must be written");
        fs::write(track_dir.join("infrastructure-type-signals.json"), IMPL_CATALOG_SIGNALS)
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
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(
            root.path().join("architecture-rules.json"),
            r#"{"version":2,"layers":[{"crate":"infrastructure","tddd":{"enabled":true,"catalogue_file":"infrastructure-types.json","catalogue_spec_signal":{"enabled":true}}}]}"#,
        )
        .unwrap();
        fs::write(
            track_dir.join("infrastructure-types.json"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../track/items/signal-report-command-2026-07-31/infrastructure-types.json"
            )),
        )
        .unwrap();
        let signals_path = track_dir.join("infrastructure-catalogue-spec-signals.json");
        let signals = r#"{
          "schema_version": 1,
          "catalogue_declaration_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
          "signals": [{
            "type_name": "SystemSignalReportSourceAdapter",
            "signal": "yellow",
            "entry_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
          }]
        }"#;
        fs::write(&signals_path, signals).unwrap();

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
    fn test_signal_report_adapter_skips_catalogue_spec_layers_without_signal_activation() {
        let root = tempfile::tempdir().unwrap();
        let track = TrackId::try_new("report-test").unwrap();
        fs::write(
            root.path().join("architecture-rules.json"),
            r#"{"version":2,"layers":[{"crate":"infrastructure","tddd":{"enabled":true}}]}"#,
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
            }] if entry_id.to_string() == "infrastructure:SystemSignalReportSourceAdapter"
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
            }] if entry_id.to_string() == "infrastructure:SystemSignalReportSourceAdapter"
                && reference.to_string() == "infrastructure-types.json#SystemSignalReportSourceAdapter"
                && reason.to_string() == "implementation does not conform to catalogue declaration"
                && location.to_string() == "track/items/report-test/infrastructure-type-signals.json"
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
