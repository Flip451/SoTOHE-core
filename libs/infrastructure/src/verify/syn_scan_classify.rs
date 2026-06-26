//! Module-reference classifier helpers used by [`super::syn_scan`].
//!
//! Split from `syn_scan.rs` to keep that file within the module-size hard limit.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::track::symlink_guard::reject_symlinks_below;

use super::syn_helpers::{extract_path_attr, has_cfg_test_attr};

/// Memoisation cache for [`classify_file_module_references`].
///
/// Keyed on `(file_path, module_path, target_file)` — identical inputs always
/// produce the same output, so results are safe to cache for the entire workspace
/// scan.  `target_file` is included because two targets with the same
/// `module_path` (e.g. `src/foo.rs` and `src/foo/mod.rs`, both `["foo"]`) must
/// produce independent results.  Sharing one cache across all calls in
/// `scan_workspace_rust_sources` prevents the cumulative `proc-macro2`
/// byte-offset from overflowing `u32::MAX`.
pub(crate) type ClassifyCache = HashMap<(PathBuf, Vec<String>, PathBuf), (bool, bool)>;

/// Memoisation cache for [`classify_sibling_probe`].
///
/// Separate from [`ClassifyCache`] because sibling probes use `#[path]`-only
/// matching, producing different results than full ident + path-attr matching.
/// Keyed on `(file_path, module_path, target_file)` for the same reason as
/// [`ClassifyCache`].  Sharing one instance across the workspace scan prevents
/// redundant re-parses (and keeps cumulative `proc-macro2` byte-offsets within
/// `u32::MAX`).
pub(crate) type SiblingClassifyCache = HashMap<(PathBuf, Vec<String>, PathBuf), (bool, bool)>;

/// Returns `(cfg_test_referenced, production_referenced)`: whether a
/// `#[cfg(test)]` or non-`cfg(test)` `mod` declaration in `path` resolves to
/// `module_path`.
///
/// Results are memoised in `cache` so each `(path, module_path)` pair triggers
/// `syn::parse_file` at most once per workspace scan, bounding the cumulative
/// parsed-byte volume below the `proc-macro2` `u32::MAX` span-offset limit.
pub(crate) fn classify_file_module_references(
    root: &Path,
    path: &Path,
    module_path: &[String],
    cache: &mut ClassifyCache,
    target_file: &Path,
) -> (bool, bool) {
    let key = (path.to_path_buf(), module_path.to_vec(), target_file.to_path_buf());
    if let Some(&cached) = cache.get(&key) {
        return cached;
    }
    let result = classify_uncached(root, path, module_path, target_file);
    cache.insert(key, result);
    result
}

fn classify_uncached(
    root: &Path,
    path: &Path,
    module_path: &[String],
    target_file: &Path,
) -> (bool, bool) {
    let Ok(true) = reject_symlinks_below(path, root) else {
        return (false, false);
    };
    let Ok(meta) = path.symlink_metadata() else {
        return (false, false);
    };
    if !meta.is_file() {
        return (false, false);
    }

    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return (false, false),
    };

    let file = match syn::parse_file(&content) {
        Ok(file) => file,
        Err(_) => return (false, false),
    };

    let inherited_cfg_test = has_cfg_test_attr(&file.attrs);
    let cfg_test = items_declare_cfg_test_module_path(
        &file.items,
        module_path,
        inherited_cfg_test,
        path,
        target_file,
    );
    let production = items_declare_production_module_path(
        &file.items,
        module_path,
        inherited_cfg_test,
        path,
        target_file,
    );
    (cfg_test, production)
}

