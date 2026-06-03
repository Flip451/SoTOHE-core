//! Shared helpers for the dry-check interactors (T004 and T005).
//!
//! Extracted to avoid duplicating SHA-256 and `FragmentRef` construction across
//! `DryCheckInteractor`, `DryCheckResultsInteractor`, and `DryCheckApprovalInteractor`.

use sha2::Digest as _;

use domain::dry_check::{FragmentContentHash, FragmentRef};
use domain::review_v2::types::FilePath;
use domain::semantic_dup::CodeFragment;

/// Compute the SHA-256 of `content` and return a validated [`FragmentContentHash`].
///
/// # Errors
///
/// Returns a [`String`] error description when [`FragmentContentHash::new`] rejects
/// the hex string (should not happen in practice for a well-formed SHA-256 digest,
/// but propagated as an error to keep production code panic-free).
pub(crate) fn content_hash_of(content: &str) -> Result<FragmentContentHash, String> {
    let digest = sha2::Sha256::digest(content.as_bytes());
    let hex = format!("{digest:x}");
    FragmentContentHash::new(hex).map_err(|e| format!("content hash: {e}"))
}

/// Build a [`FragmentRef`] from a [`CodeFragment`].
///
/// Computes the SHA-256 of `fragment.content()` to produce the
/// [`FragmentContentHash`]. The path comes from `fragment.source_path` (via
/// `to_string_lossy` — the same convention used throughout the workspace).
///
/// # Errors
///
/// Returns a [`String`] error description when `FilePath::new` rejects the
/// path (e.g., absolute path or traversal) or when `content_hash_of` fails.
pub(crate) fn fragment_ref_of(fragment: &CodeFragment) -> Result<FragmentRef, String> {
    let path_str = fragment.source_path.to_string_lossy().into_owned();
    let file_path = FilePath::new(path_str).map_err(|e| format!("invalid source_path: {e}"))?;
    let content_hash = content_hash_of(fragment.content())?;
    Ok(FragmentRef::new(file_path, content_hash))
}
