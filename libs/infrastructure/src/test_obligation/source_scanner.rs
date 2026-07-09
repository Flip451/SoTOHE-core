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

use std::path::{Component, Path, PathBuf};

use proc_macro2::LineColumn;
use sha2::Digest as _;
use syn::Item;

use domain::ContentHash;
use domain::tddd::test_obligation::binding::TestLocation;
use domain::tddd::test_obligation::errors::TestSourceScanError;
use domain::tddd::test_obligation::hashes::TestBodySpanHash;
use domain::tddd::test_obligation::ports::TestSourceScannerPort;

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
    let root = absolutize_lexical(workspace_root);
    reject_symlinked_workspace_root_chain(&root)?;
    // Keep the trusted-root anchor lexical. `canonicalize()` follows symlinked
    // ancestors and can silently re-anchor the scanner on the target tree.
    Ok(root)
}

fn reject_parent_dir_workspace_root(workspace_root: &Path) -> Result<(), TestSourceScanError> {
    if workspace_root.components().any(|component| component == Component::ParentDir) {
        return Err(TestSourceScanError::Io(diagnostic(&format!(
            "refusing to use test-source workspace root with parent-dir component: {}",
            workspace_root.display()
        ))));
    }
    Ok(())
}

fn reject_symlinked_workspace_root_chain(root: &Path) -> Result<(), TestSourceScanError> {
    let mut ancestors: Vec<&Path> = root.ancestors().collect();
    ancestors.reverse();
    for component in ancestors {
        if component.as_os_str().is_empty() {
            continue;
        }
        match component.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(TestSourceScanError::Io(diagnostic(&format!(
                    "refusing to use symlinked test-source workspace root component: {}",
                    component.display()
                ))));
            }
            Ok(_) => {}
            Err(source) => {
                return Err(TestSourceScanError::Io(diagnostic(&format!(
                    "cannot stat test-source workspace root component {}: {source}",
                    component.display()
                ))));
            }
        }
    }
    Ok(())
}

fn parse_module_path_segments(module_path: &str) -> Result<Vec<&str>, TestSourceScanError> {
    let segments: Vec<&str> = module_path.split("::").collect();
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

fn absolutize_lexical(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map(|cwd| cwd.join(path)).unwrap_or_else(|_| path.to_path_buf())
    };
    lexical_normalize(&absolute)
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut components: Vec<Component<'_>> = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => match components.last() {
                Some(Component::Normal(_)) => {
                    components.pop();
                }
                _ => components.push(component),
            },
            Component::CurDir => {}
            _ => components.push(component),
        }
    }
    components.iter().collect()
}

fn guard_candidate_file(
    file: &Path,
    workspace_root: &Path,
) -> Result<Option<PathBuf>, TestSourceScanError> {
    let guarded_file = lexical_normalize(file);
    if !guarded_file.starts_with(workspace_root) {
        return Err(TestSourceScanError::Io(diagnostic(&format!(
            "test source path {} escapes workspace root {}",
            file.display(),
            workspace_root.display()
        ))));
    }
    match reject_symlinks_below(&guarded_file, workspace_root) {
        Ok(true) => Ok(Some(guarded_file)),
        Ok(false) => Ok(None),
        Err(source) => Err(TestSourceScanError::Io(diagnostic(&format!(
            "refusing to read test source {}: {source}",
            guarded_file.display()
        )))),
    }
}

/// Candidate source files for the module segments after the crate, most-specific
/// first.
///
/// For `rest = [a, b]` under `src`, tries `a/b.rs`, `a/b/mod.rs`, `a.rs`,
/// `a/mod.rs`, then the crate roots `lib.rs` / `main.rs`. A trailing `tests`
/// segment (the conventional inline `#[cfg(test)] mod tests`) is dropped so the
/// enclosing file is reached.
struct CandidateFile {
    path: PathBuf,
    inline_modules: Vec<String>,
}

fn candidate_files(src_root: &Path, rest: &[&str]) -> Vec<CandidateFile> {
    let trimmed_len = match rest.split_last() {
        Some((&"tests", init)) => init.len(),
        _ => rest.len(),
    };

    let mut files = Vec::new();
    for len in (1..=trimmed_len).rev() {
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
                let content = match std::fs::read_to_string(&file) {
                    Ok(content) => content,
                    Err(e) => {
                        return Err(TestSourceScanError::Io(diagnostic(&format!(
                            "cannot read test source '{}': {e}",
                            file.display()
                        ))));
                    }
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
#[allow(clippy::unwrap_used, clippy::panic)]
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
        assert!(message.as_str().contains("symlinked test-source workspace root"));
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
        assert!(message.as_str().contains("symlinked test-source workspace root component"));
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
        assert!(message.as_str().contains("parent-dir component"));
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