fn items_declare_cfg_test_module_path(
    items: &[syn::Item],
    module_path: &[String],
    inherited_cfg_test: bool,
    containing_file: &Path,
    target_file: &Path,
) -> bool {
    let Some((head, tail)) = module_path.split_first() else {
        return false;
    };

    items.iter().any(|item| {
        let syn::Item::Mod(module) = item else {
            return false;
        };
        let cfg_test = inherited_cfg_test || has_cfg_test_attr(&module.attrs);

        // `#[path = "..."]` overrides the ident: compare by resolved file path, not
        // by module-path components.  Module-path comparison is ambiguous because
        // `src/foo.rs` and `src/foo/mod.rs` both produce the component sequence
        // `["foo"]`, whereas the `#[path]` value literally names one of the two.
        if let Some(path_value) = extract_path_attr(&module.attrs) {
            return module.content.is_none()
                && cfg_test
                && path_attr_resolves_to_target(&path_value, containing_file, target_file);
        }

        if module.ident != head.as_str() {
            return false;
        }
        if tail.is_empty() {
            return module.content.is_none() && cfg_test;
        }
        if cfg_test && module.content.is_none() {
            return true;
        }
        if let Some((_, nested_items)) = &module.content {
            // Inline mods shift the base directory used by rustc to resolve
            // `#[path]` attributes declared inside them.  For example,
            // `mod tests { #[path = "helpers.rs"] mod helpers; }` in
            // `src/lib.rs` resolves `helpers.rs` to `src/tests/helpers.rs`,
            // not `src/helpers.rs`.  We represent this by updating the
            // virtual containing-file path so that its `parent()` equals the
            // inline module's resolution directory.
            let inline_basefile = containing_file
                .parent()
                .map(|d| d.join(module.ident.to_string()).join("_.rs"))
                .unwrap_or_else(|| PathBuf::from(module.ident.to_string()).join("_.rs"));
            return items_declare_cfg_test_module_path(
                nested_items,
                tail,
                cfg_test,
                &inline_basefile,
                target_file,
            );
        }
        false
    })
}

/// Returns `true` when a non-`cfg(test)` file-backed `mod` declaration in
/// `items` resolves to `module_path` (mirror of `items_declare_cfg_test_module_path`).
fn items_declare_production_module_path(
    items: &[syn::Item],
    module_path: &[String],
    inherited_cfg_test: bool,
    containing_file: &Path,
    target_file: &Path,
) -> bool {
    let Some((head, tail)) = module_path.split_first() else {
        return false;
    };

    items.iter().any(|item| {
        let syn::Item::Mod(module) = item else {
            return false;
        };
        let cfg_test = inherited_cfg_test || has_cfg_test_attr(&module.attrs);

        if let Some(path_value) = extract_path_attr(&module.attrs) {
            return module.content.is_none()
                && !cfg_test
                && path_attr_resolves_to_target(&path_value, containing_file, target_file);
        }

        if module.ident != head.as_str() {
            return false;
        }
        if tail.is_empty() {
            return module.content.is_none() && !cfg_test;
        }
        if cfg_test && module.content.is_none() {
            return false;
        }
        if let Some((_, nested_items)) = &module.content {
            // Mirror the basedir-update logic from `items_declare_cfg_test_module_path`:
            // inline mods shift the `#[path]` resolution directory.
            let inline_basefile = containing_file
                .parent()
                .map(|d| d.join(module.ident.to_string()).join("_.rs"))
                .unwrap_or_else(|| PathBuf::from(module.ident.to_string()).join("_.rs"));
            return items_declare_production_module_path(
                nested_items,
                tail,
                cfg_test,
                &inline_basefile,
                target_file,
            );
        }
        false
    })
}

/// Returns `true` when the `#[path = "path_value"]` attribute in `containing_file`
/// resolves to `target_file` via lexical path normalization.
///
/// `path_value` is joined to `containing_file`'s parent directory and then
/// lexically normalized (CurDir components stripped).  `ParentDir` (`..`) and
/// absolute paths are rejected immediately with `false`.  The result is compared
/// to `target_file` after the same normalization.
///
/// File-path comparison is used instead of module-path component comparison
/// because both `src/foo.rs` and `src/foo/mod.rs` yield the same module-path
/// component sequence `["foo"]`, making module-path comparison ambiguous when a
/// `#[path]` attribute explicitly targets one of the two files.
fn path_attr_resolves_to_target(
    path_value: &str,
    containing_file: &Path,
    target_file: &Path,
) -> bool {
    use std::path::Component;

    // Reject absolute or parent-traversing path values.
    for component in Path::new(path_value).components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            _ => return false, // RootDir / ParentDir / Prefix → reject
        }
    }

    let Some(containing_dir) = containing_file.parent() else {
        return false;
    };

    let resolved = lexical_normalize(&containing_dir.join(path_value));
    lexical_normalize(target_file) == resolved
}

