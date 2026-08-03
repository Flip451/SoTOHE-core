//! Syn-based worktree scanner for bound test bodies.
//!
//! Implements [`TestSourceScannerPort`]: given a [`TestLocation`] recorded in the
//! test-bindings artifact (layer + module path + test function name), it locates
//! the test function in the worktree, extracts its body span verbatim, and hashes
//! it (IN-06 / IN-09 / AC-04, ADR D9). Test source stays plain Rust with no
//! embedded markers — freshness lives entirely in the artifact / verify cache, so
//! the scanner never edits source (ADR D9).
//!
//! Module-path resolution is convention-based: the first path segment is the
//! crate, whose source root is searched under `libs/<crate>/src` or
//! `apps/<crate>/src` (with `_`↔`-` crate-name normalisation). Candidate files
//! are derived from the remaining segments, most-specific first, and each is
//! parsed with `syn` to find a `#[test]`-attributed function of the given name.

use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use proc_macro2::LineColumn;
use sha2::Digest as _;
use syn::Item;

use domain::ContentHash;
use domain::tddd::test_obligation::binding::TestLocation;
use domain::tddd::test_obligation::errors::TestSourceScanError;
use domain::tddd::test_obligation::hashes::TestBodySpanHash;
use domain::tddd::test_obligation::ports::TestSourceScannerPort;

use crate::lexical_path::lexical_normalize;
use crate::test_obligation::diagnostic;
use crate::track::symlink_guard::reject_symlinks_below;

/// Secondary adapter that reads a bound test's body from the worktree via `syn`.
pub struct SynTestSourceScanner {
    workspace_root: PathBuf,
}

impl SynTestSourceScanner {
    /// Builds a scanner rooted at `workspace_root` (the repository root the
    /// module-path resolution is relative to).
    #[must_use]
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    /// Candidate crate source roots for `crate_name`, most-likely first.
    ///
    /// Handles both `libs/` (domain / usecase / infrastructure) and `apps/`
    /// (cli surfaces) members, and the `_`↔`-` mismatch between crate names and
    /// directory names (e.g. `cli_driver` → `apps/cli-driver`).
    fn crate_src_roots(workspace_root: &Path, crate_name: &str) -> Vec<PathBuf> {
        let hyphen = crate_name.replace('_', "-");
        let mut names = vec![crate_name.to_owned()];
        if hyphen != crate_name {
            names.push(hyphen);
        }
        let mut roots = Vec::new();
        for parent in ["libs", "apps"] {
            for name in &names {
                roots.push(workspace_root.join(parent).join(name).join("src"));
            }
        }
        roots
    }
}

fn guarded_workspace_root(workspace_root: &Path) -> Result<PathBuf, TestSourceScanError> {
    reject_parent_dir_workspace_root(workspace_root)?;
    let root = absolutize_lexical(workspace_root)?;
    reject_symlinked_workspace_root_chain(&root)?;
    // Keep the trusted-root anchor lexical. `canonicalize()` follows symlinked
    // ancestors and can silently re-anchor the scanner on the target tree.
    Ok(root)
}

/// Refuses a workspace root the caller wrote with a `..` step.
///
/// The root is the caller's own argument and has no repository-relative name to
/// report it by — naming it would print the very host path this convention keeps
/// out of diagnostics — so the refusal states what was wrong and nothing else.
fn reject_parent_dir_workspace_root(workspace_root: &Path) -> Result<(), TestSourceScanError> {
    if workspace_root.components().any(|component| component == Component::ParentDir) {
        return Err(TestSourceScanError::Io(diagnostic(
            "the test-source workspace root was refused (it contains a parent-dir component)",
        )));
    }
    Ok(())
}

/// Refuses a workspace root reached through a symlink, or one whose components
/// cannot be inspected.
///
/// Neither the offending component nor the stat failure is named: both render
/// absolute host paths, and the caller supplied the root they would describe.
fn reject_symlinked_workspace_root_chain(root: &Path) -> Result<(), TestSourceScanError> {
    let mut ancestors: Vec<&Path> = root.ancestors().collect();
    ancestors.reverse();
    for component in ancestors {
        if component.as_os_str().is_empty() {
            continue;
        }
        match component.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(TestSourceScanError::Io(diagnostic(
                    "the test-source workspace root was refused (rejected as a symlink)",
                )));
            }
            Ok(_) => {}
            Err(source) => {
                return Err(TestSourceScanError::Io(diagnostic(&format!(
                    "the test-source workspace root was refused ({})",
                    crate::sanitized_failure::io_classification(&source)
                ))));
            }
        }
    }
    Ok(())
}

