//! Corpus-root fingerprint sidecar helpers for the `dry` command family.
//!
//! Manages the `dry-check-corpus-root.json` sidecar that records which
//! workspace root was scanned during `dry write`, so that `dry check-approved`
//! can recompute the same corpus fingerprint for staleness detection (D5).

use std::path::Path;
use std::path::PathBuf;

use domain::dry_check::DryCheckCorpusFingerprint;
use domain::semantic_dup::CodeFragment;
use infrastructure::dry_check::corpus::sha256_hex;
use infrastructure::semantic_dup::extractor::extract_code_fragments;
use infrastructure::track::{
    atomic_write::atomic_write_file, symlink_guard::reject_symlinks_below,
};
use usecase::dry_check::fragment_ref_of;

use super::shared::normalize_fragment_paths;

pub(super) const DRY_CORPUS_ROOT_MANIFEST_FILE: &str = "dry-check-corpus-root.json";

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub(super) struct DryCorpusRootManifest {
    pub(super) schema_version: u32,
    pub(super) workspace_root: PathBuf,
}

pub(super) fn dry_corpus_root_manifest_path(track_dir: &Path) -> PathBuf {
    track_dir.join(DRY_CORPUS_ROOT_MANIFEST_FILE)
}

pub(super) fn workspace_root_for_manifest(workspace_root: &Path, repo_root: &Path) -> PathBuf {
    match workspace_root.strip_prefix(repo_root) {
        Ok(rel) if rel.as_os_str().is_empty() => PathBuf::from("."),
        Ok(rel) => rel.to_path_buf(),
        Err(_) => workspace_root.to_path_buf(),
    }
}

pub(super) fn append_len_prefixed_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    out.extend_from_slice(bytes);
}

pub(super) fn compute_dry_corpus_fingerprint_from_fragments(
    corpus_fragments: &[CodeFragment],
) -> DryCheckCorpusFingerprint {
    let mut entries = Vec::with_capacity(corpus_fragments.len());
    for fragment in corpus_fragments {
        let fragment_ref = match fragment_ref_of(fragment) {
            Ok(fragment_ref) => fragment_ref,
            Err(_) => return DryCheckCorpusFingerprint::fail_closed(),
        };
        entries.push((
            fragment_ref.path().as_str().to_owned(),
            fragment_ref.content_hash().as_str().to_owned(),
        ));
    }
    entries.sort();

    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"sotohe-dry-corpus-fragment-fingerprint-v1\0");
    for (path, content_hash) in entries {
        append_len_prefixed_bytes(&mut canonical, path.as_bytes());
        append_len_prefixed_bytes(&mut canonical, content_hash.as_bytes());
    }

    DryCheckCorpusFingerprint::new(sha256_hex(&canonical))
        .unwrap_or_else(|_| DryCheckCorpusFingerprint::fail_closed())
}

pub(super) fn compute_dry_corpus_fingerprint_from_root(
    corpus_root: &Path,
    repo_root: &Path,
) -> DryCheckCorpusFingerprint {
    let raw_fragments = match extract_code_fragments(corpus_root) {
        Ok(raw_fragments) => raw_fragments,
        Err(_) => return DryCheckCorpusFingerprint::fail_closed(),
    };
    let normalized_fragments = match normalize_fragment_paths(raw_fragments, repo_root) {
        Ok(normalized_fragments) => normalized_fragments,
        Err(_) => return DryCheckCorpusFingerprint::fail_closed(),
    };
    compute_dry_corpus_fingerprint_from_fragments(&normalized_fragments)
}

pub(super) fn validate_dry_corpus_root(
    raw_root: &Path,
    repo_root: &Path,
    manifest_path: &Path,
) -> Result<PathBuf, String> {
    let absolute_root =
        if raw_root.is_absolute() { raw_root.to_path_buf() } else { repo_root.join(raw_root) };
    let canonical_root = absolute_root.canonicalize().map_err(|e| {
        format!(
            "dry corpus root manifest '{}' points to a non-canonicalizable workspace root '{}': {e}",
            manifest_path.display(),
            raw_root.display()
        )
    })?;
    if !canonical_root.is_dir() || !canonical_root.starts_with(repo_root) {
        return Err(format!(
            "dry corpus root manifest '{}' must point to an existing directory under the repository root",
            manifest_path.display()
        ));
    }
    Ok(canonical_root)
}

