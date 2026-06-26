//! Reusable syn-based AST scanner for `sotp verify` gates.
//!
//! Provides [`scan_workspace_rust_sources`] which discovers `.rs` files from
//! `architecture-rules.json` `layers[]`, parses each file with
//! [`syn::parse_file`], prunes `#[cfg(test)]` / `#[test]` items, and invokes a
//! caller-supplied callback for every remaining attribute-bearing AST node.
//!
//! Later syn-based lint gates (e.g. `Result<_, String>` ban) can reuse this
//! scanner by supplying a different detection callback.

use std::path::{Path, PathBuf};

use domain::verify::{VerifyFinding, VerifyOutcome};

use crate::track::symlink_guard::reject_symlinks_below;

use super::path_safety::lexical_normalize;
use super::syn_helpers::has_cfg_test_attr;
use super::trusted_root::absolutize;

// ─────────────────────────────────────────────────────────────────────────────
// Public context type
// ─────────────────────────────────────────────────────────────────────────────

/// Context passed from the scanner to a detection callback for each
/// attribute-bearing AST node.
///
/// The callback receives one `SynScanContext` per attribute-bearing node and
/// returns any [`VerifyFinding`] instances for violations it detects.
pub(crate) struct SynScanContext {
    /// Root-relative path to the source file containing the node.
    pub(crate) relative_path: PathBuf,
    /// 1-based source line of the first attribute in `attrs` (0 when span info
    /// is unavailable, e.g. when parsing from a string without span data).
    pub(crate) line: usize,
    /// Human-readable label describing the kind of AST node:
    /// `"file"`, `"item"`, `"field"`, `"variant"`, `"impl_item"`,
    /// `"trait_item"`, or `"foreign_item"`.
    pub(crate) node_kind: String,
    /// All outer (and, for inner-attribute style nodes, inner) attributes
    /// attached to the node.  Never empty: the callback is not invoked for
    /// nodes with no attributes.
    pub(crate) attrs: Vec<syn::Attribute>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Public scanner entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Reusable syn AST scanner for verify gates.
///
/// Loads `architecture-rules.json` `layers[]` from `root`, discovers `.rs`
/// source files under each layer's `{path}/src` directory, parses each file
/// with [`syn::parse_file`], prunes `#[cfg(test)]` and `#[test]` items, and
/// invokes `inspect` for every remaining attribute-bearing AST node (file-level
/// inner attributes, items, impl/trait associated items, struct fields, enum
/// variants, foreign items).
///
/// The `inspect` callback receives a [`SynScanContext`] and returns any
/// [`VerifyFinding`] instances for violations it detects.  All findings are
/// merged and returned as the final [`VerifyOutcome`].
///
/// Detection logic is entirely in the caller-supplied `inspect` closure, which
/// makes this scanner reusable for multiple syn-based lint gates.
pub(crate) fn scan_workspace_rust_sources(
    root: &Path,
    mut inspect: impl FnMut(SynScanContext) -> Vec<VerifyFinding>,
) -> VerifyOutcome {
    let rules = match crate::arch::load_rules(root) {
        Ok(r) => r,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "syn-scan: cannot load architecture-rules.json: {e}"
            ))]);
        }
    };

    let trusted_root = lexical_normalize(&absolutize(root));
    let mut findings = Vec::new();

    for layer in rules.layers() {
        let src_dir = match resolve_layer_src_dir(&trusted_root, &layer.crate_name, &layer.path) {
            Ok(path) => path,
            Err(finding) => {
                findings.push(finding);
                continue;
            }
        };
        scan_rs_files_in_dir(&trusted_root, &src_dir, &mut findings, &mut inspect);
    }

    VerifyOutcome::from_findings(findings)
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal file discovery helpers
// ─────────────────────────────────────────────────────────────────────────────

fn resolve_layer_src_dir(
    trusted_root: &Path,
    crate_name: &str,
    layer_path: &str,
) -> Result<PathBuf, VerifyFinding> {
    let layer_path = Path::new(layer_path);
    let absolute_layer_path = if layer_path.is_absolute() {
        layer_path.to_path_buf()
    } else {
        trusted_root.join(layer_path)
    };
    let normalized_layer_path = lexical_normalize(&absolute_layer_path);

    if !normalized_layer_path.starts_with(trusted_root) {
        return Err(VerifyFinding::error(format!(
            "syn-scan: layer '{crate_name}' path '{}' resolves outside trusted root '{}'",
            layer_path.display(),
            trusted_root.display()
        )));
    }

    Ok(normalized_layer_path.join("src"))
}