fn parse_module_path_segments(module_path: &str) -> Result<Vec<&str>, TestSourceScanError> {
    // Bounded before the string is split, so an oversized path is refused rather
    // than measured by the work it would cause.
    if module_path.len() > MAX_MODULE_PATH_BYTES {
        return Err(TestSourceScanError::Io(diagnostic(
            "the test module path was refused (longer than a module path can be)",
        )));
    }
    let segments: Vec<&str> = module_path.split("::").collect();
    if segments.len() > MAX_MODULE_PATH_SEGMENTS {
        return Err(TestSourceScanError::Io(diagnostic(
            "the test module path was refused (more segments than a module path can name)",
        )));
    }
    for segment in &segments {
        if !safe_module_path_segment(segment) {
            return Err(TestSourceScanError::Io(diagnostic(&format!(
                "unsafe test module path segment '{segment}' in '{module_path}'"
            ))));
        }
    }
    Ok(segments)
}

fn safe_module_path_segment(segment: &str) -> bool {
    if segment.is_empty()
        || segment == "."
        || segment == ".."
        || segment.contains('/')
        || segment.contains('\\')
    {
        return false;
    }
    let mut components = Path::new(segment).components();
    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

/// Resolves `path` to an absolute lexical form.
///
/// # Errors
///
/// Returns [`TestSourceScanError::Io`] when a relative root cannot be made
/// absolute. Continuing with the relative path would drop the absolute trust
/// anchor every containment check below rests on, so there is no fallback.
fn absolutize_lexical(path: &Path) -> Result<PathBuf, TestSourceScanError> {
    absolutize_lexical_from(path, std::env::current_dir())
}

/// The body of [`absolutize_lexical`], with the working directory supplied so the
/// failure lane can be exercised without breaking the process's own.
fn absolutize_lexical_from(
    path: &Path,
    current_dir: std::io::Result<PathBuf>,
) -> Result<PathBuf, TestSourceScanError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        let cwd = current_dir.map_err(|error| {
            TestSourceScanError::Io(diagnostic(&format!(
                "the workspace root could not be resolved ({})",
                crate::sanitized_failure::io_classification(&error)
            )))
        })?;
        cwd.join(path)
    };
    Ok(lexical_normalize(&absolute))
}

/// The most a module path may occupy.
///
/// Rust's own limits are far below this — a path naming a test module runs to a
/// few dozen bytes — so a real one never meets it, while a caller-supplied string
/// cannot drive the candidate generation below into arbitrary work.
const MAX_MODULE_PATH_BYTES: usize = 4096;

/// The most segments a module path may name, for the same reason: candidate
/// generation is quadratic in the segment count, which is harmless only while the
/// count is bounded.
const MAX_MODULE_PATH_SEGMENTS: usize = 64;

/// The most a Rust source file this scanner will parse can occupy.
///
/// Well above anything in this workspace — the largest module here is a few
/// hundred kilobytes — so a real file is never refused, while a file that could
/// exhaust memory before `syn` ever sees it is.
const MAX_TEST_SOURCE_BYTES: u64 = 8 * 1024 * 1024;

