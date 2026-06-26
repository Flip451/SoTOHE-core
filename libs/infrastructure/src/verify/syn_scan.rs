//! Reusable syn-based AST scanner for `sotp verify` gates.
//!
//! Provides [`scan_workspace_rust_sources`] which discovers `.rs` files from
//! `architecture-rules.json` `layers[]`, parses each file with
//! [`syn::parse_file`], prunes `#[cfg(test)]` / `#[test]` items, and invokes a
//! caller-supplied callback for every remaining attribute-bearing AST node.
//!
//! Later syn-based lint gates (e.g. `Result<_, String>` ban) can reuse this
//! scanner by supplying a different detection callback.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use domain::verify::{VerifyFinding, VerifyOutcome};

use crate::track::symlink_guard::reject_symlinks_below;

use super::path_safety::lexical_normalize;
use super::syn_helpers::{has_cfg_test_attr, rs_files_in_dir, sibling_rs_files};
use super::syn_scan_classify::{
    ClassifyCache, SiblingClassifyCache, classify_file_module_references, classify_sibling_probe,
};
use super::trusted_root::absolutize;

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
    let mut classify_cache = ClassifyCache::new();
    let mut sibling_cache = SiblingClassifyCache::new();

    for layer in rules.layers() {
        let src_dir = match resolve_layer_src_dir(&trusted_root, &layer.crate_name, &layer.path) {
            Ok(path) => path,
            Err(finding) => {
                findings.push(finding);
                continue;
            }
        };
        scan_rs_files_in_dir(
            &trusted_root,
            &src_dir,
            &mut findings,
            &mut inspect,
            &mut classify_cache,
            &mut sibling_cache,
        );
    }

    VerifyOutcome::from_findings(findings)
}

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
    classify_cache: &mut ClassifyCache,
    sibling_cache: &mut SiblingClassifyCache,
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
            scan_rs_files_in_dir(root, &path, findings, inspect, classify_cache, sibling_cache);
        } else if meta.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            scan_single_file(root, &path, findings, inspect, classify_cache, sibling_cache);
        }
    }
}