fn guarded_metadata(
    root: &Path,
    path: &Path,
    findings: &mut Vec<VerifyFinding>,
) -> Option<std::fs::Metadata> {
    match reject_symlinks_below(path, root) {
        Ok(true) => {}
        Ok(false) => return None,
        Err(e) => {
            let rel = path.strip_prefix(root).unwrap_or(path);
            findings.push(VerifyFinding::error(format!(
                "{}: symlink guard rejected path: {e}",
                rel.display()
            )));
            return None;
        }
    }

    match path.symlink_metadata() {
        Ok(meta) => Some(meta),
        Err(e) => {
            let rel = path.strip_prefix(root).unwrap_or(path);
            findings
                .push(VerifyFinding::error(format!("{}: cannot stat path: {e}", rel.display())));
            None
        }
    }
}

fn scan_rs_files_in_dir(
    root: &Path,
    dir: &Path,
    findings: &mut Vec<VerifyFinding>,
    inspect: &mut impl FnMut(SynScanContext) -> Vec<VerifyFinding>,
) {
    let Some(meta) = guarded_metadata(root, dir, findings) else {
        return;
    };
    if !meta.is_dir() {
        // Layer may not have a src/ directory (e.g. doc-only or binary-only).
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            let rel = dir.strip_prefix(root).unwrap_or(dir);
            findings.push(VerifyFinding::error(format!(
                "{}: cannot read directory: {e}",
                rel.display()
            )));
            return;
        }
    };

    // Collect and sort for deterministic ordering.
    let mut paths = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => paths.push(entry.path()),
            Err(e) => {
                let rel = dir.strip_prefix(root).unwrap_or(dir);
                findings.push(VerifyFinding::error(format!(
                    "{}: cannot read directory entry: {e}",
                    rel.display()
                )));
            }
        }
    }
    paths.sort();

    for path in paths {
        let Some(meta) = guarded_metadata(root, &path, findings) else {
            continue;
        };
        if meta.is_dir() {
            scan_rs_files_in_dir(root, &path, findings, inspect);
        } else if meta.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            scan_single_file(root, &path, findings, inspect);
        }
    }
}

fn scan_single_file(
    root: &Path,
    path: &Path,
    findings: &mut Vec<VerifyFinding>,
    inspect: &mut impl FnMut(SynScanContext) -> Vec<VerifyFinding>,
) {
    let Some(meta) = guarded_metadata(root, path, findings) else {
        return;
    };
    if !meta.is_file() {
        return;
    }

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            let rel = path.strip_prefix(root).unwrap_or(path);
            findings
                .push(VerifyFinding::error(format!("{}: cannot read file: {e}", rel.display())));
            return;
        }
    };

    let file = match syn::parse_file(&content) {
        Ok(f) => f,
        Err(e) => {
            let rel = path.strip_prefix(root).unwrap_or(path);
            findings
                .push(VerifyFinding::error(format!("{}: cannot parse file: {e}", rel.display())));
            return;
        }
    };

    let rel_path = path.strip_prefix(root).unwrap_or(path).to_path_buf();

    // Skip files whose crate-level attrs include `#![cfg(test)]`.
    if has_cfg_test_attr(&file.attrs) {
        return;
    }
    if is_file_backed_test_module(root, path) {
        return;
    }

    // Emit findings for crate-level / file-level inner attributes.
    if !file.attrs.is_empty() {
        let line = attr_start_line(file.attrs.first());
        let ctx = SynScanContext {
            relative_path: rel_path.clone(),
            line,
            node_kind: "file".to_owned(),
            attrs: file.attrs.clone(),
        };
        findings.extend(inspect(ctx));
    }

    // Walk all top-level items.
    let mut visitor = NodeVisitor { rel_path: &rel_path, findings, inspect };
    for item in &file.items {
        visitor.visit_item(item);
    }
}

/// Returns the 1-based source line of the first attribute, or 0 when span info
/// is unavailable.
fn attr_start_line(attr: Option<&syn::Attribute>) -> usize {
    attr.map(|a| a.pound_token.spans[0].start().line).unwrap_or(0)
}

/// Returns `true` only when `path` is referenced exclusively by `#[cfg(test)]`
/// module declarations (no production `mod` declaration also points to it).
fn is_file_backed_test_module(root: &Path, path: &Path) -> bool {
    let (mut cfg_test_ref, mut prod_ref) = (false, false);
    for (candidate, module_path) in file_backed_module_source_probes(root, path) {
        let (ct, prod) = classify_file_module_references(root, &candidate, &module_path);
        cfg_test_ref |= ct;
        prod_ref |= prod;
    }
    cfg_test_ref && !prod_ref
}

