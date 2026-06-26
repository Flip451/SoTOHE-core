//! Shared syn-based AST helpers used across verify submodules.

use std::path::{Path, PathBuf};

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

/// Returns the string value of the first `#[path = "..."]` attribute in `attrs`, if any.
pub(crate) fn extract_path_attr(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if !attr.path().is_ident("path") {
            continue;
        }
        if let syn::Meta::NameValue(nv) = &attr.meta {
            if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &nv.value {
                return Some(s.value());
            }
        }
    }
    None
}

/// Returns all `.rs` sibling files of `path` inside `root`, excluding `path` itself.
///
/// Used by `file_backed_module_source_probes` in [`super::syn_scan`] to discover
/// sibling module source files that may contain
/// `#[cfg(test)] #[path = "foo_tests.rs"] mod tests;` declarations pointing to
/// `path` (e.g. `foo.rs` declaring that its test module lives in `foo_tests.rs`).
pub(crate) fn sibling_rs_files(root: &Path, path: &Path) -> Vec<PathBuf> {
    let Some(parent) = path.parent() else {
        return Vec::new();
    };
    if parent.strip_prefix(root).is_err() {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(parent) else {
        return Vec::new();
    };
    let mut siblings: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p != path && p.extension().is_some_and(|ext| ext == "rs"))
        .filter(|p| p.strip_prefix(root).is_ok())
        .collect();
    siblings.sort();
    siblings
}

/// Returns all `.rs` files (non-recursive) in `dir` that are within `root`.
///
/// Used by `file_backed_module_source_probes` in [`super::syn_scan`] to probe
/// ancestor directories for any `.rs` file that may contain a
/// `#[cfg(test)] #[path = "subdir/..."] mod tests;` declaration referencing a
/// target file located in a subdirectory of `dir`.
pub(crate) fn rs_files_in_dir(root: &Path, dir: &Path) -> Vec<PathBuf> {
    if dir.strip_prefix(root).is_err() {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "rs"))
        .filter(|p| p.strip_prefix(root).is_ok())
        .collect();
    files.sort();
    files
}