/// Reads a candidate, refusing anything that is not a regular file within the cap.
///
/// Returns `Ok(None)` only when the candidate is absent: the path is speculative
/// and most candidates do not exist, so the scan moves on to the next one. Any
/// other outcome is an error rather than a skip, because a candidate that exists
/// but cannot be read as source would otherwise let the scan pass over a bound
/// test and report it missing.
///
/// # Errors
///
/// Returns [`TestSourceScanError::Io`] for a candidate that is not a regular
/// file, one above the cap, or a read that fails — named relatively and
/// classified.
fn read_bounded_source(
    file: &Path,
    workspace_root: &Path,
) -> Result<Option<String>, TestSourceScanError> {
    let metadata = match std::fs::symlink_metadata(file) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(TestSourceScanError::Io(diagnostic(&format!(
                "cannot read test source '{}': {}",
                candidate_identity(file, workspace_root),
                crate::sanitized_failure::io_classification(&e)
            ))));
        }
    };
    // Refused rather than skipped, because only absence may be skipped: a
    // candidate that exists but cannot be read as source would otherwise let the
    // scan pass over a bound test and report it missing. The check happens on the
    // metadata, so a FIFO is settled before anything opens it and blocks.
    if !metadata.file_type().is_file() {
        return Err(TestSourceScanError::Io(diagnostic(&format!(
            "cannot read test source '{}': not a regular file",
            candidate_identity(file, workspace_root)
        ))));
    }
    if metadata.len() > MAX_TEST_SOURCE_BYTES {
        return Err(TestSourceScanError::Io(diagnostic(&format!(
            "cannot read test source '{}': larger than a source file this scanner parses",
            candidate_identity(file, workspace_root)
        ))));
    }

    // Bounded regardless of the metadata: the file may grow between the two.
    let handle = match std::fs::File::open(file) {
        Ok(handle) => handle,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(TestSourceScanError::Io(diagnostic(&format!(
                "cannot read test source '{}': {}",
                candidate_identity(file, workspace_root),
                crate::sanitized_failure::io_classification(&e)
            ))));
        }
    };
    use std::io::Read as _;
    let mut content = String::new();
    handle.take(MAX_TEST_SOURCE_BYTES.saturating_add(1)).read_to_string(&mut content).map_err(
        |e| {
            TestSourceScanError::Io(diagnostic(&format!(
                "cannot read test source '{}': {}",
                candidate_identity(file, workspace_root),
                crate::sanitized_failure::io_classification(&e)
            )))
        },
    )?;
    if content.len() as u64 > MAX_TEST_SOURCE_BYTES {
        return Err(TestSourceScanError::Io(diagnostic(&format!(
            "cannot read test source '{}': larger than a source file this scanner parses",
            candidate_identity(file, workspace_root)
        ))));
    }

    Ok(Some(content))
}

/// Names a candidate by its path relative to the workspace root.
///
/// The absolute path describes the machine the scan ran on; the relative one
/// describes the repository, which is what a caller can act on. A candidate that
/// does not sit under the root has no relative name, and reporting the absolute
/// one is exactly what must not happen, so it is named by what it was looked up
/// as instead.
fn candidate_identity(file: &Path, workspace_root: &Path) -> String {
    file.strip_prefix(workspace_root).map_or_else(
        |_| "a path outside the workspace".to_owned(),
        |relative| relative.display().to_string(),
    )
}

fn guard_candidate_file(
    file: &Path,
    workspace_root: &Path,
) -> Result<Option<PathBuf>, TestSourceScanError> {
    let guarded_file = lexical_normalize(file);
    if !guarded_file.starts_with(workspace_root) {
        return Err(TestSourceScanError::Io(diagnostic(
            "test source path escapes the workspace root",
        )));
    }
    match reject_symlinks_below(&guarded_file, workspace_root) {
        Ok(true) => Ok(Some(guarded_file)),
        Ok(false) => Ok(None),
        // The guard's own message renders the absolute component it refused.
        Err(source) => Err(TestSourceScanError::Io(diagnostic(&format!(
            "refusing to read test source {}: {}",
            candidate_identity(&guarded_file, workspace_root),
            crate::sanitized_failure::io_classification(&source)
        )))),
    }
}

/// Candidate source files for the module segments after the crate, most-specific
/// first.
///
/// For `rest = [a, b]` under `src`, tries `a/b.rs`, `a/b/mod.rs`, `a.rs`,
/// `a/mod.rs`, then the crate roots `lib.rs` / `main.rs`. This intentionally
/// tries both external `tests.rs` modules and inline `mod tests` blocks.
struct CandidateFile {
    path: PathBuf,
    inline_modules: Vec<String>,
}

fn candidate_files(src_root: &Path, rest: &[&str]) -> Vec<CandidateFile> {
    let mut files = Vec::new();
    for len in (1..=rest.len()).rev() {
        let Some(prefix) = rest.get(..len) else {
            continue;
        };
        let mut base = src_root.to_path_buf();
        for seg in prefix {
            base.push(seg);
        }
        let inline_modules: Vec<String> = match rest.get(len..) {
            Some(tail) => tail.iter().map(|segment| (*segment).to_owned()).collect(),
            None => Vec::new(),
        };
        files.push(CandidateFile {
            path: base.with_extension("rs"),
            inline_modules: inline_modules.clone(),
        });
        files.push(CandidateFile { path: base.join("mod.rs"), inline_modules });
    }
    let root_inline_modules: Vec<String> =
        rest.iter().map(|segment| (*segment).to_owned()).collect();
    files.push(CandidateFile {
        path: src_root.join("lib.rs"),
        inline_modules: root_inline_modules.clone(),
    });
    files.push(CandidateFile {
        path: src_root.join("main.rs"),
        inline_modules: root_inline_modules,
    });
    files
}

