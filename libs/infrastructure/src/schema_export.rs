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

    // T006 (S4): build trait_origins from rustdoc paths + external_crates.
    // For each impl that has a trait, look up the trait's defining crate via
    // krate.paths[trait_id].crate_id -> krate.external_crates[crate_id].name.
    // crate_id == 0 means the trait is defined in the current crate being documented.
    let trait_origins = build_trait_origins(crate_name, krate);

    for item in krate.index.values() {
        if item.crate_id != 0 {
            continue;
        }

        if let ItemEnum::Impl(i) = &item.inner {
            if i.is_synthetic || i.blanket_impl.is_some() || i.is_negative {
                continue;
            }
            let target = format_type(&i.for_);
            let trait_name = i
                .trait_
                .as_ref()
                .map(|p| p.path.rsplit("::").next().unwrap_or(&p.path).to_string());
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
                // Free functions: populate module_path from rustdoc paths table.
                let fn_module_path = extract_module_path(&item.id, krate);
                functions.push(FunctionInfo::with_module_path(
                    name,
                    item.docs.clone(),
                    return_type_names,
                    false,
                    params,
                    returns,
                    None,
                    f.header.is_async,
                    fn_module_path,
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

    SchemaExport::with_trait_origins(
        crate_name.to_owned(),
        types,
        functions,
        traits,
        impls,
        trait_origins,
    )
}

/// Builds a map of trait short name → defining crate name from rustdoc metadata.
///
/// For each impl block that implements a named trait, looks up the trait's `Id`
/// in `krate.paths` to find its `crate_id`, then resolves the crate name from
/// `krate.external_crates`. `crate_id == 0` means the trait is defined in the
/// current crate (`crate_name`).
///
/// The deduplication key is the trait's **definition path** from `krate.paths`
/// (a stable, canonical path) rather than the use-site `Path.path` string (which
/// may be an alias or relative path). This ensures that different use-site
/// spellings of the same trait are deduplicated correctly.
///
/// When two different traits share the same short name (e.g., a local `Display`
/// and `std::fmt::Display`), the local-crate trait (`crate_id == 0`) takes
/// precedence; if both are external, the one with the alphabetically-first
/// definition path wins, making the result deterministic regardless of the
/// `HashMap` iteration order of `krate.index`.
fn build_trait_origins(
    crate_name: &str,
    krate: &rustdoc_types::Crate,
) -> std::collections::HashMap<String, String> {
    // Collect unique trait definitions via a BTreeMap keyed by the trait's
    // canonical definition path (from krate.paths) to ensure deterministic
    // ordering and correct deduplication across different use-site spellings.
    let mut by_def_path: std::collections::BTreeMap<String, (String, String)> =
        std::collections::BTreeMap::new();

    for item in krate.index.values() {
        let ItemEnum::Impl(i) = &item.inner else { continue };
        if i.is_synthetic || i.blanket_impl.is_some() || i.is_negative {
            continue;
        }
        let trait_path = match &i.trait_ {
            Some(p) => p,
            None => continue,
        };
        // Use the definition path from krate.paths (stable, canonical) as the
        // deduplication key. Fall back to the use-site path only when the trait
        // id is not present in the paths table.
        let def_path = krate
            .paths
            .get(&trait_path.id)
            .map(|s| s.path.join("::"))
            .unwrap_or_else(|| trait_path.path.clone());
        if by_def_path.contains_key(&def_path) {
            continue;
        }
        let short_name = def_path.rsplit("::").next().unwrap_or(&def_path).to_string();
        let origin = resolve_trait_origin(crate_name, &trait_path.id, krate);
        by_def_path.insert(def_path, (short_name, origin));
    }

    // Collapse definition-path entries to short_name → origin.
    // If two definition paths share the same short name, prefer the local-crate
    // trait (origin == crate_name); otherwise the BTreeMap iteration order
    // (alphabetically first definition path) wins, which is deterministic.
    let mut origins = std::collections::HashMap::new();
    for (short_name, origin) in by_def_path.into_values() {
        origins
            .entry(short_name)
            .and_modify(|existing: &mut String| {
                // Overwrite only when the new origin is the local crate; otherwise
                // keep the existing (alphabetically-first) external crate name.
                if origin == crate_name {
                    *existing = origin.clone();
                }
            })
            .or_insert(origin);
    }

    origins
}

/// Resolves the origin crate name for a trait `Id`.
///
/// Returns `crate_name` when the trait is defined in the current crate
/// (`crate_id == 0`), the external crate name from `external_crates` otherwise,
/// or `""` when the `Id` is not found in `paths`.
fn resolve_trait_origin(
    crate_name: &str,
    trait_id: &rustdoc_types::Id,
    krate: &rustdoc_types::Crate,
) -> String {
    let summary = match krate.paths.get(trait_id) {
        Some(s) => s,
        None => return String::new(),
    };
    if summary.crate_id == 0 {
        return crate_name.to_string();
    }
    krate.external_crates.get(&summary.crate_id).map(|ec| ec.name.clone()).unwrap_or_default()
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

    // --- T006 (S4): build_trait_origins / resolve_trait_origin unit tests ---

    /// Helper: build a minimal `rustdoc_types::Crate` for testing `build_trait_origins`
    /// and `resolve_trait_origin`.
    fn minimal_krate(
        index: std::collections::HashMap<rustdoc_types::Id, rustdoc_types::Item>,
        paths: std::collections::HashMap<rustdoc_types::Id, rustdoc_types::ItemSummary>,
        external_crates: std::collections::HashMap<u32, rustdoc_types::ExternalCrate>,
    ) -> rustdoc_types::Crate {
        rustdoc_types::Crate {
            root: rustdoc_types::Id(0),
            crate_version: None,
            includes_private: false,
            index,
            paths,
            external_crates,
            target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
            format_version: 45,
        }
    }

    /// Helper: build a trait-impl `rustdoc_types::Item` with the given id and trait path id.
    fn make_impl_item(
        id: u32,
        trait_full_path: &str,
        trait_id: u32,
    ) -> (rustdoc_types::Id, rustdoc_types::Item) {
        let item_id = rustdoc_types::Id(id);
        let trait_path = rustdoc_types::Path {
            path: trait_full_path.to_string(),
            id: rustdoc_types::Id(trait_id),
            args: None,
        };
        let impl_inner = rustdoc_types::Impl {
            is_synthetic: false,
            is_unsafe: false,
            generics: rustdoc_types::Generics { params: vec![], where_predicates: vec![] },
            provided_trait_methods: vec![],
            trait_: Some(trait_path),
            for_: Type::Primitive("()".to_string()),
            items: vec![],
            is_negative: false,
            blanket_impl: None,
        };
        let item = rustdoc_types::Item {
            id: item_id,
            crate_id: 0,
            name: None,
            span: None,
            visibility: Visibility::Public,
            docs: None,
            links: std::collections::HashMap::new(),
            attrs: vec![],
            deprecation: None,
            inner: ItemEnum::Impl(impl_inner),
        };
        (item_id, item)
    }

    /// Helper: build an `ItemSummary` for a trait id with the given crate_id.
    fn make_item_summary(crate_id: u32, path: Vec<&str>) -> rustdoc_types::ItemSummary {
        rustdoc_types::ItemSummary {
            crate_id,
            path: path.into_iter().map(str::to_string).collect(),
            kind: rustdoc_types::ItemKind::Trait,
        }
    }

    /// Helper: build an `ExternalCrate` with the given name.
    fn make_external_crate(name: &str) -> rustdoc_types::ExternalCrate {
        rustdoc_types::ExternalCrate {
            name: name.to_string(),
            html_root_url: None,
            path: std::path::PathBuf::new(),
        }
    }

    /// T006b: trait defined in the current crate (crate_id == 0) maps to crate_name.
    #[test]
    fn test_resolve_trait_origin_local_crate_returns_crate_name() {
        let trait_id = rustdoc_types::Id(10);
        let mut paths = std::collections::HashMap::new();
        paths.insert(trait_id, make_item_summary(0, vec!["my_crate", "MyTrait"]));
        let krate = minimal_krate(
            std::collections::HashMap::new(),
            paths,
            std::collections::HashMap::new(),
        );
        let result = resolve_trait_origin("my_crate", &trait_id, &krate);
        assert_eq!(result, "my_crate");
    }

    /// T006b: trait defined in an external crate maps to that crate's name.
    #[test]
    fn test_resolve_trait_origin_external_crate_returns_crate_name() {
        let trait_id = rustdoc_types::Id(20);
        let mut paths = std::collections::HashMap::new();
        paths.insert(trait_id, make_item_summary(1, vec!["std", "fmt", "Display"]));
        let mut external_crates = std::collections::HashMap::new();
        external_crates.insert(1u32, make_external_crate("std"));
        let krate = minimal_krate(std::collections::HashMap::new(), paths, external_crates);
        let result = resolve_trait_origin("my_crate", &trait_id, &krate);
        assert_eq!(result, "std");
    }

    /// T006b: trait id not in paths returns empty string.
    #[test]
    fn test_resolve_trait_origin_missing_id_returns_empty() {
        let trait_id = rustdoc_types::Id(99);
        let krate = minimal_krate(
            std::collections::HashMap::new(),
            std::collections::HashMap::new(),
            std::collections::HashMap::new(),
        );
        let result = resolve_trait_origin("my_crate", &trait_id, &krate);
        assert_eq!(result, "");
    }

    /// T006b: build_trait_origins maps local trait short name → crate_name.
    #[test]
    fn test_build_trait_origins_local_trait_maps_to_crate_name() {
        let (impl_id, impl_item) = make_impl_item(1, "my_crate::MyTrait", 10);
        let trait_id = rustdoc_types::Id(10);
        let mut index = std::collections::HashMap::new();
        index.insert(impl_id, impl_item);
        let mut paths = std::collections::HashMap::new();
        paths.insert(trait_id, make_item_summary(0, vec!["my_crate", "MyTrait"]));
        let krate = minimal_krate(index, paths, std::collections::HashMap::new());

        let origins = build_trait_origins("my_crate", &krate);
        assert_eq!(origins.get("MyTrait"), Some(&"my_crate".to_string()));
    }

    /// T006b: build_trait_origins maps external trait short name → external crate name.
    #[test]
    fn test_build_trait_origins_external_trait_maps_to_external_crate_name() {
        let (impl_id, impl_item) = make_impl_item(1, "std::fmt::Display", 20);
        let trait_id = rustdoc_types::Id(20);
        let mut index = std::collections::HashMap::new();
        index.insert(impl_id, impl_item);
        let mut paths = std::collections::HashMap::new();
        paths.insert(trait_id, make_item_summary(1, vec!["std", "fmt", "Display"]));
        let mut external_crates = std::collections::HashMap::new();
        external_crates.insert(1u32, make_external_crate("std"));
        let krate = minimal_krate(index, paths, external_crates);

        let origins = build_trait_origins("my_crate", &krate);
        assert_eq!(origins.get("Display"), Some(&"std".to_string()));
    }

    /// T006b: when two traits share the same short name, the local-crate trait wins
    /// deterministically (not first-seen order from HashMap).
    #[test]
    fn test_build_trait_origins_local_crate_wins_short_name_conflict() {
        // local::Display (crate_id 0) and std::fmt::Display (crate_id 1) share "Display".
        // The local trait must win regardless of insertion/iteration order.
        let (impl_id_local, impl_item_local) = make_impl_item(1, "local::Display", 10);
        let (impl_id_ext, impl_item_ext) = make_impl_item(2, "std::fmt::Display", 20);
        let local_trait_id = rustdoc_types::Id(10);
        let ext_trait_id = rustdoc_types::Id(20);

        let mut index = std::collections::HashMap::new();
        index.insert(impl_id_local, impl_item_local);
        index.insert(impl_id_ext, impl_item_ext);
        let mut paths = std::collections::HashMap::new();
        paths.insert(local_trait_id, make_item_summary(0, vec!["local", "Display"]));
        paths.insert(ext_trait_id, make_item_summary(1, vec!["std", "fmt", "Display"]));
        let mut external_crates = std::collections::HashMap::new();
        external_crates.insert(1u32, make_external_crate("std"));
        let krate = minimal_krate(index, paths, external_crates);

        let origins = build_trait_origins("my_crate", &krate);
        // Local trait (crate_id == 0, origin == "my_crate") must take precedence.
        assert_eq!(origins.get("Display"), Some(&"my_crate".to_string()));
    }
}
