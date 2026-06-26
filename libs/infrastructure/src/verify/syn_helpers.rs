//! Shared syn-based AST helpers used across verify submodules.

use std::path::Path;

use domain::verify::{VerifyFinding, VerifyOutcome};

/// Returns `true` if `attrs` contains an exact `#[cfg(test)]` attribute.
///
/// Only exact `cfg(test)` marks code as test-only. Broader expressions such as
/// `cfg(not(test))` or `cfg(any(test, feature = "test-helpers"))` can include
/// production code and must not be excluded from production checks.
pub(crate) fn has_cfg_test_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("cfg") {
            return false;
        }
        attr.parse_args::<syn::Path>().is_ok_and(|path| path.is_ident("test"))
    })
}

/// Recursively scan all `.rs` files under `root`, calling `on_file` for each
/// parseable, non-test-only file. Returns a [`VerifyOutcome`] aggregating all
/// findings returned by `on_file` across every file.
///
/// Files whose top-level inner attribute list includes `#![cfg(test)]` are
/// skipped in their entirety. Item-level test exclusion (e.g. skipping items
/// inside `#[cfg(test)]` blocks or carrying `#[test]`) is the caller's
/// responsibility.
///
/// Parse errors and unreadable files are silently ignored; the caller's
/// check logic should rely on Rust's own compiler for syntax validation.
/// Symlinked paths are reported as error findings and skipped before any
/// directory traversal or file read can follow them.
pub(crate) fn scan_rs_files(
    root: &Path,
    mut on_file: impl FnMut(&Path, &syn::File) -> Vec<VerifyFinding>,
) -> VerifyOutcome {
    let mut findings = Vec::new();
    if let Some(finding) = reject_symlink_entry(root) {
        findings.push(finding);
        return VerifyOutcome::from_findings(findings);
    }
    visit_rs_files(root, &mut on_file, &mut findings);
    VerifyOutcome::from_findings(findings)
}

/// Internal recursive walker used by [`scan_rs_files`].
fn visit_rs_files(
    dir: &Path,
    on_file: &mut impl FnMut(&Path, &syn::File) -> Vec<VerifyFinding>,
    findings: &mut Vec<VerifyFinding>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return, // Silently skip unreadable directories.
    };

    let mut paths: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    paths.sort(); // Deterministic order for reproducible output.

    for path in paths {
        let metadata = match path.symlink_metadata() {
            Ok(meta) => meta,
            Err(e) => {
                findings.push(VerifyFinding::error(format!(
                    "verify rust source scan: failed to stat {}: {e}",
                    path.display()
                )));
                continue;
            }
        };

        if metadata.file_type().is_symlink() {
            findings.push(VerifyFinding::error(format!(
                "verify rust source scan: refusing to follow symlink: {}",
                path.display()
            )));
            continue;
        }

        if metadata.is_dir() {
            visit_rs_files(&path, on_file, findings);
        } else if metadata.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue, // Silently skip unreadable files.
            };
            let ast = match syn::parse_file(&content) {
                Ok(f) => f,
                Err(_) => continue, // Silently skip files with syntax errors.
            };
            // Skip files that are entirely test-only (`#![cfg(test)]`).
            if has_cfg_test_attr(&ast.attrs) {
                continue;
            }
            let mut file_findings = on_file(&path, &ast);
            findings.append(&mut file_findings);
        }
    }
}

fn reject_symlink_entry(path: &Path) -> Option<VerifyFinding> {
    match path.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => Some(VerifyFinding::error(format!(
            "verify rust source scan: refusing to follow symlink: {}",
            path.display()
        ))),
        Ok(_) => None,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => Some(VerifyFinding::error(format!(
            "verify rust source scan: failed to stat {}: {e}",
            path.display()
        ))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn test_scan_rs_files_symlinked_file_reports_error() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real.rs");
        let link = tmp.path().join("link.rs");
        std::fs::write(&real, "pub fn hidden_source() {}\n").unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let outcome = scan_rs_files(tmp.path(), |_, _| Vec::new());

        assert!(outcome.has_errors(), "expected symlink error: {outcome:?}");
        let msg = outcome.findings().first().map(ToString::to_string).unwrap_or_default();
        assert!(
            msg.contains("refusing to follow symlink"),
            "message missing symlink reason: {msg}"
        );
        assert!(msg.contains("link.rs"), "message missing symlink path: {msg}");
    }
}