pub(super) fn write_dry_corpus_root_manifest(
    track_dir: &Path,
    workspace_root: &Path,
    repo_root: &Path,
) -> Result<(), String> {
    let manifest = DryCorpusRootManifest {
        schema_version: 1,
        workspace_root: workspace_root_for_manifest(workspace_root, repo_root),
    };
    let manifest_path = dry_corpus_root_manifest_path(track_dir);
    let content = serde_json::to_vec_pretty(&manifest).map_err(|e| {
        format!("failed to serialize dry corpus root manifest '{}': {e}", manifest_path.display())
    })?;
    if let Some(parent) = manifest_path.parent() {
        if !parent.as_os_str().is_empty() {
            reject_symlinks_below(parent, repo_root).map_err(|e| {
                format!("symlink guard dry corpus root manifest parent '{}': {e}", parent.display())
            })?;
            std::fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "failed to create dry corpus root manifest parent '{}': {e}",
                    parent.display()
                )
            })?;
        }
    }
    reject_symlinks_below(&manifest_path, repo_root).map_err(|e| {
        format!("symlink guard dry corpus root manifest '{}': {e}", manifest_path.display())
    })?;
    atomic_write_file(&manifest_path, &content).map_err(|e| {
        format!("failed to write dry corpus root manifest '{}': {e}", manifest_path.display())
    })
}

pub(crate) fn resolve_dry_corpus_fingerprint_root(
    track_dir: &Path,
    repo_root: &Path,
) -> Result<PathBuf, String> {
    let manifest_path = dry_corpus_root_manifest_path(track_dir);
    match std::fs::symlink_metadata(&manifest_path) {
        Ok(_) => {
            reject_symlinks_below(&manifest_path, repo_root).map_err(|e| {
                format!("symlink guard dry corpus root manifest '{}': {e}", manifest_path.display())
            })?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = manifest_path.parent() {
                reject_symlinks_below(parent, repo_root).map_err(|e| {
                    format!(
                        "symlink guard dry corpus root manifest parent '{}': {e}",
                        parent.display()
                    )
                })?;
            }
            return Ok(repo_root.to_path_buf());
        }
        Err(e) => {
            return Err(format!(
                "failed to stat dry corpus root manifest '{}': {e}",
                manifest_path.display()
            ));
        }
    }
    let content = match std::fs::read_to_string(&manifest_path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(repo_root.to_path_buf()),
        Err(e) => {
            return Err(format!(
                "failed to read dry corpus root manifest '{}': {e}",
                manifest_path.display()
            ));
        }
    };

    let manifest: DryCorpusRootManifest = serde_json::from_str(&content).map_err(|e| {
        format!("failed to parse dry corpus root manifest '{}': {e}", manifest_path.display())
    })?;
    if manifest.schema_version != 1 {
        return Err(format!(
            "unsupported dry corpus root manifest schema_version {} in '{}'",
            manifest.schema_version,
            manifest_path.display()
        ));
    }

    validate_dry_corpus_root(&manifest.workspace_root, repo_root, &manifest_path)
}