fn scan_single_file(
    root: &Path,
    path: &Path,
    findings: &mut Vec<VerifyFinding>,
    inspect: &mut impl FnMut(SynScanContext) -> Vec<VerifyFinding>,
    classify_cache: &mut ClassifyCache,
    sibling_cache: &mut SiblingClassifyCache,
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
    if is_file_backed_test_module(root, path, classify_cache, sibling_cache) {
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

/// Returns `true` when `path` is a crate root entry point (`src/lib.rs` or
/// `src/main.rs`).
///
/// Crate roots have no parent `mod` declaration in any production file, so the
/// dual-ref scan would produce `cfg_test_ref=true, prod_ref=false` if a sibling
/// file contains `#[cfg(test)] #[path = "lib.rs"] mod …;`.  By short-circuiting
/// on crate roots we prevent such sibling declarations from misclassifying the
/// root as test-only and allowing `#[doc(hidden)]` to bypass the gate.
fn is_crate_root_entry_point(path: &Path) -> bool {
    let Some(file_name) = path.file_name() else {
        return false;
    };
    let name = file_name.to_string_lossy();
    if name != "lib.rs" && name != "main.rs" {
        return false;
    }
    path.parent().is_some_and(|p| p.file_name().is_some_and(|n| n == "src"))
}

/// Returns `true` when `path` is exclusively referenced by `#[cfg(test)] mod`
/// declarations, propagating test context transitively from ancestor files.
fn is_file_backed_test_module(
    root: &Path,
    path: &Path,
    cache: &mut ClassifyCache,
    sibling_cache: &mut SiblingClassifyCache,
) -> bool {
    is_file_backed_test_module_inner(
        root,
        path,
        &mut HashSet::new(),
        &mut HashMap::new(),
        cache,
        sibling_cache,
    )
}

/// Inner recursive helper for [`is_file_backed_test_module`] with DFS cycle guard.
/// Removes the `path` entry from `visiting` on return to allow re-visits from other paths.
fn is_file_backed_test_module_inner(
    root: &Path,
    path: &Path,
    visiting: &mut HashSet<PathBuf>,
    memo: &mut HashMap<PathBuf, bool>,
    cache: &mut ClassifyCache,
    sibling_cache: &mut SiblingClassifyCache,
) -> bool {
    // Crate roots (`src/lib.rs`, `src/main.rs`) are always production code.
    // They have no parent `mod` declaration, so a sibling file that declares
    // `#[cfg(test)] #[path = "lib.rs"] mod …;` would otherwise produce
    // cfg_test_ref=true, prod_ref=false — misclassifying the root as test-only.
    if is_crate_root_entry_point(path) {
        return false;
    }

    if let Some(result) = memo.get(path) {
        return *result;
    }
    if !visiting.insert(path.to_path_buf()) {
        return false;
    }
    let (mut cfg_test_ref, mut prod_ref) = (false, false);
    for (candidate, module_path, is_sibling) in file_backed_module_source_probes(root, path) {
        let parent_is_test = is_existing_source_file(root, &candidate)
            && is_file_backed_test_module_inner(
                root,
                &candidate,
                visiting,
                memo,
                cache,
                sibling_cache,
            );
        // Sibling probes (same-directory files) must only match via `#[path]` attributes.
        // A bare `mod foo;` in `src/a.rs` resolves to `src/a/foo.rs`, not to sibling
        // `src/foo.rs`, so ident-based matching is skipped for sibling probes.
        let (ct, prod) = if is_sibling {
            classify_sibling_probe(root, &candidate, &module_path, sibling_cache, path)
        } else {
            classify_file_module_references(root, &candidate, &module_path, cache, path)
        };
        if parent_is_test {
            cfg_test_ref |= ct | prod;
        } else {
            cfg_test_ref |= ct;
            prod_ref |= prod;
        }
    }
    let result = cfg_test_ref && !prod_ref;
    visiting.remove(&path.to_path_buf());
    memo.insert(path.to_path_buf(), result);
    result
}

fn is_existing_source_file(root: &Path, path: &Path) -> bool {
    let Ok(true) = reject_symlinks_below(path, root) else {
        return false;
    };
    path.symlink_metadata().is_ok_and(|meta| meta.is_file())
}

// Each probe is `(candidate_file, module_path, is_sibling_probe)`.
// `is_sibling_probe = true` means the candidate is a same-directory sibling of
// `path`.  Sibling probes must only be classified via `#[path]` attributes because
// a bare `mod foo;` in a sibling file resolves to a subdirectory, not to `path`.
fn file_backed_module_source_probes(root: &Path, path: &Path) -> Vec<(PathBuf, Vec<String>, bool)> {
    let mut probes = Vec::new();
    let Some(mut base_dir) = path.parent().map(Path::to_path_buf) else {
        return probes;
    };

    // Probe same-directory .rs siblings — they may use `#[path]` to reference `path`.
    // Marked as sibling probes so that ident-based `mod` matching is skipped.
    if let Some(module_path) = module_path_for_file_from_base(&base_dir, path) {
        for sibling in sibling_rs_files(root, path) {
            probes.push((sibling, module_path.clone(), true));
        }
    }

    loop {
        for source in parent_module_file_candidates(root, &base_dir) {
            if source.as_path() == path {
                continue;
            }
            if let Some(module_path) = module_path_for_file_from_base(&base_dir, path) {
                probes.push((source, module_path, false));
            }
        }

        if base_dir == root {
            break;
        }
        let Some(parent) = base_dir.parent() else {
            break;
        };
        base_dir = parent.to_path_buf();

        // Ancestor sibling probe: add all .rs files in `base_dir` (now the
        // ancestor directory).  Any file there may use
        // `#[cfg(test)] #[path = "subdir/helpers.rs"] mod tests;`
        // to reference `path` via a subdirectory path attribute, a pattern
        // that the canonical-candidates list above does not cover.
        if let Some(module_path) = module_path_for_file_from_base(&base_dir, path) {
            for ancestor_file in rs_files_in_dir(root, &base_dir) {
                if ancestor_file.as_path() != path {
                    probes.push((ancestor_file, module_path.clone(), false));
                }
            }
        }
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
            syn::Item::Fn(item_fn) => {
                // Walk function bodies for local item declarations, including
                // items nested in expression blocks and control-flow bodies.
                self.visit_block_for_local_items(&item_fn.block);
            }
            syn::Item::Const(item_const) => {
                // Walk const initializer expressions for block items with attributes.
                self.visit_expr_for_local_items(&item_const.expr);
            }
            syn::Item::Static(item_static) => {
                // Walk static initializer expressions for block items with attributes.
                self.visit_expr_for_local_items(&item_static.expr);
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
        if let syn::ImplItem::Fn(method) = item {
            self.visit_block_for_local_items(&method.block);
        }
    }

    fn visit_trait_item(&mut self, item: &syn::TraitItem) {
        let attrs = trait_item_attrs(item);
        if Self::is_test_item(attrs) {
            return;
        }
        self.emit("trait_item", attrs);
        if let syn::TraitItem::Fn(method) = item {
            if let Some(body) = &method.default {
                self.visit_block_for_local_items(body);
            }
        }
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

    fn visit_block_for_local_items(&mut self, block: &syn::Block) {
        let mut visitor = LocalItemVisitor { node_visitor: self };
        syn::visit::visit_block(&mut visitor, block);
    }

    fn visit_expr_for_local_items(&mut self, expr: &syn::Expr) {
        let mut visitor = LocalItemVisitor { node_visitor: self };
        syn::visit::visit_expr(&mut visitor, expr);
    }
}

struct LocalItemVisitor<'node, 'a, F> {
    node_visitor: &'node mut NodeVisitor<'a, F>,
}

impl<'ast, 'node, 'a, F> syn::visit::Visit<'ast> for LocalItemVisitor<'node, 'a, F>
where
    F: FnMut(SynScanContext) -> Vec<VerifyFinding>,
{
    fn visit_item(&mut self, item: &'ast syn::Item) {
        self.node_visitor.visit_item(item);
    }
}

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