/// Lexically normalise `path` by stripping `CurDir` (`.`) components.
///
/// Unlike `std::fs::canonicalize` this does not access the filesystem, making
/// it safe to call on hypothetical paths constructed by joining a `#[path]`
/// attribute value to a file's parent directory.
fn lexical_normalize(path: &Path) -> PathBuf {
    path.components().filter(|c| *c != std::path::Component::CurDir).collect()
}

/// Returns `(cfg_test_referenced, production_referenced)` for a same-directory
/// sibling probe.
///
/// Unlike [`classify_file_module_references`], only `#[path = "..."]`-based `mod`
/// declarations are considered.  A bare `mod foo;` in `src/a.rs` resolves to
/// `src/a/foo.rs` (not to sibling `src/foo.rs`), so ident-based matching is
/// intentionally skipped for sibling probes.
///
/// Results are memoised in `cache` to bound the cumulative `proc-macro2`
/// byte-offset within `u32::MAX` across a workspace-wide scan (the same sibling
/// file may be probed by many targets in the same directory).
pub(crate) fn classify_sibling_probe(
    root: &Path,
    path: &Path,
    module_path: &[String],
    cache: &mut SiblingClassifyCache,
    target_file: &Path,
) -> (bool, bool) {
    let key = (path.to_path_buf(), module_path.to_vec(), target_file.to_path_buf());
    if let Some(&cached) = cache.get(&key) {
        return cached;
    }
    let result = classify_sibling_uncached(root, path, target_file);
    cache.insert(key, result);
    result
}

fn classify_sibling_uncached(root: &Path, path: &Path, target_file: &Path) -> (bool, bool) {
    let Ok(true) = reject_symlinks_below(path, root) else {
        return (false, false);
    };
    let Ok(meta) = path.symlink_metadata() else {
        return (false, false);
    };
    if !meta.is_file() {
        return (false, false);
    }
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return (false, false),
    };
    let file = match syn::parse_file(&content) {
        Ok(file) => file,
        Err(_) => return (false, false),
    };
    let inherited_cfg_test = has_cfg_test_attr(&file.attrs);
    (
        items_declare_cfg_test_path_attr_only(&file.items, inherited_cfg_test, path, target_file),
        items_declare_production_path_attr_only(&file.items, inherited_cfg_test, path, target_file),
    )
}

fn items_declare_cfg_test_path_attr_only(
    items: &[syn::Item],
    inherited_cfg_test: bool,
    containing_file: &Path,
    target_file: &Path,
) -> bool {
    items.iter().any(|item| {
        let syn::Item::Mod(module) = item else {
            return false;
        };
        let cfg_test = inherited_cfg_test || has_cfg_test_attr(&module.attrs);
        if let Some(path_value) = extract_path_attr(&module.attrs) {
            return module.content.is_none()
                && cfg_test
                && path_attr_resolves_to_target(&path_value, containing_file, target_file);
        }
        false
    })
}

fn items_declare_production_path_attr_only(
    items: &[syn::Item],
    inherited_cfg_test: bool,
    containing_file: &Path,
    target_file: &Path,
) -> bool {
    items.iter().any(|item| {
        let syn::Item::Mod(module) = item else {
            return false;
        };
        let cfg_test = inherited_cfg_test || has_cfg_test_attr(&module.attrs);
        if let Some(path_value) = extract_path_attr(&module.attrs) {
            return module.content.is_none()
                && !cfg_test
                && path_attr_resolves_to_target(&path_value, containing_file, target_file);
        }
        false
    })
}
