use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use domain::TrackId;
use domain::adr_baseline::{AdrBaselineSourceState, AdrSourceFileName};
use domain::adr_decision::AdrDecisionEntry;
use usecase::adr_baseline::{AdrBaselineSourceError, AdrBaselineSourcePort};

use super::diagnostic;

const TRACK_ITEMS: &str = "track/items";
const MAX_ADR_BYTES: u64 = 8 * 1024 * 1024;
const MAX_TRACK_DOCUMENT_BYTES: u64 = 1024 * 1024;

/// Filesystem and local-Git source adapter rooted at the repository workspace.
#[derive(Debug, Clone)]
pub struct FsGitAdrBaselineSource {
    root: PathBuf,
}

impl From<PathBuf> for FsGitAdrBaselineSource {
    fn from(root: PathBuf) -> Self {
        Self { root }
    }
}

impl FsGitAdrBaselineSource {
    fn reject_symlinks(&self, path: &Path) -> Result<bool, AdrBaselineSourceError> {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(AdrBaselineSourceError::Read(diagnostic(
                    "ADR baseline source path must not be a symlink",
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(AdrBaselineSourceError::Read(diagnostic(&error.to_string())));
            }
        }
        crate::track::symlink_guard::reject_symlinks_below(path, &self.root)
            .map_err(|error| AdrBaselineSourceError::Read(diagnostic(&error.to_string())))
    }

    fn source_path(&self, source: &AdrSourceFileName) -> Result<PathBuf, AdrBaselineSourceError> {
        let adr_dir = self.root.join("knowledge/adr");
        trusted_adr_child(&adr_dir, source)
            .map_err(|error| AdrBaselineSourceError::Read(diagnostic(&error.to_string())))
    }

    fn fork_point(&self, track_id: &TrackId) -> Result<String, AdrBaselineSourceError> {
        let metadata_path =
            self.root.join(TRACK_ITEMS).join(track_id.as_ref()).join("metadata.json");
        self.reject_symlinks(&metadata_path)?;
        let text = read_utf8_limited(&metadata_path, MAX_TRACK_DOCUMENT_BYTES)
            .map_err(|error| AdrBaselineSourceError::Read(diagnostic(&error.to_string())))?;
        let (metadata, _) = crate::track::codec::decode(&text)
            .map_err(|error| AdrBaselineSourceError::Read(diagnostic(&error.to_string())))?;
        let local_base_ref = metadata.branch_strategy_snapshot().base_branch();
        let remote_base_ref = format!("origin/{local_base_ref}");
        for base_ref in [local_base_ref, remote_base_ref.as_str()] {
            let output = crate::git_cli::guarded_git_command()
                .args(["merge-base", "HEAD", base_ref])
                .current_dir(&self.root)
                .output()
                .map_err(|error| AdrBaselineSourceError::Read(diagnostic(&error.to_string())))?;
            if !output.status.success() {
                continue;
            }
            let hash = String::from_utf8(output.stdout)
                .map_err(|error| AdrBaselineSourceError::Read(diagnostic(&error.to_string())))?;
            let hash = hash.trim();
            if !hash.is_empty() {
                return Ok(hash.to_owned());
            }
        }

        Err(AdrBaselineSourceError::Read(diagnostic(&format!(
            "git merge-base could not resolve local base ref `{local_base_ref}` or remote-tracking base ref `{remote_base_ref}`"
        ))))
    }

    fn cited_from_spec(
        &self,
        track_id: &TrackId,
    ) -> Result<Vec<AdrSourceFileName>, AdrBaselineSourceError> {
        let path = self.root.join(TRACK_ITEMS).join(track_id.as_ref()).join("spec.json");
        self.reject_symlinks(&path)?;
        let text = match read_utf8_limited(&path, MAX_TRACK_DOCUMENT_BYTES) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(AdrBaselineSourceError::Read(diagnostic(&error.to_string())));
            }
        };
        let spec = crate::spec::codec::decode(&text)
            .map_err(|error| AdrBaselineSourceError::Read(diagnostic(&error.to_string())))?;
        let requirements = spec
            .goal()
            .iter()
            .chain(spec.scope().in_scope())
            .chain(spec.scope().out_of_scope())
            .chain(spec.constraints())
            .chain(spec.acceptance_criteria());
        let mut sources = std::collections::BTreeSet::new();
        for requirement in requirements {
            for reference in requirement.adr_refs() {
                let Ok(name) = reference.file.strip_prefix("knowledge/adr/") else {
                    return Err(AdrBaselineSourceError::Read(diagnostic(
                        "spec ADR reference is outside knowledge/adr",
                    )));
                };
                let Some(name) = name.to_str() else {
                    return Err(AdrBaselineSourceError::Read(diagnostic(
                        "spec ADR reference is not UTF-8",
                    )));
                };
                let source = AdrSourceFileName::try_new(name.to_owned()).map_err(|error| {
                    AdrBaselineSourceError::Read(diagnostic(&error.to_string()))
                })?;
                sources.insert(source);
            }
        }
        Ok(sources.into_iter().collect())
    }
}

impl AdrBaselineSourcePort for FsGitAdrBaselineSource {
    fn working_bytes(&self, source: &AdrSourceFileName) -> Result<Vec<u8>, AdrBaselineSourceError> {
        let path = self.source_path(source)?;
        match self.reject_symlinks(&path) {
            Ok(true) => {
                ensure_resolved_below(&path, &self.root).map_err(|error| {
                    AdrBaselineSourceError::Read(diagnostic(&error.to_string()))
                })?;
                read_file_limited(&path, MAX_ADR_BYTES)
                    .map_err(|error| AdrBaselineSourceError::Read(diagnostic(&error.to_string())))
            }
            Ok(false) => Err(AdrBaselineSourceError::Unavailable(source.clone())),
            Err(error) => Err(error),
        }
    }

