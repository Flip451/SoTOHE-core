//! Infrastructure adapter for `SchemaExporter` port.
//!
//! Uses `cargo +nightly rustdoc` to generate rustdoc JSON, then parses the output
//! with `rustdoc_types` to build domain `SchemaExport` values.

use std::path::{Path, PathBuf};
use std::process::Command;

use domain::schema::{
    FunctionInfo, ImplInfo, SchemaExport, SchemaExportError, SchemaExporter, TraitInfo, TypeInfo,
    TypeKind,
};
use rustdoc_types::{ItemEnum, Visibility};

/// Adapter implementing `SchemaExporter` via rustdoc JSON.
pub struct RustdocSchemaExporter {
    workspace_root: PathBuf,
}

impl RustdocSchemaExporter {
    /// Creates a new exporter for the given workspace root.
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

impl SchemaExporter for RustdocSchemaExporter {
    fn export(&self, crate_name: &str) -> Result<SchemaExport, SchemaExportError> {
        check_nightly_available()?;
        let json_path = run_rustdoc(&self.workspace_root, crate_name)?;
        let krate = parse_rustdoc_json(&json_path)?;
        Ok(build_schema_export(crate_name, &krate))
    }
}

fn check_nightly_available() -> Result<(), SchemaExportError> {
    let output = Command::new("rustup")
        .args(["run", "nightly", "rustc", "--version"])
        .output()
        .map_err(|_| SchemaExportError::NightlyNotFound)?;

    if !output.status.success() {
        return Err(SchemaExportError::NightlyNotFound);
    }
    Ok(())
}

fn run_rustdoc(workspace_root: &Path, crate_name: &str) -> Result<PathBuf, SchemaExportError> {
    let output = Command::new("cargo")
        .args([
            "+nightly",
            "rustdoc",
            "-p",
            crate_name,
            "--lib",
            "--",
            "-Z",
            "unstable-options",
            "--output-format",
            "json",
        ])
        .current_dir(workspace_root)
        .output()
        .map_err(|e| SchemaExportError::RustdocFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("did not match any packages")
            || (stderr.contains("package(s) `") && stderr.contains("not found in workspace"))
        {
            return Err(SchemaExportError::CrateNotFound(crate_name.to_owned()));
        }
        return Err(SchemaExportError::RustdocFailed(stderr.into_owned()));
    }

    let target_dir = resolve_target_dir(workspace_root)?;
    let artifact_name = crate_name.replace('-', "_");
    let json_path = target_dir.join("doc").join(format!("{artifact_name}.json"));

    if !json_path.is_file() {
        return Err(SchemaExportError::RustdocFailed(format!(
            "expected rustdoc JSON at {} but file not found",
            json_path.display()
        )));
    }

    Ok(json_path)
}

/// Resolves the Cargo target directory, respecting `CARGO_TARGET_DIR` and workspace config.
fn resolve_target_dir(workspace_root: &Path) -> Result<PathBuf, SchemaExportError> {
    // Check environment variable first
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        let path = PathBuf::from(dir);
        if path.is_relative() {
            return Ok(workspace_root.join(path));
        }
        return Ok(path);
    }
    // Fall back to `cargo metadata` for reliable resolution
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(workspace_root)
        .output()
        .map_err(|e| SchemaExportError::RustdocFailed(format!("cargo metadata failed: {e}")))?;

    if !output.status.success() {
        // Default fallback
        return Ok(workspace_root.join("target"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Extract target_directory from JSON without pulling in a full JSON parser dependency
    // (serde_json is already available via rustdoc_types)
    if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&stdout) {
        if let Some(dir) = meta.get("target_directory").and_then(|v| v.as_str()) {
            return Ok(PathBuf::from(dir));
        }
    }

    Ok(workspace_root.join("target"))
}

fn parse_rustdoc_json(path: &Path) -> Result<rustdoc_types::Crate, SchemaExportError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| SchemaExportError::ParseFailed(format!("read error: {e}")))?;
    serde_json::from_str(&content)
        .map_err(|e| SchemaExportError::ParseFailed(format!("JSON parse error: {e}")))
}

