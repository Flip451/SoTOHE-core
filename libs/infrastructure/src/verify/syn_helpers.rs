//! Shared syn-based AST helpers used across verify submodules.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use domain::verify::{VerifyFinding, VerifyOutcome};

use super::path_safety::lexical_normalize;

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

/// Compute the basedir for resolving file-backed sub-modules declared in `file`.
///
/// Rust's module resolution rules (2018 edition):
/// - `mod.rs`, `lib.rs`, and `main.rs` are directory-root files: their
///   sub-modules are siblings in the same directory (basedir = `file.parent()`).
/// - All other `foo.rs` files: sub-modules live under a `foo/` subdirectory
///   (basedir = `file.parent()/foo`).
///
/// Returns `None` if the path has no parent (i.e., the filesystem root).
fn module_basedir(file: &Path) -> Option<PathBuf> {
    let file_name = file.file_name()?.to_str()?;
    let parent = file.parent()?;
    match file_name {
        "mod.rs" | "lib.rs" | "main.rs" => Some(parent.to_path_buf()),
        other => {
            let stem = Path::new(other).file_stem()?.to_str()?;
            Some(parent.join(stem))
        }
    }
}

/// Extract the literal string value of a `#[path = "..."]` attribute, if present.
///
/// Returns `Some(path_value)` when the attribute list contains exactly a
/// `#[path = "<literal>"]` `NameValue` form.  Returns `None` when no such
/// attribute exists.  `#[cfg_attr(..., path = "...")]` forms are not resolved
/// here; only the direct `#[path]` form is handled.
fn extract_path_attr_value(attrs: &[syn::Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| {
        if !attr.path().is_ident("path") {
            return None;
        }
        let syn::Meta::NameValue(nv) = &attr.meta else {
            return None;
        };
        let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &nv.value else {
            return None;
        };
        Some(s.value())
    })
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum ModuleScanContext {
    Production,
    TestOnly,
}

impl ModuleScanContext {
    fn with_attrs(self, attrs: &[syn::Attribute]) -> Self {
        if self.is_test_only() || has_cfg_test_attr(attrs) {
            Self::TestOnly
        } else {
            Self::Production
        }
    }

    fn is_test_only(self) -> bool {
        matches!(self, Self::TestOnly)
    }
}

fn is_real_file(path: &Path) -> bool {
    match path.symlink_metadata() {
        Ok(metadata) => !metadata.file_type().is_symlink() && metadata.is_file(),
        Err(_) => false,
    }
}

fn is_safe_module_file(path: &Path, scan_root: &Path) -> bool {
    path.starts_with(scan_root)
        && crate::track::symlink_guard::reject_symlinks_below(path, scan_root)
            .is_ok_and(|exists| exists)
        && is_real_file(path)
}

fn parse_real_rs_file(path: &Path) -> Option<syn::File> {
    if !is_real_file(path) {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    syn::parse_file(&content).ok()
}

fn module_candidate_paths(basedir: &Path, mod_item: &syn::ItemMod) -> [PathBuf; 2] {
    let mod_name = mod_item.ident.to_string();
    [basedir.join(format!("{mod_name}.rs")), basedir.join(&mod_name).join("mod.rs")]
}

fn collect_rs_files_from_dir(dir: &Path, rs_files: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return, // Silently skip unreadable directories.
    };

    let mut paths: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    paths.sort();

    for path in paths {
        let metadata = match path.symlink_metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if metadata.file_type().is_symlink() {
            continue; // Skip symlinks; Pass 2 reports them as errors.
        }
        if metadata.is_dir() {
            collect_rs_files_from_dir(&path, rs_files);
        } else if metadata.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            rs_files.push(path);
        }
    }
}

fn collect_module_child_files(rs_files: &[PathBuf], scan_root: &Path) -> HashSet<PathBuf> {
    let mut child_files = HashSet::new();
    for file in rs_files {
        let Some(ast) = parse_real_rs_file(file) else {
            continue;
        };
        let Some(basedir) = module_basedir(file) else {
            continue;
        };
        let Some(file_dir) = file.parent() else {
            continue;
        };
        collect_module_child_files_inner(
            &ast.items,
            &basedir,
            file_dir,
            scan_root,
            &mut child_files,
        );
    }
    child_files
}