/// Returns `true` when `attrs` carries a `#[test]`-family attribute
/// (`#[test]`, `#[tokio::test]`, `#[async_std::test]`, …).
fn has_test_attr(attrs: &[syn::Attribute]) -> bool {
    attrs
        .iter()
        .any(|attr| attr.path().segments.last().is_some_and(|segment| segment.ident == "test"))
}

fn find_test_span_in_items(
    items: &[Item],
    target: &str,
    inline_modules: &[String],
) -> Option<(LineColumn, LineColumn)> {
    if let Some((head, tail)) = inline_modules.split_first() {
        for item in items {
            let Item::Mod(module) = item else {
                continue;
            };
            if module.ident != head.as_str() {
                continue;
            }
            let Some((_, nested_items)) = &module.content else {
                continue;
            };
            if let Some(span) = find_test_span_in_items(nested_items, target, tail) {
                return Some(span);
            }
        }
        return None;
    }

    for item in items {
        let Item::Fn(function) = item else {
            continue;
        };
        if function.sig.ident == target && has_test_attr(&function.attrs) {
            let delim = function.block.brace_token.span;
            return Some((delim.open().start(), delim.close().end()));
        }
    }
    None
}

/// Byte offsets of the first character of each 1-based line in `content`.
fn line_start_offsets(content: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    for (i, byte) in content.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(i + 1);
        }
    }
    starts
}

/// Resolves a `syn` [`LineColumn`] (1-based line, 0-based char column) to a byte
/// offset into `content`. A column past the line's characters clamps to the
/// line's newline (or end of content).
fn line_col_to_byte(content: &str, starts: &[usize], lc: LineColumn) -> Option<usize> {
    let line_start = *starts.get(lc.line.checked_sub(1)?)?;
    let tail = content.get(line_start..)?;
    for (chars, (i, c)) in tail.char_indices().enumerate() {
        if chars == lc.column || c == '\n' {
            return Some(line_start + i);
        }
    }
    Some(content.len())
}

/// Extracts the verbatim source between two `LineColumn` positions.
fn slice_span(content: &str, start: LineColumn, end: LineColumn) -> Option<String> {
    let starts = line_start_offsets(content);
    let start_byte = line_col_to_byte(content, &starts, start)?;
    let end_byte = line_col_to_byte(content, &starts, end)?;
    if start_byte > end_byte {
        return None;
    }
    content.get(start_byte..end_byte).map(str::to_owned)
}

/// Finds the `target` test function's body span in `content`, if present.
///
/// # Errors
///
/// Returns [`TestSourceScanError::Parse`] when `content` is not parseable Rust.
fn scan_body_in_file(
    content: &str,
    target: &str,
    inline_modules: &[String],
) -> Result<Option<String>, TestSourceScanError> {
    let file = syn::parse_file(content).map_err(|e| {
        TestSourceScanError::Parse(diagnostic(&format!("cannot parse test source: {e}")))
    })?;
    Ok(find_test_span_in_items(&file.items, target, inline_modules)
        .and_then(|(start, end)| slice_span(content, start, end)))
}

impl TestSourceScannerPort for SynTestSourceScanner {
    fn scan_test_body(
        &self,
        location: &TestLocation,
    ) -> Result<Option<String>, TestSourceScanError> {
        let workspace_root = guarded_workspace_root(&self.workspace_root)?;
        let module_path = location.module_path().as_str();
        let segments = parse_module_path_segments(module_path)?;
        let Some((crate_name, rest)) = segments.split_first() else {
            return Ok(None);
        };
        let test_name = location.test_name().as_str();

        for src_root in Self::crate_src_roots(&workspace_root, crate_name) {
            for candidate in candidate_files(&src_root, rest) {
                let Some(file) = guard_candidate_file(&candidate.path, &workspace_root)? else {
                    continue;
                };
                let Some(content) = read_bounded_source(&file, &workspace_root)? else {
                    // A candidate can disappear between the symlink check and
                    // the read. It is still only a speculative path, so allow
                    // the remaining candidates to provide the bound test.
                    continue;
                };
                if let Some(body) =
                    scan_body_in_file(&content, test_name, &candidate.inline_modules)?
                {
                    return Ok(Some(body));
                }
            }
        }
        Ok(None)
    }