fn build_schema_export(crate_name: &str, krate: &rustdoc_types::Crate) -> SchemaExport {
    let mut types = Vec::new();
    let mut functions = Vec::new();
    let mut traits = Vec::new();
    let mut impls = Vec::new();

    // Collect Ids that belong to impl/trait blocks so we can exclude them from free functions.
    let mut method_ids: std::collections::HashSet<&rustdoc_types::Id> =
        std::collections::HashSet::new();
    for item in krate.index.values() {
        match &item.inner {
            ItemEnum::Impl(i) => method_ids.extend(&i.items),
            ItemEnum::Trait(t) => method_ids.extend(&t.items),
            _ => {}
        }
    }

    for item in krate.index.values() {
        // Skip items from external crates (crate_id 0 = local crate).
        if item.crate_id != 0 {
            continue;
        }

        // Impl blocks have name=None and non-Public visibility; handle them separately.
        if let ItemEnum::Impl(i) = &item.inner {
            if i.is_synthetic || i.blanket_impl.is_some() {
                continue;
            }
            let target = type_name(&i.for_);
            let trait_name = i.trait_.as_ref().map(|p| p.path.clone());
            let methods = extract_methods(&i.items, krate);
            if !methods.is_empty() || trait_name.is_some() {
                impls.push(ImplInfo::new(target, trait_name, methods));
            }
            continue;
        }

        if !matches!(item.visibility, Visibility::Public) {
            continue;
        }
        let name = match &item.name {
            Some(n) => n.clone(),
            None => continue,
        };

        match &item.inner {
            ItemEnum::Struct(s) => {
                let members = extract_struct_fields(s, krate);
                types.push(TypeInfo::new(name, TypeKind::Struct, item.docs.clone(), members));
            }
            ItemEnum::Enum(e) => {
                let variants = extract_enum_variants(e, krate);
                types.push(TypeInfo::new(name, TypeKind::Enum, item.docs.clone(), variants));
            }
            ItemEnum::TypeAlias(_) => {
                types.push(TypeInfo::new(name, TypeKind::TypeAlias, item.docs.clone(), Vec::new()));
            }
            ItemEnum::Function(f) if !method_ids.contains(&item.id) => {
                let sig = format_sig(&name, &f.sig);
                let return_type_names = extract_return_type_names(&f.sig);
                // Free functions never have a self receiver.
                functions.push(FunctionInfo::new(
                    name,
                    sig,
                    item.docs.clone(),
                    return_type_names,
                    false,
                ));
            }
            ItemEnum::Trait(t) => {
                let methods = extract_methods(&t.items, krate);
                traits.push(TraitInfo::new(name, item.docs.clone(), methods));
            }
            _ => {}
        }
    }

    // Sort for deterministic output (HashMap iteration order is non-deterministic).
    types.sort_by(|a, b| a.name().cmp(b.name()));
    functions.sort_by(|a, b| a.name().cmp(b.name()));
    traits.sort_by(|a, b| a.name().cmp(b.name()));
    impls.sort_by(|a, b| {
        a.target_type().cmp(b.target_type()).then_with(|| a.trait_name().cmp(&b.trait_name()))
    });

    SchemaExport::new(crate_name.to_owned(), types, functions, traits, impls)
}

/// Extract public field names from a struct.
fn extract_struct_fields(s: &rustdoc_types::Struct, krate: &rustdoc_types::Crate) -> Vec<String> {
    match &s.kind {
        rustdoc_types::StructKind::Plain { fields, .. } => fields
            .iter()
            .filter_map(|id| krate.index.get(id))
            .filter(|item| matches!(item.visibility, Visibility::Public))
            .filter_map(|item| item.name.clone())
            .collect(),
        rustdoc_types::StructKind::Tuple(fields) => fields
            .iter()
            .enumerate()
            .filter_map(|(i, opt)| opt.as_ref().map(|_| i.to_string()))
            .collect(),
        rustdoc_types::StructKind::Unit => Vec::new(),
    }
}

