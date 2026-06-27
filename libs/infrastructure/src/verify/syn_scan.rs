//! Reusable syn-based AST scanner for `sotp verify` gates.
//!
//! Provides [`scan_workspace_rust_sources`] which discovers `.rs` files from
//! `architecture-rules.json` `layers[]`, parses each file with
//! [`syn::parse_file`], and invokes a caller-supplied callback for every
//! remaining attribute-bearing AST node.
//!
//! Later syn-based lint gates (e.g. `Result<_, String>` ban) can reuse this
//! scanner by supplying a different detection callback.

use std::path::{Path, PathBuf};

use domain::verify::{VerifyFinding, VerifyOutcome};

use crate::track::symlink_guard::reject_symlinks_below;

use super::path_safety::lexical_normalize;
use super::syn_helpers::{foreign_item_attrs, impl_item_attrs, item_attrs, trait_item_attrs};
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
/// source files under each layer's `{path}` directory (including `src/`,
/// `tests/`, `examples/`, `benches/`, and any other subdirectories), parses
/// each file with [`syn::parse_file`], and invokes `inspect` for every
/// attribute-bearing AST node (file-level inner attributes, items, impl/trait
/// associated items, struct fields, enum variants, foreign items).
///
/// `target/` and `.git/` directories are excluded at every nesting level.
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
        let layer_dir = match resolve_layer_dir(&trusted_root, &layer.crate_name, &layer.path) {
            Ok(path) => path,
            Err(finding) => {
                findings.push(finding);
                continue;
            }
        };
        scan_rs_files_in_dir(&trusted_root, &layer_dir, &mut findings, &mut inspect);
    }

    VerifyOutcome::from_findings(findings)
}

fn resolve_layer_dir(
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

    Ok(normalized_layer_path)
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
        // Skip build-artifact and VCS directories at every nesting level.
        if let Some(name) = path.file_name() {
            if name == "target" || name == ".git" {
                continue;
            }
        }

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

    fn visit_item(&mut self, item: &syn::Item) {
        let attrs = item_attrs(item);

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
        self.emit("impl_item", attrs);
        match item {
            syn::ImplItem::Fn(method) => {
                self.visit_block_for_local_items(&method.block);
            }
            syn::ImplItem::Const(item_const) => {
                // Walk associated const initializer expressions for block items
                // with attributes (mirrors top-level Item::Const handling).
                self.visit_expr_for_local_items(&item_const.expr);
            }
            _ => {}
        }
    }

    fn visit_trait_item(&mut self, item: &syn::TraitItem) {
        let attrs = trait_item_attrs(item);
        self.emit("trait_item", attrs);
        match item {
            syn::TraitItem::Fn(method) => {
                if let Some(body) = &method.default {
                    self.visit_block_for_local_items(body);
                }
            }
            syn::TraitItem::Const(item_const) => {
                // Walk trait associated const default value expressions for block
                // items with attributes (mirrors top-level Item::Const handling).
                if let Some((_, expr)) = &item_const.default {
                    self.visit_expr_for_local_items(expr);
                }
            }
            _ => {}
        }
    }

    fn visit_field(&mut self, field: &syn::Field) {
        self.emit("field", &field.attrs);
    }

    fn visit_variant(&mut self, variant: &syn::Variant) {
        self.emit("variant", &variant.attrs);
        // Also visit fields inside the variant (for struct-like variants).
        for field in &variant.fields {
            self.visit_field(field);
        }
    }

    fn visit_foreign_item(&mut self, item: &syn::ForeignItem) {
        let attrs = foreign_item_attrs(item);
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
