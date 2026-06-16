//! Corpus fingerprint computation for the D5 coverage staleness gate.
//!
//! Computes a deterministic SHA-256 fingerprint over the full set of
//! `(repo_relative_path, file_content_hash)` pairs that the `dry write`
//! corpus indexer scans, using the same file-walk rules as
//! `infrastructure::semantic_dup::extractor::extract_code_fragments`:
//! - Only `*.rs` files are included.
//! - `target/` directories and dot-prefixed directories are excluded.
//! - Files are visited in sorted (deterministic) order.
//!
//! The resulting fingerprint uniquely identifies "what the indexer saw".
//! A change to any corpus file (add, remove, modify) produces a different
//! fingerprint, letting `dry check-approved` detect stale indexes even when
//! diff-side fragments are unchanged.

use std::path::Path;

use domain::dry_check::DryCheckCorpusFingerprint;

// ── Public API ────────────────────────────────────────────────────────────────

/// Compute the corpus fingerprint for `workspace_root`.
///
/// Walks all `*.rs` files reachable from `workspace_root` using the same
/// exclusion rules as `extract_code_fragments` (skip `target/` and
/// dot-prefixed directories), then computes SHA-256 over a deterministic
/// length-prefixed byte encoding of the sorted list of
/// `(repo_relative_path_bytes, sha256_of_file_content)` pairs.
///
/// The function is fail-closed: any I/O error returns
/// [`DryCheckCorpusFingerprint::fail_closed`] so that `check_approved` returns
/// `Blocked` rather than granting approval with a potentially stale index.
///
/// # Determinism
///
/// Files are visited in sorted order (by absolute path); within a file,
/// the content hash is SHA-256 over the raw file bytes. Each record feeds the
/// hasher with the relative path byte length, exact relative path bytes, content
/// hash length, and content hash bytes. This avoids delimiter collisions and
/// lossy UTF-8 conversion for Unix filenames.
pub fn compute_corpus_fingerprint(workspace_root: &Path) -> DryCheckCorpusFingerprint {
    match collect_corpus_entries(workspace_root) {
        Ok(entries) => fingerprint_from_entries(&entries),
        Err(_) => DryCheckCorpusFingerprint::fail_closed(),
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// One corpus entry: `(repo_relative_path_bytes, sha256_of_content_hex)`.
type CorpusEntry = (Vec<u8>, String);

/// Recursively collect corpus entries from `dir`.
///
/// Mirrors `collect_rs_fragments` from the extractor: sorted entries, same
/// directory exclusions, `*.rs` files only.
fn collect_corpus_entries(workspace_root: &Path) -> Result<Vec<CorpusEntry>, std::io::Error> {
    let mut entries: Vec<CorpusEntry> = Vec::new();
    collect_entries_recursive(workspace_root, workspace_root, &mut entries)?;
    // Entries are already inserted in sorted order (sort_by_key on path within
    // each directory). Sort the whole list once at the end to guarantee global
    // deterministic order across directory-recursive inserts.
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(entries)
}

fn collect_entries_recursive(
    workspace_root: &Path,
    dir: &Path,
    out: &mut Vec<CorpusEntry>,
) -> Result<(), std::io::Error> {
    let read_dir = std::fs::read_dir(dir)?;

    let mut dir_entries = Vec::new();
    for entry_result in read_dir {
        dir_entries.push(entry_result?);
    }
    dir_entries.sort_by_key(|e| e.path());

    for entry in dir_entries {
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            if is_excluded_dir(&path) {
                continue;
            }
            collect_entries_recursive(workspace_root, &path, out)?;
        } else if file_type.is_file() && has_rs_extension(&path) {
            let rel = relative_path_bytes(workspace_root, &path);
            let content = std::fs::read(&path)?;
            let hash = sha256_hex(&content);
            out.push((rel, hash));
        }
        // symlinks and other types are silently skipped (mirrors extractor)
    }

    Ok(())
}

/// Compute SHA-256 over the sorted list of corpus entries and return a
/// [`DryCheckCorpusFingerprint`].
fn fingerprint_from_entries(entries: &[CorpusEntry]) -> DryCheckCorpusFingerprint {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"sotohe-dry-corpus-fingerprint-v1\0");
    for (path, hash) in entries {
        append_len_prefixed_bytes(&mut canonical, path);
        append_len_prefixed_bytes(&mut canonical, hash.as_bytes());
    }

    let hex = sha256_hex(&canonical);
    // SHA-256 always produces exactly 64 lowercase hex chars, so this cannot fail.
    DryCheckCorpusFingerprint::new(hex).unwrap_or_else(|_| DryCheckCorpusFingerprint::fail_closed())
}