fn collect_module_child_files_inner(
    items: &[syn::Item],
    basedir: &Path,
    file_dir: &Path,
    scan_root: &Path,
    child_files: &mut HashSet<PathBuf>,
) {
    for item in items {
        let syn::Item::Mod(mod_item) = item else {
            continue;
        };
        match &mod_item.content {
            None => {
                if let Some(path_val) = extract_path_attr_value(&mod_item.attrs) {
                    // Resolve #[path = "..."] relative to the containing file's directory.
                    // Lexically normalize to collapse any `..` components before the
                    // scan-root containment and symlink-ancestor checks to prevent
                    // path-traversal bypasses.
                    let resolved = lexical_normalize(&file_dir.join(&path_val));
                    if is_safe_module_file(&resolved, scan_root) {
                        child_files.insert(resolved);
                    }
                } else {
                    for candidate in module_candidate_paths(basedir, mod_item) {
                        if is_safe_module_file(&candidate, scan_root) {
                            child_files.insert(candidate);
                        }
                    }
                }
            }
            Some((_, inner_items)) => {
                let inner_basedir = basedir.join(mod_item.ident.to_string());
                // For inline modules, Rust resolves #[path] relative to the inline
                // module's basedir (the accumulated stack), not the containing file's
                // directory. Pass inner_basedir as the new path-resolution base.
                collect_module_child_files_inner(
                    inner_items,
                    &inner_basedir,
                    &inner_basedir,
                    scan_root,
                    child_files,
                );
            }
        }
    }
}

fn collect_module_refs_for_context(
    items: &[syn::Item],
    basedir: &Path,
    file_dir: &Path,
    scan_root: &Path,
    context: ModuleScanContext,
    worklist: &mut Vec<(PathBuf, ModuleScanContext)>,
) {
    for item in items {
        let syn::Item::Mod(mod_item) = item else {
            continue;
        };
        let child_context = context.with_attrs(&mod_item.attrs);
        match &mod_item.content {
            None => {
                if let Some(path_val) = extract_path_attr_value(&mod_item.attrs) {
                    // Resolve #[path = "..."] relative to the containing file's directory.
                    // Lexically normalize to collapse any `..` components before the
                    // scan-root containment and symlink-ancestor checks to prevent
                    // path-traversal bypasses.
                    let resolved = lexical_normalize(&file_dir.join(&path_val));
                    if is_safe_module_file(&resolved, scan_root) {
                        worklist.push((resolved, child_context));
                    }
                } else {
                    for candidate in module_candidate_paths(basedir, mod_item) {
                        if is_safe_module_file(&candidate, scan_root) {
                            worklist.push((candidate, child_context));
                        }
                    }
                }
            }
            Some((_, inner_items)) => {
                let inner_basedir = basedir.join(mod_item.ident.to_string());
                // For inline modules, Rust resolves #[path] relative to the inline
                // module's basedir (the accumulated stack), not the containing file's
                // directory. Pass inner_basedir as the new path-resolution base.
                collect_module_refs_for_context(
                    inner_items,
                    &inner_basedir,
                    &inner_basedir,
                    scan_root,
                    child_context,
                    worklist,
                );
            }
        }
    }
}

pub(crate) fn is_scan_crate_root(root: &Path, file: &Path) -> bool {
    let Some(file_name) = file.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    matches!(file_name, "lib.rs" | "main.rs") && file.parent() == Some(root)
}

/// Pass 1: collect the set of `.rs` file paths that are declared behind
/// `#[cfg(test)] mod X;` and must therefore be skipped during the production
/// code scan.
///
/// The returned set is transitively closed: any file added to the set is itself
/// scanned for further file-backed sub-module declarations so that their children
/// are also included.
///
/// `#[path = "..."]` attributes on file-backed `mod` declarations are resolved
/// relative to the containing file's directory, normalized lexically to prevent
/// `..`-traversal bypasses, and guarded by a scan-root containment check before
/// being added to the worklist.  Only exact `#[path]` forms are handled;
/// `#[cfg_attr(..., path = "...")]` is not resolved and those modules are skipped.
fn collect_cfg_test_files(root: &Path) -> HashSet<PathBuf> {
    let mut rs_files = Vec::new();
    collect_rs_files_from_dir(root, &mut rs_files);
    let module_child_files = collect_module_child_files(&rs_files, root);

    let mut worklist: Vec<(PathBuf, ModuleScanContext)> = rs_files
        .into_iter()
        .filter(|file| is_scan_crate_root(root, file) || !module_child_files.contains(file))
        .map(|file| (file, ModuleScanContext::Production))
        .collect();
    let mut cfg_test_files: HashSet<PathBuf> = HashSet::new();
    let mut production_files: HashSet<PathBuf> = HashSet::new();

    let mut visited: HashSet<(PathBuf, ModuleScanContext)> = HashSet::new();
    while let Some((file, context)) = worklist.pop() {
        if !visited.insert((file.clone(), context)) {
            continue; // Already processed — prevents cycles.
        }
        let Some(ast) = parse_real_rs_file(&file) else {
            continue;
        };
        let file_context = context.with_attrs(&ast.attrs);
        if file_context.is_test_only() {
            cfg_test_files.insert(file.clone());
        } else {
            production_files.insert(file.clone());
        }
        if let (Some(basedir), Some(file_dir)) = (module_basedir(&file), file.parent()) {
            collect_module_refs_for_context(
                &ast.items,
                &basedir,
                file_dir,
                root,
                file_context,
                &mut worklist,
            );
        }
    }

    cfg_test_files.retain(|file| !production_files.contains(file));
    cfg_test_files
}

