//! Fixed in-memory known-bad probe pairs for D4 calibration barrier (T012).
//!
//! A probe pair is a synthetic `(changed_fragment, candidate_fragment)` where the
//! correct verdict is `Violation`. The calibration barrier runs these probes against
//! the agent and checks the detection rate before trusting fast-tier results.

use std::path::PathBuf;

use domain::semantic_dup::CodeFragment;

// ── KnownBadProbeError ────────────────────────────────────────────────────────

/// Error type for [`known_bad_probe_pairs`].
#[derive(Debug, thiserror::Error)]
pub enum KnownBadProbeError {
    /// A probe's `CodeFragment` could not be constructed (e.g. empty content or
    /// invalid line span). Callers should treat this as an internal fixture error
    /// and abort the calibration run.
    #[error("known-bad probe fixture invalid: {0}")]
    FixtureInvalid(String),
}

/// A single known-bad probe: a pair where the correct verdict is `Violation`.
pub struct KnownBadProbePair {
    /// The changed (diff-side) fragment — identical content to candidate in all probes.
    pub changed: CodeFragment,
    /// The candidate (existing-code) fragment — identical content to changed in all probes.
    pub candidate: CodeFragment,
}

/// Returns the fixed in-memory set of known-bad probe pairs, or an error if any
/// fixture fails construction.
///
/// These are synthetic Rust code fragments that are clear DRY violations
/// (identical or near-identical logic in two different files/functions).
/// The set is intentionally small (≤ 5 pairs) so probing is cheap.
///
/// # Errors
///
/// Returns [`KnownBadProbeError::FixtureInvalid`] when a probe's `CodeFragment`
/// cannot be constructed (e.g. empty content or invalid line span). Callers should
/// treat this as an internal fixture error and abort the calibration run rather
/// than silently reducing the probe count.
pub fn known_bad_probe_pairs() -> Result<Vec<KnownBadProbePair>, KnownBadProbeError> {
    let pairs: &[(&str, &str, &str, &str)] = &[
        // (changed_path, changed_content, candidate_path, candidate_content)
        (
            "probes/changed_a.rs",
            "fn compute_total(items: &[u32]) -> u32 { items.iter().sum() }",
            "probes/candidate_a.rs",
            "fn compute_total(items: &[u32]) -> u32 { items.iter().sum() }",
        ),
        (
            "probes/changed_b.rs",
            "fn parse_id(s: &str) -> Option<u64> { s.parse().ok() }",
            "probes/candidate_b.rs",
            "fn parse_id(s: &str) -> Option<u64> { s.parse().ok() }",
        ),
        (
            "probes/changed_c.rs",
            "fn is_valid_email(email: &str) -> bool { email.contains('@') && email.contains('.') }",
            "probes/candidate_c.rs",
            "fn is_valid_email(email: &str) -> bool { email.contains('@') && email.contains('.') }",
        ),
    ];

    let mut result = Vec::with_capacity(pairs.len());
    for (idx, (ch_path, ch_content, ca_path, ca_content)) in pairs.iter().enumerate() {
        let changed = CodeFragment::new(PathBuf::from(ch_path), (*ch_content).to_owned(), 1, 1)
            .map_err(|e| {
                KnownBadProbeError::FixtureInvalid(format!("probe[{idx}] changed fragment: {e}"))
            })?;
        let candidate = CodeFragment::new(PathBuf::from(ca_path), (*ca_content).to_owned(), 1, 1)
            .map_err(|e| {
            KnownBadProbeError::FixtureInvalid(format!("probe[{idx}] candidate fragment: {e}"))
        })?;
        result.push(KnownBadProbePair { changed, candidate });
    }
    Ok(result)
}