/// Extract variant names from an enum.
fn extract_enum_variants(e: &rustdoc_types::Enum, krate: &rustdoc_types::Crate) -> Vec<String> {
    e.variants
        .iter()
        .filter_map(|id| krate.index.get(id))
        .filter_map(|item| item.name.clone())
        .collect()
}

/// Returns `true` if the function signature's first parameter is a self receiver
/// (`self`, `&self`, or `&mut self`).
fn has_self_param(sig: &rustdoc_types::FunctionSignature) -> bool {
    sig.inputs.first().map(|(name, _)| name == "self").unwrap_or(false)
}

/// Extract method FunctionInfos from a list of item Ids.
/// Accepts both `Public` and `Default` visibility (trait associated items use `Default`).
fn extract_methods(ids: &[rustdoc_types::Id], krate: &rustdoc_types::Crate) -> Vec<FunctionInfo> {
    ids.iter()
        .filter_map(|id| krate.index.get(id))
        .filter(|item| matches!(item.visibility, Visibility::Public | Visibility::Default))
        .filter_map(|item| {
            let name = item.name.as_ref()?;
            if let ItemEnum::Function(f) = &item.inner {
                let return_type_names = extract_return_type_names(&f.sig);
                let has_self = has_self_param(&f.sig);
                Some(FunctionInfo::new(
                    name.clone(),
                    format_sig(name, &f.sig),
                    item.docs.clone(),
                    return_type_names,
                    has_self,
                ))
            } else {
                None
            }
        })
        .collect()
}

/// Extract the list of type names from the return type of a function signature.
fn extract_return_type_names(sig: &rustdoc_types::FunctionSignature) -> Vec<String> {
    sig.output.as_ref().map_or_else(Vec::new, |ty| {
        let mut names = Vec::new();
        collect_type_names(ty, &mut names);
        names
    })
}

/// Collect type names from a rustdoc `Type`, selectively unwrapping only
/// `Result<T, E>` and `Option<T>` (extracting the first generic argument).
///
/// Other generic wrappers (`Vec<T>`, `HashMap<K,V>`, `Box<T>`, `Arc<T>`, etc.)
/// are added as bare names without recursing into their type arguments.
/// `BorrowedRef` (`&T`) is unwrapped to extract the inner type.
/// `Tuple` elements are NOT expanded (tuples are not transition targets).
fn collect_type_names(ty: &rustdoc_types::Type, out: &mut Vec<String>) {
    match ty {
        rustdoc_types::Type::ResolvedPath(p) => {
            let name = p.path.rsplit("::").next().unwrap_or(&p.path);
            match name {
                "Result" | "Option" => {
                    // Unwrap first generic argument only.
                    if let Some(args) = &p.args {
                        if let rustdoc_types::GenericArgs::AngleBracketed { args, .. } =
                            args.as_ref()
                        {
                            if let Some(rustdoc_types::GenericArg::Type(inner)) = args.first() {
                                collect_type_names(inner, out);
                            }
                        }
                    }
                }
                _ => {
                    // Non-wrapper type — add bare name, do NOT recurse into generics.
                    out.push(name.to_string());
                }
            }
        }
        rustdoc_types::Type::BorrowedRef { type_: inner, .. } => {
            collect_type_names(inner, out);
        }
        _ => {}
    }
}

/// Build a human-readable signature from FunctionSignature.
/// Keeps it simple: only param names and top-level type names. No recursive Type formatting.
fn format_sig(name: &str, sig: &rustdoc_types::FunctionSignature) -> String {
    let params: Vec<String> = sig
        .inputs
        .iter()
        .map(|(param_name, ty)| format!("{param_name}: {}", type_name(ty)))
        .collect();
    let ret = sig.output.as_ref().map(|ty| format!(" -> {}", type_name(ty)));
    format!("fn {name}({}){}", params.join(", "), ret.unwrap_or_default())
}