fn file_backed_module_source_probes(root: &Path, path: &Path) -> Vec<(PathBuf, Vec<String>)> {
    let mut probes = Vec::new();
    let Some(mut base_dir) = path.parent().map(Path::to_path_buf) else {
        return probes;
    };

    loop {
        for source in parent_module_file_candidates(root, &base_dir) {
            if source.as_path() == path {
                continue;
            }
            if let Some(module_path) = module_path_for_file_from_base(&base_dir, path) {
                probes.push((source, module_path));
            }
        }

        if base_dir == root {
            break;
        }
        let Some(parent) = base_dir.parent() else {
            break;
        };
        base_dir = parent.to_path_buf();
    }

    probes
}

fn parent_module_file_candidates(root: &Path, parent_dir: &Path) -> Vec<PathBuf> {
    let mut candidates =
        vec![parent_dir.join("mod.rs"), parent_dir.join("lib.rs"), parent_dir.join("main.rs")];

    if let Some(grandparent) = parent_dir.parent() {
        if let Some(dir_name) = parent_dir.file_name() {
            candidates.push(grandparent.join(format!("{}.rs", dir_name.to_string_lossy())));
        }
    }

    candidates.into_iter().filter(|candidate| candidate.strip_prefix(root).is_ok()).collect()
}

fn module_path_for_file_from_base(base_dir: &Path, path: &Path) -> Option<Vec<String>> {
    let file_name = path.file_name()?.to_string_lossy();
    let module_path = if file_name == "mod.rs" {
        let module_dir = path.parent()?;
        normal_component_strings(module_dir.strip_prefix(base_dir).ok()?)?
    } else {
        let parent_dir = path.parent()?;
        let mut components = normal_component_strings(parent_dir.strip_prefix(base_dir).ok()?)?;
        components.push(path.file_stem()?.to_string_lossy().into_owned());
        components
    };

    if module_path.is_empty() { None } else { Some(module_path) }
}

fn normal_component_strings(path: &Path) -> Option<Vec<String>> {
    let mut names = Vec::new();
    for component in path.components() {
        let std::path::Component::Normal(name) = component else {
            return None;
        };
        names.push(name.to_string_lossy().into_owned());
    }
    Some(names)
}