    fn fork_point_bytes(
        &self,
        track_id: &TrackId,
        source: &AdrSourceFileName,
    ) -> Result<Vec<u8>, AdrBaselineSourceError> {
        let fork_point = self.fork_point(track_id)?;
        let object = format!("{fork_point}:knowledge/adr/{}", source.as_str());
        let size = crate::git_cli::guarded_git_command()
            .args(["cat-file", "-s", &object])
            .current_dir(&self.root)
            .output()
            .map_err(|error| AdrBaselineSourceError::Read(diagnostic(&error.to_string())))?;
        if !size.status.success() {
            return git_object_error(source, &size.stderr);
        }
        let size = String::from_utf8(size.stdout)
            .map_err(|error| AdrBaselineSourceError::Read(diagnostic(&error.to_string())))?
            .trim()
            .parse::<u64>()
            .map_err(|error| AdrBaselineSourceError::Read(diagnostic(&error.to_string())))?;
        if size > MAX_ADR_BYTES {
            return Err(AdrBaselineSourceError::Read(diagnostic(
                "fork-point ADR exceeds the configured byte limit",
            )));
        }
        let output = crate::git_cli::guarded_git_command()
            .args(["show", &object])
            .current_dir(&self.root)
            .output()
            .map_err(|error| AdrBaselineSourceError::Read(diagnostic(&error.to_string())))?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            git_object_error(source, &output.stderr)
        }
    }

    fn cited_sources(
        &self,
        track_id: &TrackId,
    ) -> Result<Vec<AdrSourceFileName>, AdrBaselineSourceError> {
        self.cited_from_spec(track_id)
    }

    fn source_state(
        &self,
        track_id: &TrackId,
        source: &AdrSourceFileName,
    ) -> Result<AdrBaselineSourceState, AdrBaselineSourceError> {
        match self.fork_point_bytes(track_id, source) {
            Ok(_) => Ok(AdrBaselineSourceState::ExistingAtForkPoint),
            Err(AdrBaselineSourceError::Read(error)) => Err(AdrBaselineSourceError::Read(error)),
            Err(AdrBaselineSourceError::Unavailable(_)) => {
                let bytes = self.working_bytes(source)?;
                let text = std::str::from_utf8(&bytes).map_err(|error| {
                    AdrBaselineSourceError::Read(diagnostic(&error.to_string()))
                })?;
                let frontmatter =
                    crate::adr_decision::parse_adr_frontmatter(text).map_err(|error| {
                        AdrBaselineSourceError::Read(diagnostic(&error.to_string()))
                    })?;
                if has_user_decision_ref(&frontmatter) {
                    Ok(AdrBaselineSourceState::TrackBornPromoted)
                } else {
                    Ok(AdrBaselineSourceState::TrackBornDraft)
                }
            }
        }
    }
}

fn has_user_decision_ref(frontmatter: &domain::AdrFrontMatter) -> bool {
    frontmatter.decisions().iter().any(|entry| match entry {
        AdrDecisionEntry::ProposedDecision(decision) => {
            decision.common.user_decision_ref().is_some()
        }
        AdrDecisionEntry::AcceptedDecision(decision) => {
            decision.common.user_decision_ref().is_some()
        }
        AdrDecisionEntry::ImplementedDecision(decision) => {
            decision.common.user_decision_ref().is_some()
        }
        AdrDecisionEntry::SupersededDecision(decision) => {
            decision.common.user_decision_ref().is_some()
        }
        AdrDecisionEntry::DeprecatedDecision(decision) => {
            decision.common.user_decision_ref().is_some()
        }
    })
}

fn trusted_adr_child(root: &Path, source: &AdrSourceFileName) -> std::io::Result<PathBuf> {
    if has_windows_drive_prefix(source.as_str()) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "ADR source filename must not have a Windows drive prefix",
        ));
    }
    let path = root.join(source.as_str());
    if !path.starts_with(root) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "ADR source path escapes its trusted root",
        ));
    }
    Ok(path)
}

fn has_windows_drive_prefix(value: &str) -> bool {
    matches!(
        (value.as_bytes().first(), value.as_bytes().get(1)),
        (Some(first), Some(b':')) if first.is_ascii_alphabetic()
    )
}

fn ensure_resolved_below(path: &Path, trusted_root: &Path) -> std::io::Result<()> {
    let canonical_root = trusted_root.canonicalize()?;
    let resolved = match path.canonicalize() {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => path
            .parent()
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "ADR source path has no parent",
                )
            })?
            .canonicalize()?,
        Err(error) => return Err(error),
    };
    if resolved.starts_with(&canonical_root) {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "resolved ADR source path escapes its trusted root",
        ))
    }
}

fn read_utf8_limited(path: &Path, limit: u64) -> std::io::Result<String> {
    let bytes = read_file_limited(path, limit)?;
    String::from_utf8(bytes)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

fn read_file_limited(path: &Path, limit: u64) -> std::io::Result<Vec<u8>> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > limit {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "ADR baseline input exceeds the configured byte limit",
        ));
    }
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "ADR baseline input exceeds the configured byte limit",
        ));
    }
    Ok(bytes)
}

fn git_object_error(
    source: &AdrSourceFileName,
    stderr: &[u8],
) -> Result<Vec<u8>, AdrBaselineSourceError> {
    let stderr = String::from_utf8_lossy(stderr);
    if stderr.contains("does not exist in") || stderr.contains("exists on disk, but not in") {
        Err(AdrBaselineSourceError::Unavailable(source.clone()))
    } else {
        Err(AdrBaselineSourceError::Read(diagnostic(&stderr)))
    }
}