/// Extract a short type name. Only resolves the outermost type — no recursive expansion.
fn type_name(ty: &rustdoc_types::Type) -> String {
    match ty {
        rustdoc_types::Type::ResolvedPath(p) => p.path.clone(),
        rustdoc_types::Type::Primitive(p) => p.clone(),
        rustdoc_types::Type::BorrowedRef { is_mutable, type_: inner, .. } => {
            if *is_mutable {
                format!("&mut {}", type_name(inner))
            } else {
                format!("&{}", type_name(inner))
            }
        }
        rustdoc_types::Type::Slice(inner) => format!("[{}]", type_name(inner)),
        rustdoc_types::Type::Tuple(types) if types.is_empty() => "()".to_owned(),
        _ => "_".to_owned(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Helper: build a `ResolvedPath` type with optional generic args.
    fn resolved(name: &str, args: Option<Vec<rustdoc_types::GenericArg>>) -> rustdoc_types::Type {
        rustdoc_types::Type::ResolvedPath(rustdoc_types::Path {
            path: name.to_string(),
            id: rustdoc_types::Id(0),
            args: args.map(|a| {
                Box::new(rustdoc_types::GenericArgs::AngleBracketed {
                    args: a,
                    constraints: vec![],
                })
            }),
        })
    }

    fn type_arg(ty: rustdoc_types::Type) -> rustdoc_types::GenericArg {
        rustdoc_types::GenericArg::Type(ty)
    }

    fn simple(name: &str) -> rustdoc_types::Type {
        resolved(name, None)
    }

    #[test]
    fn test_collect_type_names_result_unwraps_first_arg() {
        let ty = resolved(
            "Result",
            Some(vec![type_arg(simple("Published")), type_arg(simple("Error"))]),
        );
        let mut out = Vec::new();
        collect_type_names(&ty, &mut out);
        assert_eq!(out, vec!["Published"]);
    }

    #[test]
    fn test_collect_type_names_option_unwraps_first_arg() {
        let ty = resolved("Option", Some(vec![type_arg(simple("Published"))]));
        let mut out = Vec::new();
        collect_type_names(&ty, &mut out);
        assert_eq!(out, vec!["Published"]);
    }

    #[test]
    fn test_collect_type_names_vec_does_not_unwrap() {
        let ty = resolved("Vec", Some(vec![type_arg(simple("Published"))]));
        let mut out = Vec::new();
        collect_type_names(&ty, &mut out);
        assert_eq!(out, vec!["Vec"]);
    }

    #[test]
    fn test_collect_type_names_hashmap_does_not_unwrap() {
        let ty = resolved(
            "HashMap",
            Some(vec![type_arg(simple("String")), type_arg(simple("Published"))]),
        );
        let mut out = Vec::new();
        collect_type_names(&ty, &mut out);
        assert_eq!(out, vec!["HashMap"]);
    }

    #[test]
    fn test_collect_type_names_box_does_not_unwrap() {
        let ty = resolved("Box", Some(vec![type_arg(simple("Published"))]));
        let mut out = Vec::new();
        collect_type_names(&ty, &mut out);
        assert_eq!(out, vec!["Box"]);
    }

    #[test]
    fn test_collect_type_names_borrowed_ref_unwraps_inner() {
        let ty = rustdoc_types::Type::BorrowedRef {
            lifetime: None,
            is_mutable: false,
            type_: Box::new(simple("Draft")),
        };
        let mut out = Vec::new();
        collect_type_names(&ty, &mut out);
        assert_eq!(out, vec!["Draft"]);
    }

    #[test]
    fn test_collect_type_names_tuple_does_not_expand() {
        let ty = rustdoc_types::Type::Tuple(vec![simple("A"), simple("B")]);
        let mut out = Vec::new();
        collect_type_names(&ty, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn test_collect_type_names_simple_type_returns_bare_name() {
        let ty = simple("Published");
        let mut out = Vec::new();
        collect_type_names(&ty, &mut out);
        assert_eq!(out, vec!["Published"]);
    }

    #[test]
    fn test_collect_type_names_result_with_nested_option() {
        // Result<Option<Published>, Error> → unwrap Result → unwrap Option → Published
        let inner = resolved("Option", Some(vec![type_arg(simple("Published"))]));
        let ty = resolved("Result", Some(vec![type_arg(inner), type_arg(simple("Error"))]));
        let mut out = Vec::new();
        collect_type_names(&ty, &mut out);
        assert_eq!(out, vec!["Published"]);
    }
}