pub(crate) fn compute_current_dry_corpus_fingerprint(
    track_dir: &Path,
    repo_root: &Path,
) -> domain::dry_check::DryCheckCorpusFingerprint {
    match resolve_dry_corpus_fingerprint_root(track_dir, repo_root) {
        Ok(corpus_root) => compute_dry_corpus_fingerprint_from_root(&corpus_root, repo_root),
        Err(e) => {
            eprintln!("[warn] dry check-approved: {e}; treating corpus fingerprint as stale");
            domain::dry_check::DryCheckCorpusFingerprint::fail_closed()
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::path::PathBuf;

    use infrastructure::semantic_dup::extractor::extract_code_fragments;

    use super::*;
    use crate::dry::shared::normalize_fragment_paths;

    #[test]
    fn test_resolve_dry_corpus_fingerprint_root_missing_manifest_falls_back_to_repo_root() {
        let repo = tempfile::tempdir().unwrap();
        let repo_root = repo.path().canonicalize().unwrap();
        let track_dir = repo_root.join("track/items/test-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        let resolved = resolve_dry_corpus_fingerprint_root(&track_dir, &repo_root).unwrap();

        assert_eq!(
            resolved, repo_root,
            "older manifests without the corpus-root sidecar must keep repo-root behavior"
        );
    }

    #[test]
    fn test_resolve_dry_corpus_fingerprint_root_reads_written_workspace_root() {
        let repo = tempfile::tempdir().unwrap();
        let repo_root = repo.path().canonicalize().unwrap();
        let workspace_root = repo_root.join("apps/cli-composition");
        let track_dir = repo_root.join("track/items/test-track");
        std::fs::create_dir_all(&workspace_root).unwrap();
        std::fs::create_dir_all(&track_dir).unwrap();

        write_dry_corpus_root_manifest(&track_dir, &workspace_root, &repo_root).unwrap();
        let manifest_json =
            std::fs::read_to_string(dry_corpus_root_manifest_path(&track_dir)).unwrap();
        let manifest: DryCorpusRootManifest = serde_json::from_str(&manifest_json).unwrap();
        assert_eq!(
            manifest.workspace_root,
            PathBuf::from("apps/cli-composition"),
            "sidecar must be portable across checkout roots"
        );
        let resolved = resolve_dry_corpus_fingerprint_root(&track_dir, &repo_root).unwrap();

        assert_eq!(
            resolved,
            workspace_root.canonicalize().unwrap(),
            "approval must recompute the corpus fingerprint from the same workspace_root used by write"
        );
    }

    #[test]
    fn test_write_dry_corpus_root_manifest_creates_missing_track_dir() {
        let repo = tempfile::tempdir().unwrap();
        let repo_root = repo.path().canonicalize().unwrap();
        let workspace_root = repo_root.join("apps/cli-composition");
        let items_dir = repo_root.join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&workspace_root).unwrap();
        std::fs::create_dir_all(&items_dir).unwrap();

        write_dry_corpus_root_manifest(&track_dir, &workspace_root, &repo_root).unwrap();

        assert!(
            dry_corpus_root_manifest_path(&track_dir).is_file(),
            "first dry write must create the track directory before writing the sidecar"
        );
    }

    #[test]
    fn test_dry_corpus_fingerprint_from_fragments_detects_later_workspace_drift() {
        let repo = tempfile::tempdir().unwrap();
        let repo_root = repo.path().canonicalize().unwrap();
        let src_dir = repo_root.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(src_dir.join("lib.rs"), "pub fn original() {}\n").unwrap();

        let raw_fragments = extract_code_fragments(&repo_root).unwrap();
        let normalized_fragments = normalize_fragment_paths(raw_fragments, &repo_root).unwrap();
        let snapshot_fingerprint =
            compute_dry_corpus_fingerprint_from_fragments(&normalized_fragments);

        std::fs::write(src_dir.join("lib.rs"), "pub fn changed() {}\n").unwrap();
        let current_fingerprint = compute_dry_corpus_fingerprint_from_root(&repo_root, &repo_root);

        assert_ne!(
            snapshot_fingerprint, current_fingerprint,
            "approval must detect source drift after the dry-write corpus snapshot was extracted"
        );
    }

    #[test]
    fn test_dry_corpus_fingerprint_from_fragments_ignores_line_span_shift() {
        use domain::semantic_dup::CodeFragment;

        let path = PathBuf::from("src/lib.rs");
        let content = "pub fn stable() {}\n".to_owned();
        let before = CodeFragment::new(path.clone(), content.clone(), 1, 1).unwrap();
        let after = CodeFragment::new(path, content, 20, 20).unwrap();

        let before_fingerprint = compute_dry_corpus_fingerprint_from_fragments(&[before]);
        let after_fingerprint = compute_dry_corpus_fingerprint_from_fragments(&[after]);

        assert_eq!(
            before_fingerprint, after_fingerprint,
            "corpus fingerprint identity is path plus content hash; line shifts alone must not stale coverage"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_write_dry_corpus_root_manifest_rejects_symlinked_sidecar() {
        let repo = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let repo_root = repo.path().canonicalize().unwrap();
        let workspace_root = repo_root.join("apps/cli-composition");
        let track_dir = repo_root.join("track/items/test-track");
        std::fs::create_dir_all(&workspace_root).unwrap();
        std::fs::create_dir_all(&track_dir).unwrap();

        let outside_target = outside.path().join("target.json");
        std::fs::write(&outside_target, "do not overwrite").unwrap();
        std::os::unix::fs::symlink(&outside_target, dry_corpus_root_manifest_path(&track_dir))
            .unwrap();

        let result = write_dry_corpus_root_manifest(&track_dir, &workspace_root, &repo_root);

        assert!(result.is_err(), "symlinked corpus-root sidecar must be rejected");
        assert_eq!(
            std::fs::read_to_string(&outside_target).unwrap(),
            "do not overwrite",
            "sidecar write must not follow a symlink outside the repository"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_resolve_dry_corpus_fingerprint_root_rejects_symlinked_sidecar() {
        let repo = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let repo_root = repo.path().canonicalize().unwrap();
        let track_dir = repo_root.join("track/items/test-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        let outside_target = outside.path().join("target.json");
        std::fs::write(&outside_target, r#"{"schema_version":1,"workspace_root":"."}"#).unwrap();
        std::os::unix::fs::symlink(&outside_target, dry_corpus_root_manifest_path(&track_dir))
            .unwrap();

        let result = resolve_dry_corpus_fingerprint_root(&track_dir, &repo_root);

        assert!(result.is_err(), "symlinked corpus-root sidecar must fail closed");
    }

    #[test]
    fn test_compute_current_dry_corpus_fingerprint_invalid_manifest_returns_fail_closed() {
        let repo = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let repo_root = repo.path().canonicalize().unwrap();
        let track_dir = repo_root.join("track/items/test-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        let manifest = DryCorpusRootManifest {
            schema_version: 1,
            workspace_root: outside.path().to_path_buf(),
        };
        std::fs::write(
            dry_corpus_root_manifest_path(&track_dir),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();

        let fingerprint = compute_current_dry_corpus_fingerprint(&track_dir, &repo_root);

        assert_eq!(
            fingerprint.as_str(),
            domain::dry_check::DryCheckCorpusFingerprint::fail_closed().as_str(),
            "invalid sidecars must block approval instead of silently using a wrong root"
        );
    }
}
