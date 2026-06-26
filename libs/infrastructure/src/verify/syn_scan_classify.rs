//! Module-reference classifier helpers used by [`super::syn_scan`].
//!
//! Split from `syn_scan.rs` to keep that file within the module-size hard limit.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::track::symlink_guard::reject_symlinks_below;

use super::syn_helpers::{extract_path_attr, has_cfg_test_attr};

/// Memoisation cache for [`classify_file_module_references`].
///
/// Keyed on `(file_path, module_path)` — identical inputs always produce the
/// same output, so results are safe to cache for the entire workspace scan.
/// Sharing one cache across all calls in `scan_workspace_rust_sources` prevents
/// the cumulative `proc-macro2` byte-offset from overflowing `u32::MAX`.
pub(crate) type ClassifyCache = HashMap<(PathBuf, Vec<String>), (bool, bool)>;

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
) -> (bool, bool) {
    let key = (path.to_path_buf(), module_path.to_vec());
    if let Some(&cached) = cache.get(&key) {
        return cached;
    }
    let result = classify_uncached(root, path, module_path);
    cache.insert(key, result);
    result
}

fn classify_uncached(root: &Path, path: &Path, module_path: &[String]) -> (bool, bool) {
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
    let cfg_test = items_declare_cfg_test_module_path(&file.items, module_path, inherited_cfg_test);
    let production =
        items_declare_production_module_path(&file.items, module_path, inherited_cfg_test);
    (cfg_test, production)
}

fn items_declare_cfg_test_module_path(
    items: &[syn::Item],
    module_path: &[String],
    inherited_cfg_test: bool,
) -> bool {
    let Some((head, tail)) = module_path.split_first() else {
        return false;
    };

    items.iter().any(|item| {
        let syn::Item::Mod(module) = item else {
            return false;
        };
        let cfg_test = inherited_cfg_test || has_cfg_test_attr(&module.attrs);

        // `#[path = "..."]` overrides the ident.
        if let Some(path_value) = extract_path_attr(&module.attrs) {
            return module.content.is_none()
                && cfg_test
                && path_attr_matches_module_path(&path_value, module_path);
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
            return items_declare_cfg_test_module_path(nested_items, tail, cfg_test);
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
                && path_attr_matches_module_path(&path_value, module_path);
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
            return items_declare_production_module_path(nested_items, tail, cfg_test);
        }
        false
    })
}

/// Returns `true` when `path_value` (a `#[path = "..."]` string), treated as a
/// relative path, matches the entire `module_path` component sequence.  The last
/// component is compared by file stem; `mod.rs` pops the stem and matches one level up.
fn path_attr_matches_module_path(path_value: &str, module_path: &[String]) -> bool {
    let path = Path::new(path_value);
    let mut components = Vec::new();
    for component in path.components() {
        let std::path::Component::Normal(name) = component else {
            return false;
        };
        components.push(name.to_string_lossy().into_owned());
    }

    if components.is_empty() {
        return false;
    }

    if let Some(last_component) = components.last_mut() {
        let stem = Path::new(last_component.as_str())
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| last_component.clone());
        if stem == "mod" {
            components.pop();
        } else {
            *last_component = stem;
        }
    }

    components.len() == module_path.len()
        && components
            .iter()
            .zip(module_path.iter())
            .all(|(component, expected)| component == expected)
}