/// Reads `path` once and returns `(cfg_test_referenced, production_referenced)`:
/// whether a `#[cfg(test)]` or non-`cfg(test)` `mod` in `path` resolves to `module_path`.
fn classify_file_module_references(
    root: &Path,
    path: &Path,
    module_path: &[String],
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

    let cfg_test = items_declare_cfg_test_module_path(&file.items, module_path, false);
    let production = items_declare_production_module_path(&file.items, module_path, false);
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

/// Returns the string value of the first `#[path = "..."]` attribute in `attrs`, if any.
fn extract_path_attr(attrs: &[syn::Attribute]) -> Option<String> {
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

// ─────────────────────────────────────────────────────────────────────────────
// Recursive AST visitor
// ─────────────────────────────────────────────────────────────────────────────

struct NodeVisitor<'a, F> {
    rel_path: &'a Path,
    findings: &'a mut Vec<VerifyFinding>,
    inspect: &'a mut F,
}

impl<'a, F> NodeVisitor<'a, F>
where
    F: FnMut(SynScanContext) -> Vec<VerifyFinding>,
{
    /// Invoke the callback with a context for `attrs`, if non-empty.
    fn emit(&mut self, node_kind: &str, attrs: &[syn::Attribute]) {
        if attrs.is_empty() {
            return;
        }
        let line = attr_start_line(attrs.first());
        let ctx = SynScanContext {
            relative_path: self.rel_path.to_path_buf(),
            line,
            node_kind: node_kind.to_owned(),
            attrs: attrs.to_vec(),
        };
        self.findings.extend((self.inspect)(ctx));
    }

    /// Returns `true` when the attributes mark the item as test-only.
    ///
    /// Exact `#[cfg(test)]` and `#[test]` attributes cause an item to be
    /// excluded from scanning (test code is not in scope for doc-hidden checks).
    fn is_test_item(attrs: &[syn::Attribute]) -> bool {
        has_cfg_test_attr(attrs) || attrs.iter().any(|a| a.path().is_ident("test"))
    }

    fn visit_item(&mut self, item: &syn::Item) {
        let attrs = item_attrs(item);

        // Skip test-gated items entirely — do not recurse into their children.
        if Self::is_test_item(attrs) {
            return;
        }

        self.emit("item", attrs);

        // Recurse into nested attribute-bearing nodes.
        match item {
            syn::Item::Mod(m) => {
                if let Some((_, items)) = &m.content {
                    for inner in items {
                        self.visit_item(inner);
                    }
                }
            }
            syn::Item::Impl(impl_block) => {
                for assoc_item in &impl_block.items {
                    self.visit_impl_item(assoc_item);
                }
            }
            syn::Item::Trait(trait_block) => {
                for assoc_item in &trait_block.items {
                    self.visit_trait_item(assoc_item);
                }
            }
            syn::Item::Struct(s) => {
                for field in &s.fields {
                    self.visit_field(field);
                }
            }
            syn::Item::Union(u) => {
                for field in &u.fields.named {
                    self.visit_field(field);
                }
            }
            syn::Item::Enum(e) => {
                for variant in &e.variants {
                    self.visit_variant(variant);
                }
            }
            syn::Item::ForeignMod(fm) => {
                for foreign_item in &fm.items {
                    self.visit_foreign_item(foreign_item);
                }
            }
            _ => {}
        }
    }

    fn visit_impl_item(&mut self, item: &syn::ImplItem) {
        let attrs = impl_item_attrs(item);
        if Self::is_test_item(attrs) {
            return;
        }
        self.emit("impl_item", attrs);
    }

    fn visit_trait_item(&mut self, item: &syn::TraitItem) {
        let attrs = trait_item_attrs(item);
        if Self::is_test_item(attrs) {
            return;
        }
        self.emit("trait_item", attrs);
    }

    fn visit_field(&mut self, field: &syn::Field) {
        if Self::is_test_item(&field.attrs) {
            return;
        }
        self.emit("field", &field.attrs);
    }

    fn visit_variant(&mut self, variant: &syn::Variant) {
        if Self::is_test_item(&variant.attrs) {
            return;
        }
        self.emit("variant", &variant.attrs);
        // Also visit fields inside the variant (for struct-like variants).
        for field in &variant.fields {
            self.visit_field(field);
        }
    }

    fn visit_foreign_item(&mut self, item: &syn::ForeignItem) {
        let attrs = foreign_item_attrs(item);
        if Self::is_test_item(attrs) {
            return;
        }
        self.emit("foreign_item", attrs);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Attribute extraction helpers
// ─────────────────────────────────────────────────────────────────────────────

fn item_attrs(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(i) => &i.attrs,
        syn::Item::Enum(i) => &i.attrs,
        syn::Item::ExternCrate(i) => &i.attrs,
        syn::Item::Fn(i) => &i.attrs,
        syn::Item::ForeignMod(i) => &i.attrs,
        syn::Item::Impl(i) => &i.attrs,
        syn::Item::Macro(i) => &i.attrs,
        syn::Item::Mod(i) => &i.attrs,
        syn::Item::Static(i) => &i.attrs,
        syn::Item::Struct(i) => &i.attrs,
        syn::Item::Trait(i) => &i.attrs,
        syn::Item::TraitAlias(i) => &i.attrs,
        syn::Item::Type(i) => &i.attrs,
        syn::Item::Union(i) => &i.attrs,
        syn::Item::Use(i) => &i.attrs,
        _ => &[],
    }
}

fn impl_item_attrs(item: &syn::ImplItem) -> &[syn::Attribute] {
    match item {
        syn::ImplItem::Const(i) => &i.attrs,
        syn::ImplItem::Fn(i) => &i.attrs,
        syn::ImplItem::Type(i) => &i.attrs,
        syn::ImplItem::Macro(i) => &i.attrs,
        _ => &[],
    }
}

fn trait_item_attrs(item: &syn::TraitItem) -> &[syn::Attribute] {
    match item {
        syn::TraitItem::Const(i) => &i.attrs,
        syn::TraitItem::Fn(i) => &i.attrs,
        syn::TraitItem::Type(i) => &i.attrs,
        syn::TraitItem::Macro(i) => &i.attrs,
        _ => &[],
    }
}

fn foreign_item_attrs(item: &syn::ForeignItem) -> &[syn::Attribute] {
    match item {
        syn::ForeignItem::Fn(i) => &i.attrs,
        syn::ForeignItem::Static(i) => &i.attrs,
        syn::ForeignItem::Type(i) => &i.attrs,
        syn::ForeignItem::Macro(i) => &i.attrs,
        _ => &[],
    }
}
