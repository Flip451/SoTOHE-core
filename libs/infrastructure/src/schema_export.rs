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

    for item in krate.index.values() {
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
            ItemEnum::Function(f) => {
                let sig = format_sig(&name, &f.sig);
                functions.push(FunctionInfo::new(name, sig, item.docs.clone()));
            }
            ItemEnum::Trait(t) => {
                let methods = extract_methods(&t.items, krate);
                traits.push(TraitInfo::new(name, item.docs.clone(), methods));
            }
            ItemEnum::Impl(i) => {
                if i.is_synthetic || i.blanket_impl.is_some() {
                    continue;
                }
                let target = type_name(&i.for_);
                let trait_name = i.trait_.as_ref().map(|p| p.path.clone());
                let methods = extract_methods(&i.items, krate);
                if !methods.is_empty() || trait_name.is_some() {
                    impls.push(ImplInfo::new(target, trait_name, methods));
                }
            }
            _ => {}
        }
    }

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

/// Extract public method FunctionInfos from a list of item Ids.
fn extract_methods(ids: &[rustdoc_types::Id], krate: &rustdoc_types::Crate) -> Vec<FunctionInfo> {
    ids.iter()
        .filter_map(|id| krate.index.get(id))
        .filter(|item| matches!(item.visibility, Visibility::Public))
        .filter_map(|item| {
            let name = item.name.as_ref()?;
            if let ItemEnum::Function(f) = &item.inner {
                Some(FunctionInfo::new(name.clone(), format_sig(name, &f.sig), item.docs.clone()))
            } else {
                None
            }
        })
        .collect()
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
        rustdoc_types::Type::BorrowedRef { type_: inner, .. } => {
            format!("&{}", type_name(inner))
        }
        rustdoc_types::Type::Slice(inner) => format!("[{}]", type_name(inner)),
        rustdoc_types::Type::Tuple(types) if types.is_empty() => "()".to_owned(),
        _ => "_".to_owned(),
    }
}