/// Collect the set of `.rs` file paths that are reachable through `pub mod`
/// declarations (bare `pub` only) starting from the crate roots (`lib.rs` /
/// `main.rs`) in `root`.
///
/// Only bare `pub` visibility propagates rustdoc reachability; `pub(crate)`,
/// `pub(super)`, and private module declarations are not followed. The crate
/// roots themselves are always included in the returned set.
///
/// The returned set is transitively closed: a pub-reachable file is itself
/// scanned for further `pub mod` declarations so that their children are added.
/// `#[path = "..."]` attributes are resolved relative to the containing file's
/// directory and guarded by scan-root containment and symlink-ancestor checks.
pub(crate) fn collect_pub_reachable_files(root: &Path) -> HashSet<PathBuf> {
    let mut rs_files = Vec::new();
    collect_rs_files_from_dir(root, &mut rs_files);

    // Seed: crate roots are always pub-reachable.
    let mut worklist: Vec<PathBuf> =
        rs_files.iter().filter(|f| is_scan_crate_root(root, f)).cloned().collect();
    let mut pub_reachable: HashSet<PathBuf> = worklist.iter().cloned().collect();
    let mut visited: HashSet<PathBuf> = HashSet::new();

    while let Some(file) = worklist.pop() {
        if !visited.insert(file.clone()) {
            continue;
        }
        let Some(ast) = parse_real_rs_file(&file) else {
            continue;
        };
        let Some(basedir) = module_basedir(&file) else {
            continue;
        };
        let Some(file_dir) = file.parent() else {
            continue;
        };
        collect_pub_reachable_refs(
            &ast.items,
            &basedir,
            file_dir,
            root,
            &mut pub_reachable,
            &mut worklist,
        );
    }

    pub_reachable
}

/// Recursive helper for [`collect_pub_reachable_files`]: walks `items` and
/// adds file paths reachable through `pub mod` declarations to `pub_reachable`
/// and `worklist`.
fn collect_pub_reachable_refs(
    items: &[syn::Item],
    basedir: &Path,
    file_dir: &Path,
    scan_root: &Path,
    pub_reachable: &mut HashSet<PathBuf>,
    worklist: &mut Vec<PathBuf>,
) {
    for item in items {
        let syn::Item::Mod(mod_item) = item else {
            continue;
        };
        // cfg-test mods are test-scoped and never pub-reachable for doc purposes.
        if has_cfg_test_attr(&mod_item.attrs) {
            continue;
        }
        // Only bare `pub` propagates rustdoc reachability.
        if !matches!(mod_item.vis, syn::Visibility::Public(_)) {
            continue;
        }
        match &mod_item.content {
            None => {
                // File-backed pub mod: resolve the child file path.
                if let Some(path_val) = extract_path_attr_value(&mod_item.attrs) {
                    let resolved = lexical_normalize(&file_dir.join(&path_val));
                    if is_safe_module_file(&resolved, scan_root)
                        && pub_reachable.insert(resolved.clone())
                    {
                        worklist.push(resolved);
                    }
                } else {
                    for candidate in module_candidate_paths(basedir, mod_item) {
                        if is_safe_module_file(&candidate, scan_root)
                            && pub_reachable.insert(candidate.clone())
                        {
                            worklist.push(candidate);
                        }
                    }
                }
            }
            Some((_, inner_items)) => {
                // Inline pub mod: recurse into its items using the stacked basedir.
                let inner_basedir = basedir.join(mod_item.ident.to_string());
                collect_pub_reachable_refs(
                    inner_items,
                    &inner_basedir,
                    &inner_basedir,
                    scan_root,
                    pub_reachable,
                    worklist,
                );
            }
        }
    }
}

/// Recursively scan all `.rs` files under `root`, calling `on_file` for each
/// parseable, non-test-only file. Returns a [`VerifyOutcome`] aggregating all
/// findings returned by `on_file` across every file.
///
/// Uses a 2-pass approach to correctly handle file-backed test modules:
/// - **Pass 1**: walks the tree and collects all `.rs` files that are referenced
///   by `#[cfg(test)] mod X;` declarations (and their transitive children).
/// - **Pass 2**: walks the tree again invoking `on_file`, skipping files in the
///   cfg-test set collected in Pass 1.
///
/// Additionally, files whose top-level inner attribute list includes
/// `#![cfg(test)]` are skipped in their entirety. Item-level test exclusion
/// (e.g. skipping items inside `#[cfg(test)]` blocks or carrying `#[test]`) is
/// the caller's responsibility.
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
    // Pass 1: collect file paths declared behind #[cfg(test)] mod X;.
    let cfg_test_files = collect_cfg_test_files(root);
    // Pass 2: walk and invoke on_file, skipping cfg-test files.
    visit_rs_files(root, &cfg_test_files, &mut on_file, &mut findings);
    VerifyOutcome::from_findings(findings)
}

