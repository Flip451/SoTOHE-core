//! Corpus-manifest filesystem adapter — [`FsDryCorpusMetaAdapter`].
//!
//! Implements [`usecase::fixpoint_resolve::DryCorpusMetaPort`] by reading the
//! `dry-check-corpus-root.json` sidecar (written by `dry write`) and computing
//! the current corpus fingerprint from the recorded workspace root.
//!
//! All I/O is contained here so that the usecase interactor
//! (`FixpointDryGateInteractor`) stays free of `std::fs` (CN-07).
//!
//! Design: D4 / D5 / CN-02 / CN-07 / AC-03 / AC-07.

use std::path::{Path, PathBuf};

use domain::dry_check::DryCheckCorpusFingerprint;
use domain::semantic_dup::CodeFragment;
use usecase::dry_check::fragment_ref_of;
use usecase::fixpoint_resolve::DryCorpusMetaPort;

use crate::dry_check::corpus::sha256_hex;
use crate::semantic_dup::extractor::extract_code_fragments;
use crate::track::symlink_guard::reject_symlinks_below;

// ── Manifest types ────────────────────────────────────────────────────────────

const DRY_CORPUS_ROOT_MANIFEST_FILE: &str = "dry-check-corpus-root.json";

#[derive(Debug, serde::Deserialize)]
struct SchemaVersionEnvelope {
    schema_version: u32,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct DryCorpusRootManifest {
    schema_version: u32,
    workspace_root: PathBuf,
}

fn dry_corpus_root_manifest_path(track_dir: &Path) -> PathBuf {
    track_dir.join(DRY_CORPUS_ROOT_MANIFEST_FILE)
}

// ── FsDryCorpusMetaAdapter ────────────────────────────────────────────────────

/// Infrastructure adapter implementing [`DryCorpusMetaPort`].
///
/// Reads `<track_dir>/dry-check-corpus-root.json` to determine which workspace
/// root was scanned during `dry write`, then computes the current corpus
/// fingerprint from that root.
///
/// Policy:
/// - Manifest **absent**: fall back to `canonical_root` as the workspace root and
///   compute the fingerprint from `canonical_root`.
/// - Manifest **present**: resolve the recorded workspace root (validated to lie
///   within `repo_root`); compute the fingerprint from that root.
/// - Resolution **error**: emit an `[warn]` and return
///   `DryCheckCorpusFingerprint::fail_closed()` (blocking approval).
///
/// Relocated from `apps/cli-composition/src/track/fixpoint_resolve.rs` per the
/// T008 FixpointDryGate extraction requirement.
pub struct FsDryCorpusMetaAdapter;

impl DryCorpusMetaPort for FsDryCorpusMetaAdapter {
    fn resolve_corpus_meta(
        &self,
        track_dir: &Path,
        canonical_root: &Path,
        repo_root: &Path,
    ) -> Result<(PathBuf, DryCheckCorpusFingerprint), String> {
        let manifest_path = dry_corpus_root_manifest_path(track_dir);

        match std::fs::symlink_metadata(&manifest_path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Manifest absent: use canonical_root as the workspace root.
                let fingerprint = compute_dry_write_corpus_fingerprint(canonical_root, repo_root);
                Ok((canonical_root.to_path_buf(), fingerprint))
            }
            Err(e) => {
                // stat failure — treat as stale (fail-closed).
                eprintln!(
                    "[warn] fixpoint-resolve: failed to stat corpus-root manifest '{}': {e}; \
                     treating corpus fingerprint as stale",
                    manifest_path.display()
                );
                Ok((repo_root.to_path_buf(), DryCheckCorpusFingerprint::fail_closed()))
            }
            Ok(_) => {
                // Manifest present: resolve workspace_root from the sidecar.
                match resolve_workspace_root_from_manifest(&manifest_path, repo_root) {
                    Ok(workspace_root) => {
                        let fingerprint =
                            compute_dry_write_corpus_fingerprint(&workspace_root, repo_root);
                        Ok((workspace_root, fingerprint))
                    }
                    Err(e) => {
                        eprintln!(
                            "[warn] fixpoint-resolve: {e}; treating corpus fingerprint as stale"
                        );
                        Ok((repo_root.to_path_buf(), DryCheckCorpusFingerprint::fail_closed()))
                    }
                }
            }
        }
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Read the `dry-check-corpus-root.json` sidecar and return the validated
/// canonical absolute workspace root path.
///
/// # Errors
///
/// Returns a human-readable `String` error when:
/// - A symlink is detected at or below the manifest path.
/// - The manifest cannot be read or parsed.
/// - The recorded workspace root is not a directory under `repo_root`.
fn resolve_workspace_root_from_manifest(
    manifest_path: &Path,
    repo_root: &Path,
) -> Result<PathBuf, String> {
    // Symlink guard on the manifest file itself.
    reject_symlinks_below(manifest_path, repo_root).map_err(|e| {
        format!("symlink guard on corpus-root manifest '{}': {e}", manifest_path.display())
    })?;

    let content = std::fs::read_to_string(manifest_path).map_err(|e| {
        format!("failed to read corpus-root manifest '{}': {e}", manifest_path.display())
    })?;

    let envelope: SchemaVersionEnvelope = serde_json::from_str(&content).map_err(|e| {
        format!(
            "failed to parse corpus-root manifest schema_version '{}': {e}",
            manifest_path.display()
        )
    })?;
    if envelope.schema_version != 1 {
        return Err(format!(
            "unsupported corpus-root manifest schema_version {} in '{}'",
            envelope.schema_version,
            manifest_path.display()
        ));
    }

    let manifest: DryCorpusRootManifest = serde_json::from_str(&content).map_err(|e| {
        format!("failed to parse corpus-root manifest '{}': {e}", manifest_path.display())
    })?;

    validate_workspace_root(&manifest.workspace_root, repo_root, manifest_path)
}

/// Canonicalize and validate `raw_root` as an existing directory under `repo_root`.
fn validate_workspace_root(
    raw_root: &Path,
    repo_root: &Path,
    manifest_path: &Path,
) -> Result<PathBuf, String> {
    let absolute_root =
        if raw_root.is_absolute() { raw_root.to_path_buf() } else { repo_root.join(raw_root) };

    reject_symlinks_below(&absolute_root, repo_root).map_err(|e| {
        format!("symlink guard on corpus-root workspace root '{}': {e}", raw_root.display())
    })?;

    let canonical_root = absolute_root.canonicalize().map_err(|e| {
        format!(
            "corpus-root manifest '{}' points to a non-canonicalizable workspace root '{}': {e}",
            manifest_path.display(),
            raw_root.display()
        )
    })?;

    if !canonical_root.is_dir() || !canonical_root.starts_with(repo_root) {
        return Err(format!(
            "corpus-root manifest '{}' must point to an existing directory under the repository root",
            manifest_path.display()
        ));
    }

    Ok(canonical_root)
}

fn compute_dry_write_corpus_fingerprint(
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
    compute_dry_write_corpus_fingerprint_from_fragments(&normalized_fragments)
}

fn normalize_fragment_paths(
    fragments: Vec<CodeFragment>,
    repo_root: &Path,
) -> Result<Vec<CodeFragment>, String> {
    let mut normalized = Vec::with_capacity(fragments.len());
    for fragment in fragments {
        let relative_path = fragment
            .source_path
            .strip_prefix(repo_root)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| fragment.source_path.clone());
        let path = relative_path.to_string_lossy().replace('\\', "/");
        let rebuilt = CodeFragment::new(
            PathBuf::from(path),
            fragment.content().to_owned(),
            fragment.start_line(),
            fragment.end_line(),
        )
        .map_err(|e| format!("fragment rebuild failed: {e}"))?;
        normalized.push(rebuilt);
    }
    Ok(normalized)
}

fn compute_dry_write_corpus_fingerprint_from_fragments(
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

fn append_len_prefixed_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    out.extend_from_slice(bytes);
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::track::atomic_write::atomic_write_file;

    fn write_manifest(track_dir: &Path, manifest: &DryCorpusRootManifest) {
        let manifest_path = dry_corpus_root_manifest_path(track_dir);
        let content = serde_json::to_vec_pretty(manifest).unwrap();
        atomic_write_file(&manifest_path, &content).unwrap();
    }

    // ── Missing manifest → canonical_root fallback ────────────────────────────

    #[test]
    fn missing_manifest_uses_canonical_root_as_workspace_root() {
        let repo = tempfile::tempdir().unwrap();
        let repo_root = repo.path().canonicalize().unwrap();
        let track_dir = repo_root.join("track/items/test-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(repo_root.join("lib.rs"), "fn f() {}").unwrap();

        let adapter = FsDryCorpusMetaAdapter;
        let (workspace_root, fingerprint) =
            adapter.resolve_corpus_meta(&track_dir, &repo_root, &repo_root).unwrap();

        assert_eq!(workspace_root, repo_root, "missing manifest must fall back to canonical_root");
        assert_ne!(
            fingerprint,
            DryCheckCorpusFingerprint::fail_closed(),
            "missing manifest must not produce fail-closed fingerprint"
        );
    }

    // ── Present manifest → recorded workspace_root ────────────────────────────

    #[test]
    fn present_manifest_uses_recorded_workspace_root() {
        let repo = tempfile::tempdir().unwrap();
        let repo_root = repo.path().canonicalize().unwrap();
        let workspace_root = repo_root.join("apps/cli");
        let track_dir = repo_root.join("track/items/test-track");
        std::fs::create_dir_all(&workspace_root).unwrap();
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(workspace_root.join("main.rs"), "fn main() {}").unwrap();

        write_manifest(
            &track_dir,
            &DryCorpusRootManifest { schema_version: 1, workspace_root: PathBuf::from("apps/cli") },
        );

        let adapter = FsDryCorpusMetaAdapter;
        let (resolved_root, _fingerprint) =
            adapter.resolve_corpus_meta(&track_dir, &repo_root, &repo_root).unwrap();

        assert_eq!(
            resolved_root,
            workspace_root.canonicalize().unwrap(),
            "present manifest must resolve the recorded workspace root"
        );
    }

    #[test]
    fn test_resolve_corpus_meta_scoped_workspace_uses_repo_relative_corpus_fingerprint() {
        let repo = tempfile::tempdir().unwrap();
        let repo_root = repo.path().canonicalize().unwrap();
        let workspace_root = repo_root.join("scoped");
        let src_dir = workspace_root.join("src");
        let track_dir = repo_root.join("track/items/test-track");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(src_dir.join("lib.rs"), "pub fn scoped_value() -> u32 {\n    1\n}\n")
            .unwrap();

        write_manifest(
            &track_dir,
            &DryCorpusRootManifest { schema_version: 1, workspace_root: PathBuf::from("scoped") },
        );

        let adapter = FsDryCorpusMetaAdapter;
        let (_resolved_root, fingerprint) =
            adapter.resolve_corpus_meta(&track_dir, &repo_root, &repo_root).unwrap();

        let repo_relative_fingerprint =
            compute_dry_write_corpus_fingerprint(&workspace_root, &repo_root);
        let scoped_relative_fingerprint =
            compute_dry_write_corpus_fingerprint(&workspace_root, &workspace_root);

        assert_eq!(
            fingerprint, repo_relative_fingerprint,
            "approval must match the dry-write corpus fingerprint for scoped workspaces"
        );
        assert_ne!(
            fingerprint, scoped_relative_fingerprint,
            "scoped workspace paths must retain their repo-relative prefix"
        );
    }

    // ── Manifest with workspace_root outside repo → fail-closed ──────────────

    #[test]
    fn manifest_pointing_outside_repo_returns_fail_closed_fingerprint() {
        let repo = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let repo_root = repo.path().canonicalize().unwrap();
        let track_dir = repo_root.join("track/items/test-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        write_manifest(
            &track_dir,
            &DryCorpusRootManifest {
                schema_version: 1,
                workspace_root: outside.path().to_path_buf(),
            },
        );

        let adapter = FsDryCorpusMetaAdapter;
        let (_, fingerprint) =
            adapter.resolve_corpus_meta(&track_dir, &repo_root, &repo_root).unwrap();

        assert_eq!(
            fingerprint,
            DryCheckCorpusFingerprint::fail_closed(),
            "workspace_root outside repo must produce fail-closed fingerprint"
        );
    }

    #[test]
    fn test_resolve_corpus_meta_manifest_with_unknown_field_returns_fail_closed_fingerprint() {
        let repo = tempfile::tempdir().unwrap();
        let repo_root = repo.path().canonicalize().unwrap();
        let track_dir = repo_root.join("track/items/test-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            dry_corpus_root_manifest_path(&track_dir),
            r#"{"schema_version":1,"workspace_root":".","future_field":true}"#,
        )
        .unwrap();

        let adapter = FsDryCorpusMetaAdapter;
        let (_, fingerprint) =
            adapter.resolve_corpus_meta(&track_dir, &repo_root, &repo_root).unwrap();

        assert_eq!(
            fingerprint,
            DryCheckCorpusFingerprint::fail_closed(),
            "corpus-root manifests with unknown fields must fail closed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_resolve_corpus_meta_symlinked_workspace_root_returns_fail_closed_fingerprint() {
        let repo = tempfile::tempdir().unwrap();
        let repo_root = repo.path().canonicalize().unwrap();
        let real_workspace = repo_root.join("real-workspace");
        let symlinked_workspace = repo_root.join("workspace-link");
        let track_dir = repo_root.join("track/items/test-track");
        std::fs::create_dir_all(&real_workspace).unwrap();
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(real_workspace.join("lib.rs"), "fn f() {}").unwrap();
        std::os::unix::fs::symlink(&real_workspace, &symlinked_workspace).unwrap();

        write_manifest(
            &track_dir,
            &DryCorpusRootManifest {
                schema_version: 1,
                workspace_root: PathBuf::from("workspace-link"),
            },
        );

        let adapter = FsDryCorpusMetaAdapter;
        let (_, fingerprint) =
            adapter.resolve_corpus_meta(&track_dir, &repo_root, &repo_root).unwrap();

        assert_eq!(
            fingerprint,
            DryCheckCorpusFingerprint::fail_closed(),
            "symlinked workspace roots must fail closed before canonicalization"
        );
    }

    // ── Symlinked manifest → error ────────────────────────────────────────────

    #[cfg(unix)]
    #[test]
    fn symlinked_manifest_is_rejected() {
        let repo = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let repo_root = repo.path().canonicalize().unwrap();
        let track_dir = repo_root.join("track/items/test-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        // Create a valid manifest outside the repo and symlink to it from inside.
        let outside_target = outside.path().join("target.json");
        std::fs::write(&outside_target, r#"{"schema_version":1,"workspace_root":"."}"#).unwrap();
        std::os::unix::fs::symlink(&outside_target, dry_corpus_root_manifest_path(&track_dir))
            .unwrap();

        let adapter = FsDryCorpusMetaAdapter;
        // A symlinked manifest must produce fail-closed — resolve_corpus_meta
        // wraps the rejection into a warn+fallback, so result is Ok but fail-closed.
        let (_, fingerprint) =
            adapter.resolve_corpus_meta(&track_dir, &repo_root, &repo_root).unwrap();

        assert_eq!(
            fingerprint,
            DryCheckCorpusFingerprint::fail_closed(),
            "symlinked corpus-root manifest must fail closed"
        );
    }
}