/// Compute SHA-256 of `data` and return a 64-char lowercase hex string.
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest as _;
    let hash_bytes = sha2::Sha256::digest(data);
    hash_bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn append_len_prefixed_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    out.extend_from_slice(bytes);
}

/// Compute the repo-relative path bytes of `file` with respect to
/// `workspace_root`, with no leading `./`.
fn relative_path_bytes(workspace_root: &Path, file: &Path) -> Vec<u8> {
    let relative = file.strip_prefix(workspace_root).unwrap_or(file);
    path_bytes(relative)
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt as _;

    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt as _;

    path.as_os_str().encode_wide().flat_map(u16::to_be_bytes).collect()
}

#[cfg(not(any(unix, windows)))]
fn path_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().into_owned().into_bytes()
}

/// Return `true` when `path` is a directory that should be excluded (mirrors
/// `is_excluded_dir` in the extractor).
fn is_excluded_dir(path: &Path) -> bool {
    match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => name == "target" || name.starts_with('.'),
        None => false,
    }
}

/// Return `true` when `path` has the `.rs` extension (case-sensitive).
fn has_rs_extension(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "rs")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn write_file(dir: &Path, name: &str, content: &[u8]) {
        std::fs::write(dir.join(name), content).unwrap();
    }

    #[cfg(unix)]
    fn write_file_os(dir: &Path, name: Vec<u8>, content: &[u8]) {
        use std::os::unix::ffi::OsStringExt as _;

        std::fs::write(
            dir.join(std::path::PathBuf::from(std::ffi::OsString::from_vec(name))),
            content,
        )
        .unwrap();
    }

    // ── compute_corpus_fingerprint: determinism ───────────────────────────────

    #[test]
    fn test_compute_corpus_fingerprint_is_deterministic_for_same_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_file(root, "a.rs", b"fn a() {}");
        write_file(root, "b.rs", b"fn b() {}");

        let fp1 = compute_corpus_fingerprint(root);
        let fp2 = compute_corpus_fingerprint(root);

        assert_eq!(fp1, fp2, "same corpus must produce the same fingerprint");
    }

    // ── compute_corpus_fingerprint: change detection ──────────────────────────

    #[test]
    fn test_compute_corpus_fingerprint_differs_when_file_content_changes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_file(root, "a.rs", b"fn a() {}");
        let fp_before = compute_corpus_fingerprint(root);

        write_file(root, "a.rs", b"fn a_modified() {}");
        let fp_after = compute_corpus_fingerprint(root);

        assert_ne!(fp_before, fp_after, "modified file must produce a different fingerprint");
    }

    #[test]
    fn test_compute_corpus_fingerprint_differs_when_file_added() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_file(root, "a.rs", b"fn a() {}");
        let fp_before = compute_corpus_fingerprint(root);

        write_file(root, "b.rs", b"fn b() {}");
        let fp_after = compute_corpus_fingerprint(root);

        assert_ne!(fp_before, fp_after, "adding a file must produce a different fingerprint");
    }

    #[test]
    fn test_compute_corpus_fingerprint_differs_when_file_removed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_file(root, "a.rs", b"fn a() {}");
        write_file(root, "b.rs", b"fn b() {}");
        let fp_before = compute_corpus_fingerprint(root);

        std::fs::remove_file(root.join("b.rs")).unwrap();
        let fp_after = compute_corpus_fingerprint(root);

        assert_ne!(fp_before, fp_after, "removing a file must produce a different fingerprint");
    }

    // ── compute_corpus_fingerprint: exclusions ────────────────────────────────

    #[test]
    fn test_compute_corpus_fingerprint_excludes_target_directory() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_file(root, "a.rs", b"fn a() {}");
        let fp_before = compute_corpus_fingerprint(root);

        // Add a .rs file inside target/ — must not change the fingerprint.
        let target = root.join("target");
        std::fs::create_dir_all(&target).unwrap();
        write_file(&target, "generated.rs", b"fn generated() {}");
        let fp_after = compute_corpus_fingerprint(root);

        assert_eq!(
            fp_before, fp_after,
            "target/ directory must be excluded from the corpus fingerprint"
        );
    }

    #[test]
    fn test_compute_corpus_fingerprint_excludes_dot_directories() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_file(root, "a.rs", b"fn a() {}");
        let fp_before = compute_corpus_fingerprint(root);

        // Add a .rs file inside a hidden directory — must not change the fingerprint.
        let hidden = root.join(".git");
        std::fs::create_dir_all(&hidden).unwrap();
        write_file(&hidden, "hook.rs", b"fn hook() {}");
        let fp_after = compute_corpus_fingerprint(root);

        assert_eq!(
            fp_before, fp_after,
            "dot-prefixed directories must be excluded from the corpus fingerprint"
        );
    }

    #[test]
    fn test_compute_corpus_fingerprint_excludes_non_rs_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_file(root, "a.rs", b"fn a() {}");
        let fp_before = compute_corpus_fingerprint(root);

        // Add a non-.rs file — must not change the fingerprint.
        write_file(root, "README.md", b"# readme");
        let fp_after = compute_corpus_fingerprint(root);

        assert_eq!(fp_before, fp_after, "non-.rs files must not affect the corpus fingerprint");
    }

    #[cfg(unix)]
    #[test]
    fn test_compute_corpus_fingerprint_keeps_exact_unix_path_bytes() {
        let file_content = b"fn same_content() {}";

        let slash_dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(slash_dir.path().join("a")).unwrap();
        write_file(&slash_dir.path().join("a"), "b.rs", file_content);

        let backslash_dir = tempfile::tempdir().unwrap();
        write_file_os(backslash_dir.path(), b"a\\b.rs".to_vec(), file_content);

        let slash_fp = compute_corpus_fingerprint(slash_dir.path());
        let backslash_fp = compute_corpus_fingerprint(backslash_dir.path());

        assert_ne!(
            slash_fp, backslash_fp,
            "a literal backslash in a Unix filename must not be normalized to a path separator"
        );
    }

    // ── compute_corpus_fingerprint: empty workspace ───────────────────────────

    #[test]
    fn test_compute_corpus_fingerprint_empty_workspace_is_valid_fingerprint() {
        let dir = tempfile::tempdir().unwrap();
        let fp = compute_corpus_fingerprint(dir.path());
        // An empty workspace produces a fingerprint over an empty canonical string.
        // It must NOT be the fail-closed sentinel.
        assert_ne!(
            fp,
            DryCheckCorpusFingerprint::fail_closed(),
            "empty workspace must not be fail-closed"
        );
        assert_eq!(fp.as_str().len(), 64, "fingerprint must be 64 chars");
    }

    // ── compute_corpus_fingerprint: fail-closed on I/O error ─────────────────

    #[test]
    fn test_compute_corpus_fingerprint_nonexistent_workspace_returns_fail_closed() {
        let fp = compute_corpus_fingerprint(Path::new("/nonexistent/path/that/does/not/exist"));
        assert_eq!(
            fp,
            DryCheckCorpusFingerprint::fail_closed(),
            "nonexistent workspace_root must return fail-closed sentinel"
        );
    }
}