/// Internal recursive walker used by [`scan_rs_files`] (Pass 2).
fn visit_rs_files(
    dir: &Path,
    cfg_test_files: &HashSet<PathBuf>,
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
            visit_rs_files(&path, cfg_test_files, on_file, findings);
        } else if metadata.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            // Skip files declared behind #[cfg(test)] mod X; in a parent module.
            if cfg_test_files.contains(&path) {
                continue;
            }
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

    #[cfg(unix)]
    #[test]
    fn test_scan_rs_files_symlinked_cfg_test_mod_does_not_hide_child_file() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("outside_tests.rs");
        let link = tmp.path().join("tests.rs");
        let child_dir = tmp.path().join("tests");
        let child = child_dir.join("inner.rs");
        std::fs::write(tmp.path().join("lib.rs"), "#[cfg(test)] mod tests;\n").unwrap();
        std::fs::write(&target, "mod inner;\n").unwrap();
        std::fs::create_dir_all(&child_dir).unwrap();
        std::fs::write(&child, "pub fn child() {}\n").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let outcome = scan_rs_files(tmp.path(), |path, _| {
            if path.file_name().is_some_and(|name| name == "inner.rs") {
                vec![domain::verify::VerifyFinding::error("visited inner.rs")]
            } else {
                Vec::new()
            }
        });

        let messages: Vec<_> = outcome.findings().iter().map(ToString::to_string).collect();
        assert!(
            messages.iter().any(|msg| msg.contains("refusing to follow symlink")),
            "expected symlink rejection: {messages:?}"
        );
        assert!(
            messages.iter().any(|msg| msg.contains("visited inner.rs")),
            "symlinked cfg-test mod must not mark child files as test-only: {messages:?}"
        );
    }

    #[test]
    fn test_scan_rs_files_cfg_test_mod_in_one_root_does_not_hide_production_mod() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("lib.rs"), "mod api;\n").unwrap();
        std::fs::write(tmp.path().join("main.rs"), "#[cfg(test)] mod api;\n").unwrap();
        std::fs::write(tmp.path().join("api.rs"), "pub fn api() {}\n").unwrap();

        let outcome = scan_rs_files(tmp.path(), |path, _| {
            if path.file_name().is_some_and(|name| name == "api.rs") {
                vec![domain::verify::VerifyFinding::error("visited api.rs")]
            } else {
                Vec::new()
            }
        });

        let messages: Vec<_> = outcome.findings().iter().map(ToString::to_string).collect();
        assert!(
            messages.iter().any(|msg| msg.contains("visited api.rs")),
            "cfg-test mod in main.rs must not hide lib.rs production module: {messages:?}"
        );
    }

    #[test]
    fn test_scan_rs_files_cfg_test_mod_does_not_hide_crate_root() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("lib.rs"), "pub fn lib_root() {}\n").unwrap();
        std::fs::write(tmp.path().join("main.rs"), "#[cfg(test)] mod lib;\n").unwrap();

        let outcome = scan_rs_files(tmp.path(), |path, _| {
            if path.file_name().is_some_and(|name| name == "lib.rs") {
                vec![domain::verify::VerifyFinding::error("visited lib.rs")]
            } else {
                Vec::new()
            }
        });

        let messages: Vec<_> = outcome.findings().iter().map(ToString::to_string).collect();
        assert!(
            messages.iter().any(|msg| msg.contains("visited lib.rs")),
            "cfg-test mod in main.rs must not hide scan-root lib.rs: {messages:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_collect_pub_reachable_files_path_mod_rejects_symlink_parent() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_file = outside.path().join("outside.rs");
        let link_dir = tmp.path().join("link-dir");
        std::fs::write(
            tmp.path().join("lib.rs"),
            "#[path = \"link-dir/outside.rs\"] pub mod outside;\n",
        )
        .unwrap();
        std::fs::write(&outside_file, "pub fn exposed() {}\n").unwrap();
        std::os::unix::fs::symlink(outside.path(), &link_dir).unwrap();

        let pub_reachable = collect_pub_reachable_files(tmp.path());

        assert!(
            !pub_reachable.contains(&lexical_normalize(&link_dir.join("outside.rs"))),
            "#[path] resolution must not follow a symlinked parent outside scan root"
        );
    }
}