    fn hash_test_body(&self, source: &str) -> TestBodySpanHash {
        let mut hasher = sha2::Sha256::new();
        hasher.update(source.as_bytes());
        let digest = hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        TestBodySpanHash::new(ContentHash::from_bytes(out))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use domain::tddd::layer_id::LayerId;
    use domain::tddd::test_obligation::ids::{TestFunctionName, TestModulePath};

    use super::*;

    fn location(layer: &str, module_path: &str, test_name: &str) -> TestLocation {
        TestLocation::new(
            LayerId::try_new(layer).unwrap(),
            TestModulePath::try_new(module_path.to_owned()).unwrap(),
            TestFunctionName::try_new(test_name.to_owned()).unwrap(),
        )
    }

    /// Writes `content` to `<root>/libs/<crate>/src/<rel>` and returns the root.
    fn write_lib_source(crate_name: &str, rel: &str, content: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("libs").join(crate_name).join("src").join(rel);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, content).unwrap();
        dir
    }

    const SOURCE_WITH_TESTS: &str = r#"pub struct User;

#[cfg(test)]
mod tests {
    #[test]
    fn test_rejects_empty() {
        let value = 1 + 1;
        assert_eq!(value, 2);
    }

    #[test]
    fn test_other() {
        assert!(true);
    }
}
"#;

    #[test]
    fn scans_named_test_body_from_worktree() {
        let dir = write_lib_source("domain", "user.rs", SOURCE_WITH_TESTS);
        let scanner = SynTestSourceScanner::new(dir.path().to_path_buf());
        let body = scanner
            .scan_test_body(&location("domain", "domain::user::tests", "test_rejects_empty"))
            .unwrap()
            .unwrap();
        assert!(body.starts_with('{'));
        assert!(body.trim_end().ends_with('}'));
        assert!(body.contains("assert_eq!(value, 2)"));
        // The other test's body must not bleed in.
        assert!(!body.contains("assert!(true)"));
    }

    #[test]
    fn resolves_via_mod_rs_layout() {
        let dir = write_lib_source("domain", "user/mod.rs", SOURCE_WITH_TESTS);
        let scanner = SynTestSourceScanner::new(dir.path().to_path_buf());
        let body = scanner
            .scan_test_body(&location("domain", "domain::user::tests", "test_other"))
            .unwrap()
            .unwrap();
        assert!(body.contains("assert!(true)"));
    }

