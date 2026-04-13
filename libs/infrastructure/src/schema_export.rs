//! Infrastructure adapter for `SchemaExporter` port.
//!
//! Uses `cargo +nightly rustdoc` to generate rustdoc JSON, then parses the output
//! with `rustdoc_types` to build domain `SchemaExport` values.
//!
//! T004 (TDDD-01 3c) rewrites `extract_methods` / `build_schema_export` to
//! populate the new structured signature fields (`params` / `returns` /
//! `receiver` / `is_async`) and to build `TypeInfo::members` as
//! `Vec<MemberDeclaration>`. The old single-string `signature` field is gone;
//! callers that need a human-readable signature should rebuild a
//! `MethodDeclaration` and call `signature_string()`. See ADR
//! `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` §Phase 1-4.

use std::path::{Path, PathBuf};
use std::process::Command;

use domain::schema::{
    FunctionInfo, ImplInfo, SchemaExport, SchemaExportError, SchemaExporter, TraitInfo, TypeInfo,
    TypeKind,
};
use domain::tddd::catalogue::{MemberDeclaration, ParamDeclaration};
use rustdoc_types::{GenericArg, GenericArgs, ItemEnum, Type, Visibility};

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
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        let path = PathBuf::from(dir);
        if path.is_relative() {
            return Ok(workspace_root.join(path));
        }
        return Ok(path);
    }
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(workspace_root)
        .output()
        .map_err(|e| SchemaExportError::RustdocFailed(format!("cargo metadata failed: {e}")))?;

    if !output.status.success() {
        return Ok(workspace_root.join("target"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
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
        if item.crate_id != 0 {
            continue;
        }

        if let ItemEnum::Impl(i) = &item.inner {
            if i.is_synthetic || i.blanket_impl.is_some() {
                continue;
            }
            let target = format_type(&i.for_);
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

        let module_path = extract_module_path(&item.id, krate);

        match &item.inner {
            ItemEnum::Struct(s) => {
                let members = extract_struct_fields(s, krate);
                let ti = if let Some(mp) = module_path {
                    TypeInfo::with_module_path(
                        name,
                        TypeKind::Struct,
                        item.docs.clone(),
                        members,
                        mp,
                    )
                } else {
                    TypeInfo::new(name, TypeKind::Struct, item.docs.clone(), members)
                };
                types.push(ti);
            }
            ItemEnum::Enum(e) => {
                let variants = extract_enum_variants(e, krate);
                let ti = if let Some(mp) = module_path {
                    TypeInfo::with_module_path(
                        name,
                        TypeKind::Enum,
                        item.docs.clone(),
                        variants,
                        mp,
                    )
                } else {
                    TypeInfo::new(name, TypeKind::Enum, item.docs.clone(), variants)
                };
                types.push(ti);
            }
            ItemEnum::TypeAlias(_) => {
                let ti = if let Some(mp) = module_path {
                    TypeInfo::with_module_path(
                        name,
                        TypeKind::TypeAlias,
                        item.docs.clone(),
                        Vec::new(),
                        mp,
                    )
                } else {
                    TypeInfo::new(name, TypeKind::TypeAlias, item.docs.clone(), Vec::new())
                };
                types.push(ti);
            }
            ItemEnum::Function(f) if !method_ids.contains(&item.id) => {
                let return_type_names = extract_return_type_names(&f.sig);
                let params = extract_params(&f.sig);
                let returns = format_return(&f.sig);
                // Free functions never have a self receiver.
                functions.push(FunctionInfo::new(
                    name,
                    item.docs.clone(),
                    return_type_names,
                    false,
                    params,
                    returns,
                    None,
                    f.header.is_async,
                ));
            }
            ItemEnum::Trait(t) => {
                let methods = extract_methods(&t.items, krate);
                traits.push(TraitInfo::new(name, item.docs.clone(), methods));
            }
            _ => {}
        }
    }

    types.sort_by(|a, b| a.name().cmp(b.name()));
    functions.sort_by(|a, b| a.name().cmp(b.name()));
    traits.sort_by(|a, b| a.name().cmp(b.name()));
    impls.sort_by(|a, b| {
        a.target_type().cmp(b.target_type()).then_with(|| a.trait_name().cmp(&b.trait_name()))
    });

    SchemaExport::new(crate_name.to_owned(), types, functions, traits, impls)
}

/// Extract public fields from a struct as `MemberDeclaration::Field`.
fn extract_struct_fields(
    s: &rustdoc_types::Struct,
    krate: &rustdoc_types::Crate,
) -> Vec<MemberDeclaration> {
    match &s.kind {
        rustdoc_types::StructKind::Plain { fields, .. } => fields
            .iter()
            .filter_map(|id| krate.index.get(id))
            .filter(|item| matches!(item.visibility, Visibility::Public))
            .filter_map(|item| {
                let name = item.name.clone()?;
                if let ItemEnum::StructField(ty) = &item.inner {
                    Some(MemberDeclaration::field(name, format_type(ty)))
                } else {
                    None
                }
            })
            .collect(),
        rustdoc_types::StructKind::Tuple(fields) => fields
            .iter()
            .enumerate()
            .filter_map(|(i, opt)| {
                let id = opt.as_ref()?;
                let item = krate.index.get(id)?;
                if let ItemEnum::StructField(ty) = &item.inner {
                    Some(MemberDeclaration::field(i.to_string(), format_type(ty)))
                } else {
                    None
                }
            })
            .collect(),
        rustdoc_types::StructKind::Unit => Vec::new(),
    }
}

/// Extract enum variants as `MemberDeclaration::Variant`.
fn extract_enum_variants(
    e: &rustdoc_types::Enum,
    krate: &rustdoc_types::Crate,
) -> Vec<MemberDeclaration> {
    e.variants
        .iter()
        .filter_map(|id| krate.index.get(id))
        .filter_map(|item| item.name.clone())
        .map(MemberDeclaration::variant)
        .collect()
}

/// Extract the module path for a type from the rustdoc `paths` table.
fn extract_module_path(id: &rustdoc_types::Id, krate: &rustdoc_types::Crate) -> Option<String> {
    let summary = krate.paths.get(id)?;
    summary
        .path
        .get(..summary.path.len().saturating_sub(1))
        .filter(|parent| !parent.is_empty())
        .map(|parent| parent.join("::"))
}

/// Returns the self-receiver form (`"&self"` / `"&mut self"` / `"self"`), or
/// `None` if the first input is not a self receiver.
fn extract_receiver(sig: &rustdoc_types::FunctionSignature) -> Option<String> {
    let (name, ty) = sig.inputs.first()?;
    if name != "self" {
        return None;
    }
    match ty {
        Type::BorrowedRef { is_mutable: false, .. } => Some("&self".to_string()),
        Type::BorrowedRef { is_mutable: true, .. } => Some("&mut self".to_string()),
        _ => Some("self".to_string()),
    }
}

/// Returns `true` if the function signature's first parameter is a self receiver.
fn has_self_param(sig: &rustdoc_types::FunctionSignature) -> bool {
    sig.inputs.first().map(|(name, _)| name == "self").unwrap_or(false)
}

/// Extract the ordered parameter list from a function signature, excluding
/// the self receiver if present.
fn extract_params(sig: &rustdoc_types::FunctionSignature) -> Vec<ParamDeclaration> {
    sig.inputs
        .iter()
        .filter(|(name, _)| name != "self")
        .map(|(name, ty)| ParamDeclaration::new(name.clone(), format_type(ty)))
        .collect()
}

/// Format the return type. `Option<Type>::None` is rendered as `"()"`.
fn format_return(sig: &rustdoc_types::FunctionSignature) -> String {
    sig.output.as_ref().map_or_else(|| "()".to_string(), format_type)
}

/// Extract method `FunctionInfo`s from a list of item Ids.
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
                let receiver = extract_receiver(&f.sig);
                let params = extract_params(&f.sig);
                let returns = format_return(&f.sig);
                Some(FunctionInfo::new(
                    name.clone(),
                    item.docs.clone(),
                    return_type_names,
                    has_self,
                    params,
                    returns,
                    receiver,
                    f.header.is_async,
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
fn collect_type_names(ty: &Type, out: &mut Vec<String>) {
    match ty {
        Type::ResolvedPath(p) => {
            let name = p.path.rsplit("::").next().unwrap_or(&p.path);
            match name {
                "Result" | "Option" => {
                    if let Some(args) = &p.args {
                        if let GenericArgs::AngleBracketed { args, .. } = args.as_ref() {
                            if let Some(GenericArg::Type(inner)) = args.first() {
                                collect_type_names(inner, out);
                            }
                        }
                    }
                }
                _ => {
                    out.push(name.to_string());
                }
            }
        }
        Type::BorrowedRef { type_: inner, .. } => {
            collect_type_names(inner, out);
        }
        _ => {}
    }
}

/// Recursive type formatter at L1 resolution.
///
/// Renders a rustdoc `Type` as a short-name string, preserving generic
/// structure verbatim. Module paths are stripped (last segment only). The
/// unit type `()` is rendered explicitly.
fn format_type(ty: &Type) -> String {
    match ty {
        Type::ResolvedPath(p) => {
            let short = p.path.rsplit("::").next().unwrap_or(&p.path).to_string();
            if let Some(args) = &p.args {
                let rendered = format_args(args);
                if rendered.is_empty() { short } else { format!("{short}<{rendered}>") }
            } else {
                short
            }
        }
        Type::Generic(name) => name.clone(),
        Type::Primitive(name) => name.clone(),
        Type::BorrowedRef { is_mutable, type_: inner, .. } => {
            let mut_str = if *is_mutable { "mut " } else { "" };
            format!("&{mut_str}{}", format_type(inner))
        }
        Type::Slice(inner) => format!("[{}]", format_type(inner)),
        Type::Array { type_: inner, len } => {
            // Sanitize: const-generic length expressions may contain `::` (e.g. `N::VALUE`).
            // Replace `::` with `.` to preserve the L1 invariant that rendered type strings
            // never contain `::`.
            let safe_len = len.replace("::", ".");
            format!("[{}; {}]", format_type(inner), safe_len)
        }
        Type::Tuple(tys) if tys.is_empty() => "()".to_string(),
        Type::Tuple(tys) => {
            let items: Vec<String> = tys.iter().map(format_type).collect();
            format!("({})", items.join(", "))
        }
        Type::RawPointer { is_mutable, type_: inner } => {
            let kw = if *is_mutable { "mut" } else { "const" };
            format!("*{kw} {}", format_type(inner))
        }
        Type::ImplTrait(bounds) => {
            let rendered = bounds
                .iter()
                .filter_map(|b| match b {
                    rustdoc_types::GenericBound::TraitBound { trait_, .. } => {
                        Some(trait_.path.rsplit("::").next().unwrap_or(&trait_.path).to_string())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" + ");
            if rendered.is_empty() { "impl _".to_string() } else { format!("impl {rendered}") }
        }
        Type::DynTrait(dyn_trait) => {
            let rendered = dyn_trait
                .traits
                .iter()
                .map(|pt| pt.trait_.path.rsplit("::").next().unwrap_or(&pt.trait_.path).to_string())
                .collect::<Vec<_>>()
                .join(" + ");
            if rendered.is_empty() { "dyn _".to_string() } else { format!("dyn {rendered}") }
        }
        _ => "_".to_string(),
    }
}

/// Render angle-bracketed generic argument lists. Lifetime and const
/// arguments are preserved in source order; type arguments are recursively
/// formatted via `format_type`.
fn format_args(args: &GenericArgs) -> String {
    match args {
        GenericArgs::AngleBracketed { args, .. } => args
            .iter()
            .map(|arg| match arg {
                GenericArg::Type(t) => format_type(t),
                GenericArg::Lifetime(lt) => lt.clone(),
                // Sanitize: const expressions may contain `::` (e.g. `N::VALUE`).
                // Replace `::` with `.` to preserve the L1 invariant that rendered
                // type strings never contain `::`.
                GenericArg::Const(c) => c.expr.replace("::", "."),
                GenericArg::Infer => "_".to_string(),
            })
            .collect::<Vec<_>>()
            .join(", "),
        GenericArgs::Parenthesized { .. } => String::new(),
        GenericArgs::ReturnTypeNotation => String::new(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// Helper: build a `ResolvedPath` type with optional generic args.
    fn resolved(name: &str, args: Option<Vec<GenericArg>>) -> Type {
        Type::ResolvedPath(rustdoc_types::Path {
            path: name.to_string(),
            id: rustdoc_types::Id(0),
            args: args
                .map(|a| Box::new(GenericArgs::AngleBracketed { args: a, constraints: vec![] })),
        })
    }

    fn type_arg(ty: Type) -> GenericArg {
        GenericArg::Type(ty)
    }

    fn simple(name: &str) -> Type {
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
        let ty = Type::BorrowedRef {
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
        let ty = Type::Tuple(vec![simple("A"), simple("B")]);
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
        let inner = resolved("Option", Some(vec![type_arg(simple("Published"))]));
        let ty = resolved("Result", Some(vec![type_arg(inner), type_arg(simple("Error"))]));
        let mut out = Vec::new();
        collect_type_names(&ty, &mut out);
        assert_eq!(out, vec!["Published"]);
    }

    #[test]
    fn format_type_strips_module_path_to_short_name() {
        let ty = resolved("domain::review::Draft", None);
        assert_eq!(format_type(&ty), "Draft");
    }

    #[test]
    fn format_type_preserves_generics_recursively() {
        let inner = resolved("Option", Some(vec![type_arg(simple("User"))]));
        let ty = resolved("Result", Some(vec![type_arg(inner), type_arg(simple("DomainError"))]));
        assert_eq!(format_type(&ty), "Result<Option<User>, DomainError>");
    }

    #[test]
    fn format_type_renders_borrowed_ref() {
        let ty =
            Type::BorrowedRef { lifetime: None, is_mutable: false, type_: Box::new(simple("str")) };
        assert_eq!(format_type(&ty), "&str");

        let mut_ty =
            Type::BorrowedRef { lifetime: None, is_mutable: true, type_: Box::new(simple("User")) };
        assert_eq!(format_type(&mut_ty), "&mut User");
    }

    #[test]
    fn format_type_renders_unit_tuple() {
        let ty = Type::Tuple(vec![]);
        assert_eq!(format_type(&ty), "()");
    }

    #[test]
    fn format_return_maps_none_to_unit() {
        let sig =
            rustdoc_types::FunctionSignature { inputs: vec![], output: None, is_c_variadic: false };
        assert_eq!(format_return(&sig), "()");
    }

    #[test]
    fn format_return_renders_resolved_path() {
        let sig = rustdoc_types::FunctionSignature {
            inputs: vec![],
            output: Some(simple("Published")),
            is_c_variadic: false,
        };
        assert_eq!(format_return(&sig), "Published");
    }

    #[test]
    fn extract_receiver_detects_ref_self() {
        let self_ty = Type::BorrowedRef {
            lifetime: None,
            is_mutable: false,
            type_: Box::new(simple("Self")),
        };
        let sig = rustdoc_types::FunctionSignature {
            inputs: vec![("self".to_string(), self_ty)],
            output: None,
            is_c_variadic: false,
        };
        assert_eq!(extract_receiver(&sig), Some("&self".to_string()));
    }

    #[test]
    fn extract_receiver_detects_owned_self() {
        let sig = rustdoc_types::FunctionSignature {
            inputs: vec![("self".to_string(), simple("Self"))],
            output: None,
            is_c_variadic: false,
        };
        assert_eq!(extract_receiver(&sig), Some("self".to_string()));
    }

    #[test]
    fn extract_receiver_none_for_associated_function() {
        let sig = rustdoc_types::FunctionSignature {
            inputs: vec![("id".to_string(), simple("UserId"))],
            output: None,
            is_c_variadic: false,
        };
        assert_eq!(extract_receiver(&sig), None);
    }

    #[test]
    fn extract_params_skips_self_receiver() {
        let self_ty = Type::BorrowedRef {
            lifetime: None,
            is_mutable: false,
            type_: Box::new(simple("Self")),
        };
        let sig = rustdoc_types::FunctionSignature {
            inputs: vec![
                ("self".to_string(), self_ty),
                ("id".to_string(), simple("UserId")),
                ("name".to_string(), simple("String")),
            ],
            output: None,
            is_c_variadic: false,
        };
        let params = extract_params(&sig);
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name(), "id");
        assert_eq!(params[0].ty(), "UserId");
        assert_eq!(params[1].name(), "name");
        assert_eq!(params[1].ty(), "String");
    }
}