    #[test]
    fn resolves_external_tests_rs_module_before_inline_tests_fallback() {
        let dir = write_lib_source(
            "usecase",
            "test_obligation/evaluate/tests.rs",
            r#"#[test]
fn test_external_module() {
    assert_eq!("tests.rs", "tests.rs");
}
"#,
        );
        let scanner = SynTestSourceScanner::new(dir.path().to_path_buf());
        let body = scanner
            .scan_test_body(&location(
                "usecase",
                "usecase::test_obligation::evaluate::tests",
                "test_external_module",
            ))
            .unwrap()
            .unwrap();
        assert!(body.contains(r#"assert_eq!("tests.rs", "tests.rs")"#));
    }

    #[test]
    fn returns_none_when_test_absent() {
        let dir = write_lib_source("domain", "user.rs", SOURCE_WITH_TESTS);
        let scanner = SynTestSourceScanner::new(dir.path().to_path_buf());
        let result = scanner
            .scan_test_body(&location("domain", "domain::user::tests", "test_missing"))
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn returns_none_when_file_absent() {
        let dir = tempfile::tempdir().unwrap();
        let scanner = SynTestSourceScanner::new(dir.path().to_path_buf());
        let result = scanner
            .scan_test_body(&location("domain", "domain::nothing::tests", "test_x"))
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_candidate_probe_non_not_found_read_error_aborts_before_fallback() {
        let dir = write_lib_source("domain", "user.rs", SOURCE_WITH_TESTS);
        let unreadable_candidate = dir.path().join("libs/domain/src/user/tests.rs");
        std::fs::create_dir_all(&unreadable_candidate).unwrap();

        let scanner = SynTestSourceScanner::new(dir.path().to_path_buf());
        let err = scanner
            .scan_test_body(&location("domain", "domain::user::tests", "test_rejects_empty"))
            .unwrap_err();

        let TestSourceScanError::Io(message) = err else {
            panic!("expected read failure");
        };
        assert!(message.as_str().contains("cannot read test source"));
    }

    #[test]
    fn fallback_root_requires_matching_inline_module_path() {
        let source = r#"pub mod other {
    #[cfg(test)]
    mod tests {
        #[test]
        fn test_x() {
            assert!(false);
        }
    }
}
"#;
        let dir = write_lib_source("domain", "lib.rs", source);
        let scanner = SynTestSourceScanner::new(dir.path().to_path_buf());
        let result = scanner
            .scan_test_body(&location("domain", "domain::missing::tests", "test_x"))
            .unwrap();
        assert!(result.is_none(), "same-named tests in other modules must not match");
    }

    #[test]
    fn rejects_parent_dir_module_path_segment() {
        let dir = tempfile::tempdir().unwrap();
        let scanner = SynTestSourceScanner::new(dir.path().to_path_buf());
        let err =
            scanner.scan_test_body(&location("domain", "domain::..::tests", "test_x")).unwrap_err();
        let TestSourceScanError::Io(message) = err else {
            panic!("expected IO guard error");
        };
        assert!(message.as_str().contains("unsafe test module path segment"));
    }

    #[test]
    fn rejects_absolute_module_path_segment() {
        let dir = tempfile::tempdir().unwrap();
        let scanner = SynTestSourceScanner::new(dir.path().to_path_buf());
        let err = scanner
            .scan_test_body(&location("domain", "domain::/tmp::tests", "test_x"))
            .unwrap_err();
        let TestSourceScanError::Io(message) = err else {
            panic!("expected IO guard error");
        };
        assert!(message.as_str().contains("unsafe test module path segment"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_source_file() {
        let dir = tempfile::tempdir().unwrap();
        let src_dir = dir.path().join("libs/domain/src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let real_file = dir.path().join("real_user.rs");
        std::fs::write(&real_file, SOURCE_WITH_TESTS).unwrap();
        std::os::unix::fs::symlink(&real_file, src_dir.join("user.rs")).unwrap();

        let scanner = SynTestSourceScanner::new(dir.path().to_path_buf());
        let err = scanner
            .scan_test_body(&location("domain", "domain::user::tests", "test_rejects_empty"))
            .unwrap_err();
        let TestSourceScanError::Io(message) = err else {
            panic!("expected IO guard error");
        };
        assert!(message.as_str().contains("refusing to read test source"));
        // The refusal names the candidate by its repository-relative path and
        // classifies the cause; neither the absolute candidate, the workspace root,
        // nor the guard's own path-bearing message may reach a caller.
        assert!(
            message.as_str().contains("libs/domain/src/user.rs"),
            "names the candidate relatively: {}",
            message.as_str()
        );
        assert!(
            message.as_str().contains("rejected as a symlink"),
            "classified: {}",
            message.as_str()
        );
        assert!(
            !message.as_str().contains(&dir.path().display().to_string()),
            "no absolute path may reach the caller: {}",
            message.as_str()
        );
    }

    #[test]
    fn test_a_relative_root_is_refused_when_the_working_directory_cannot_be_read() {
        // Continuing with the relative path would drop the absolute anchor every
        // containment check below rests on, so there is no fallback. The cwd is
        // supplied rather than broken for real, which would derail the whole run.
        let err = absolutize_lexical_from(
            Path::new("relative/root"),
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
        )
        .expect_err("a relative root with no working directory must be refused");

        let TestSourceScanError::Io(message) = err else {
            panic!("expected IO guard error");
        };
        assert!(
            message.as_str().contains("workspace root could not be resolved"),
            "{}",
            message.as_str()
        );
        assert!(message.as_str().contains("permission denied"), "{}", message.as_str());

        // An absolute root never consults the working directory, so its failure
        // cannot reach that lane at all.
        assert_eq!(
            absolutize_lexical_from(
                Path::new("/repo/./root"),
                Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
            )
            .unwrap(),
            PathBuf::from("/repo/root")
        );
    }

    #[test]
    fn test_a_module_path_past_its_budget_is_refused_before_candidates_are_built() {
        // At the budget: accepted, so the refusal follows the excess and not the
        // shape of the path.
        let at_budget = vec!["m"; MAX_MODULE_PATH_SEGMENTS].join("::");
        assert_eq!(parse_module_path_segments(&at_budget).unwrap().len(), MAX_MODULE_PATH_SEGMENTS);

        // One segment past it: refused. Candidate generation is quadratic in the
        // segment count, so this is the bound that keeps it harmless.
        let too_many = vec!["m"; MAX_MODULE_PATH_SEGMENTS + 1].join("::");
        let err = parse_module_path_segments(&too_many)
            .expect_err("a module path past the segment budget must be refused");
        let TestSourceScanError::Io(message) = err else {
            panic!("expected IO guard error");
        };
        assert!(message.as_str().contains("more segments"), "{}", message.as_str());

        // And the byte budget, which bounds the string before it is even split.
        let too_long = "m".repeat(MAX_MODULE_PATH_BYTES + 1);
        let err = parse_module_path_segments(&too_long)
            .expect_err("a module path past the byte budget must be refused");
        let TestSourceScanError::Io(message) = err else {
            panic!("expected IO guard error");
        };
        assert!(message.as_str().contains("longer than"), "{}", message.as_str());
    }

    #[test]
    fn test_a_candidate_outside_the_workspace_is_refused_without_naming_either_path() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();

        let err = guard_candidate_file(&outside.path().join("user.rs"), dir.path())
            .expect_err("a candidate outside the workspace root must be refused");

        let TestSourceScanError::Io(message) = err else {
            panic!("expected IO guard error");
        };
        assert!(
            message.as_str().contains("escapes the workspace root"),
            "got: {}",
            message.as_str()
        );
        assert!(
            !message.as_str().contains(&dir.path().display().to_string())
                && !message.as_str().contains(&outside.path().display().to_string()),
            "neither path may reach the caller: {}",
            message.as_str()
        );
    }

    #[test]
    fn test_a_candidate_larger_than_the_scanner_parses_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("libs/domain/src/user.rs");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        // A sparse file one byte past the cap: `set_len` reserves the length
        // without writing the bytes, so the oversize case is exercised for real at
        // no I/O cost.
        std::fs::File::create(&file).unwrap().set_len(MAX_TEST_SOURCE_BYTES + 1).unwrap();

        let err = read_bounded_source(&file, dir.path())
            .expect_err("a candidate past the cap must be refused");

        let TestSourceScanError::Io(message) = err else {
            panic!("expected IO guard error");
        };
        assert!(
            message.as_str().contains("larger than a source file this scanner parses"),
            "got: {}",
            message.as_str()
        );
        assert!(
            message.as_str().contains("libs/domain/src/user.rs"),
            "names the candidate relatively: {}",
            message.as_str()
        );

        // A file inside the cap still reads, so the refusal follows the size and
        // not the fixture.
        std::fs::write(&file, "x".repeat(64)).unwrap();
        assert_eq!(
            read_bounded_source(&file, dir.path()).unwrap().map(|content| content.len()),
            Some(64)
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_a_candidate_that_is_not_a_regular_file_is_refused_rather_than_opened() {
        // Opening a FIFO blocks until a writer arrives, so the type is settled
        // from the metadata first. Refused rather than skipped: a candidate that
        // exists but cannot be read must not let the scan report the test missing.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("libs/domain/src/user.rs");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        rustix::fs::mkfifoat(rustix::fs::CWD, &file, rustix::fs::Mode::from_raw_mode(0o600))
            .unwrap();

        let err =
            read_bounded_source(&file, dir.path()).expect_err("a FIFO candidate must be refused");

        let TestSourceScanError::Io(message) = err else {
            panic!("expected IO guard error");
        };
        assert!(message.as_str().contains("not a regular file"), "got: {}", message.as_str());
        assert!(
            message.as_str().contains("libs/domain/src/user.rs"),
            "names the candidate relatively: {}",
            message.as_str()
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_workspace_root() {
        let dir = tempfile::tempdir().unwrap();
        let real_root = dir.path().join("real-root");
        let link_root = dir.path().join("link-root");
        let src_dir = real_root.join("libs/domain/src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(src_dir.join("user.rs"), SOURCE_WITH_TESTS).unwrap();
        std::os::unix::fs::symlink(&real_root, &link_root).unwrap();

        let scanner = SynTestSourceScanner::new(link_root);
        let err = scanner
            .scan_test_body(&location("domain", "domain::user::tests", "test_rejects_empty"))
            .unwrap_err();
        let TestSourceScanError::Io(message) = err else {
            panic!("expected IO guard error");
        };
        assert!(message.as_str().contains("workspace root was refused"), "{}", message.as_str());
        assert!(message.as_str().contains("rejected as a symlink"), "{}", message.as_str());
        // The root is the caller's own argument: naming it, or the component the
        // guard refused, would print the host path this reports around.
        assert!(
            !message.as_str().contains(&dir.path().display().to_string()),
            "no absolute path may reach the caller: {}",
            message.as_str()
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_workspace_root_parent() {
        let dir = tempfile::tempdir().unwrap();
        let real_parent = dir.path().join("real-parent");
        let link_parent = dir.path().join("link-parent");
        let root_via_link_parent = link_parent.join("repo");
        std::fs::create_dir_all(real_parent.join("repo")).unwrap();
        std::os::unix::fs::symlink(&real_parent, &link_parent).unwrap();

        let err = guarded_workspace_root(&root_via_link_parent).unwrap_err();
        let TestSourceScanError::Io(message) = err else {
            panic!("expected IO guard error");
        };
        assert!(message.as_str().contains("workspace root was refused"), "{}", message.as_str());
        assert!(message.as_str().contains("rejected as a symlink"), "{}", message.as_str());
        assert!(
            !message.as_str().contains(&dir.path().display().to_string()),
            "no absolute path may reach the caller: {}",
            message.as_str()
        );
    }

    #[test]
    fn rejects_parent_dir_workspace_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        let root_with_parent = dir.path().join("other").join("..").join("repo");

        let err = guarded_workspace_root(&root_with_parent).unwrap_err();
        let TestSourceScanError::Io(message) = err else {
            panic!("expected IO guard error");
        };
        assert!(message.as_str().contains("workspace root was refused"), "{}", message.as_str());
        assert!(message.as_str().contains("parent-dir component"), "{}", message.as_str());
        assert!(
            !message.as_str().contains(&dir.path().display().to_string()),
            "no absolute path may reach the caller: {}",
            message.as_str()
        );
        let _ = &root;
    }

    #[test]
    fn non_test_function_is_not_matched() {
        let source = "fn test_rejects_empty() { let _ = 1; }\n";
        let dir = write_lib_source("domain", "user.rs", source);
        let scanner = SynTestSourceScanner::new(dir.path().to_path_buf());
        let result = scanner
            .scan_test_body(&location("domain", "domain::user", "test_rejects_empty"))
            .unwrap();
        assert!(result.is_none(), "a function without #[test] must not match");
    }

    #[test]
    fn parse_error_is_reported() {
        let dir = write_lib_source("domain", "user.rs", "fn broken( {");
        let scanner = SynTestSourceScanner::new(dir.path().to_path_buf());
        let err =
            scanner.scan_test_body(&location("domain", "domain::user", "whatever")).unwrap_err();
        assert!(matches!(err, TestSourceScanError::Parse(_)));
    }

    #[test]
    fn hash_is_deterministic_and_content_sensitive() {
        let dir = tempfile::tempdir().unwrap();
        let scanner = SynTestSourceScanner::new(dir.path().to_path_buf());
        let a = scanner.hash_test_body("{ assert!(true); }");
        let b = scanner.hash_test_body("{ assert!(true); }");
        let c = scanner.hash_test_body("{ assert!(false); }");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn scanned_body_hash_matches_direct_hash() {
        let dir = write_lib_source("domain", "user.rs", SOURCE_WITH_TESTS);
        let scanner = SynTestSourceScanner::new(dir.path().to_path_buf());
        let body = scanner
            .scan_test_body(&location("domain", "domain::user::tests", "test_rejects_empty"))
            .unwrap()
            .unwrap();
        let via_scan = scanner.hash_test_body(&body);
        let direct = scanner.hash_test_body(&body);
        assert_eq!(via_scan, direct);
    }

    #[test]
    fn slice_span_extracts_expected_region() {
        let content = "line one\nsecond line\nthird\n";
        // From line 2 col 0 to line 2 col 6 → "second".
        let start = LineColumn { line: 2, column: 0 };
        let end = LineColumn { line: 2, column: 6 };
        assert_eq!(slice_span(content, start, end).as_deref(), Some("second"));
    }
}
