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
use rustdoc_types::{ItemEnum, Visibility};

#[path = "schema_export/format_helpers.rs"]
mod format_helpers;

#[path = "schema_export/bin_target.rs"]
mod bin_target;

#[path = "schema_export/extract.rs"]
mod extract;
use extract::{
    extract_enum_variants, extract_methods, extract_module_path, extract_params,
    extract_return_type_names, extract_struct_fields, format_return,
};

#[path = "schema_export/trait_origins.rs"]
mod trait_origins;
use trait_origins::build_trait_origins;

#[path = "schema_export/path_resolution.rs"]
mod path_resolution;
use path_resolution::{
    absolutize_for_target_guard, checked_workspace_root, parse_rustdoc_json,
    reject_symlinks_for_rustdoc_path,
};

/// Adapter implementing `SchemaExporter` via rustdoc JSON.
pub struct RustdocSchemaExporter {
    workspace_root: PathBuf,
}

impl RustdocSchemaExporter {
    /// Creates a new exporter for the given workspace root.
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    /// Runs `cargo +nightly rustdoc --output-format json` for `crate_name` and
    /// returns the path to the generated JSON file.
    ///
    /// The file is written inside the workspace's `target/doc/` directory.
    /// The caller is responsible for reading or copying the file before the
    /// next rustdoc run overwrites it.
    ///
    /// # Errors
    ///
    /// Returns [`SchemaExportError::NightlyNotFound`] if the nightly toolchain
    /// is not installed.
    ///
    /// Returns [`SchemaExportError::RustdocFailed`] if `cargo rustdoc` fails.
    ///
    /// Returns [`SchemaExportError::CrateNotFound`] if the crate is not in the
    /// workspace.
    pub fn export_rustdoc_json_path(
        &self,
        crate_name: &str,
    ) -> Result<std::path::PathBuf, SchemaExportError> {
        check_nightly_available()?;
        bin_target::run_rustdoc(&self.workspace_root, crate_name)
    }
}

impl SchemaExporter for RustdocSchemaExporter {
    fn export(&self, crate_name: &str) -> Result<SchemaExport, SchemaExportError> {
        check_nightly_available()?;
        let json_path = bin_target::run_rustdoc(&self.workspace_root, crate_name)?;
        let krate = parse_rustdoc_json(&json_path)?;
        build_schema_export(crate_name, &krate)
    }
}

impl usecase::export_schema::SchemaExporterPort for RustdocSchemaExporter {
    fn export_as_json(
        &self,
        crate_name: &str,
    ) -> Result<String, usecase::export_schema::SchemaExporterError> {
        let export = <Self as SchemaExporter>::export(self, crate_name).map_err(|err| {
            usecase::export_schema::SchemaExporterError::ExportFailed(err.to_string())
        })?;
        crate::schema_export_codec::encode(&export, true).map_err(|err| {
            usecase::export_schema::SchemaExporterError::ExportFailed(err.to_string())
        })
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

/// Validate that a computed rustdoc JSON path is safely rooted at `target_dir`.
///
/// `target_dir` is the resolved Cargo target directory (from `resolve_target_dir`),
/// which may legitimately live outside the workspace when callers set an absolute
/// `CARGO_TARGET_DIR` (e.g., `/cargo-target` in CI containers — see the
/// Dockerfile's `IMAGE_CARGO_TARGET_DIR`). The workspace root is not the
/// authoritative trust boundary for the JSON path; the target directory is.
///
/// Checks:
/// 1. `target_dir` itself is not a symlink (delegated to `checked_workspace_root`).
/// 2. The normalized JSON path stays beneath `target_dir` (catches escapes via
///    crafted relative segments).
/// 3. No symlinks beneath `target_dir` redirect the JSON path elsewhere.
pub(super) fn ensure_rustdoc_json_path_safe(
    target_dir: &Path,
    json_path: &Path,
    source: &str,
) -> Result<(), SchemaExportError> {
    let trusted_root = checked_workspace_root(target_dir)?;
    let json_abs = absolutize_for_target_guard(json_path)?;
    let normalized_json = crate::verify::path_safety::lexical_normalize(&json_abs);

    if !normalized_json.starts_with(&trusted_root) {
        return Err(SchemaExportError::RustdocFailed(format!(
            "{source} resolves rustdoc JSON outside target directory: {} (target dir: {})",
            json_path.display(),
            target_dir.display()
        )));
    }

    reject_symlinks_for_rustdoc_path(&normalized_json, &trusted_root, source)?;
    Ok(())
}

fn build_schema_export(
    crate_name: &str,
    krate: &rustdoc_types::Crate,
) -> Result<SchemaExport, SchemaExportError> {
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
            let target = format_helpers::format_type(&i.for_);
            let (trait_name, trait_def_path) = match &i.trait_ {
                Some(p) => {
                    let short = p.path.rsplit("::").next().unwrap_or(&p.path).to_string();
                    // Stable def_path from krate.paths.  When the trait id is absent from
                    // krate.paths the use-site `p.path` is NOT a stable identity key — it
                    // is an import-spelling that can alias different traits.  Keep the
                    // def_path as `None` so the origin lookup in code_profile_builder
                    // gracefully degrades to an empty origin rather than recording a
                    // potentially wrong mapping.
                    let def_path = krate.paths.get(&p.id).map(|s| s.path.join("::"));
                    (Some(short), def_path)
                }
                None => (None, None),
            };
            let methods = extract_methods(&i.items, krate)?;
            if !methods.is_empty() || trait_name.is_some() {
                impls.push(ImplInfo::with_trait_def_path(
                    target,
                    trait_name,
                    methods,
                    trait_def_path,
                ));
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
                let variants = extract_enum_variants(e, krate)?;
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
                let params = extract_params(&f.sig)?;
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
                let methods = extract_methods(&t.items, krate)?;
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

    Ok(SchemaExport::with_trait_origins(
        crate_name.to_owned(),
        types,
        functions,
        traits,
        impls,
        trait_origins,
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use rustdoc_types::{
        FunctionHeader, FunctionPointer, FunctionSignature, GenericArg, GenericArgs,
    };

    use super::extract::{extract_enum_variants, extract_params, extract_receiver, format_return};
    use super::format_helpers::{collect_type_names, format_type};
    use super::path_resolution::resolve_configured_target_dir;
    use super::trait_origins::{build_trait_origins, resolve_trait_origin};
    use super::*;

    /// Helper: build a `ResolvedPath` type with optional generic args.
    fn resolved(name: &str, args: Option<Vec<GenericArg>>) -> rustdoc_types::Type {
        rustdoc_types::Type::ResolvedPath(rustdoc_types::Path {
            path: name.to_string(),
            id: rustdoc_types::Id(0),
            args: args
                .map(|a| Box::new(GenericArgs::AngleBracketed { args: a, constraints: vec![] })),
        })
    }

    fn type_arg(ty: rustdoc_types::Type) -> GenericArg {
        GenericArg::Type(ty)
    }

    fn simple(name: &str) -> rustdoc_types::Type {
        resolved(name, None)
    }

    #[test]
    fn test_resolve_configured_target_dir_relative_inside_workspace_returns_target() {
        let workspace = tempfile::tempdir().unwrap();

        let target = resolve_configured_target_dir(
            workspace.path(),
            PathBuf::from("target"),
            "CARGO_TARGET_DIR",
        )
        .unwrap();

        assert_eq!(target, workspace.path().join("target"));
    }

    #[test]
    fn test_resolve_configured_target_dir_relative_escape_returns_error() {
        let workspace = tempfile::tempdir().unwrap();

        let err = resolve_configured_target_dir(
            workspace.path(),
            PathBuf::from("../outside"),
            "CARGO_TARGET_DIR",
        )
        .unwrap_err();

        assert!(matches!(err, SchemaExportError::RustdocFailed(_)));
        assert!(err.to_string().contains("outside workspace root"));
    }

    #[cfg(unix)]
    #[test]
    fn test_resolve_configured_target_dir_symlinked_workspace_returns_error() {
        let parent = tempfile::tempdir().unwrap();
        let workspace = parent.path().join("workspace");
        let workspace_link = parent.path().join("workspace-link");
        std::fs::create_dir(&workspace).unwrap();
        std::os::unix::fs::symlink(&workspace, &workspace_link).unwrap();

        let err = resolve_configured_target_dir(
            &workspace_link,
            PathBuf::from("target"),
            "CARGO_TARGET_DIR",
        )
        .unwrap_err();

        assert!(matches!(err, SchemaExportError::RustdocFailed(_)));
        assert!(err.to_string().contains("symlink guard"));
    }

    #[cfg(unix)]
    #[test]
    fn test_resolve_configured_target_dir_symlinked_target_returns_error() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target_link = workspace.path().join("target");
        std::os::unix::fs::symlink(outside.path(), &target_link).unwrap();

        let err = resolve_configured_target_dir(
            workspace.path(),
            PathBuf::from("target"),
            "CARGO_TARGET_DIR",
        )
        .unwrap_err();

        assert!(matches!(err, SchemaExportError::RustdocFailed(_)));
        assert!(err.to_string().contains("symlink guard"));
    }

    #[cfg(unix)]
    #[test]
    fn test_ensure_rustdoc_json_path_safe_symlinked_leaf_returns_error() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let doc_dir = workspace.path().join("target/doc");
        let outside_json = outside.path().join("cli.json");
        let json_link = doc_dir.join("cli.json");
        std::fs::create_dir_all(&doc_dir).unwrap();
        std::fs::write(&outside_json, "{}").unwrap();
        std::os::unix::fs::symlink(&outside_json, &json_link).unwrap();

        let err = ensure_rustdoc_json_path_safe(workspace.path(), &json_link, "rustdoc --lib")
            .unwrap_err();

        assert!(matches!(err, SchemaExportError::RustdocFailed(_)));
        assert!(err.to_string().contains("symlink guard"));
    }

    fn trait_bound(name: &str) -> rustdoc_types::GenericBound {
        rustdoc_types::GenericBound::TraitBound {
            trait_: rustdoc_types::Path {
                path: name.to_owned(),
                id: rustdoc_types::Id(0),
                args: None,
            },
            generic_params: vec![],
            modifier: rustdoc_types::TraitBoundModifier::None,
        }
    }

    fn poly_trait(name: &str) -> rustdoc_types::PolyTrait {
        rustdoc_types::PolyTrait {
            trait_: rustdoc_types::Path {
                path: name.to_owned(),
                id: rustdoc_types::Id(0),
                args: None,
            },
            generic_params: vec![],
        }
    }

    fn default_fn_header() -> FunctionHeader {
        FunctionHeader {
            is_async: false,
            is_const: false,
            is_unsafe: false,
            abi: rustdoc_types::Abi::Rust,
        }
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
        let ty = rustdoc_types::Type::BorrowedRef {
            lifetime: None,
            is_mutable: false,
            type_: Box::new(simple("str")),
        };
        assert_eq!(format_type(&ty), "&str");

        let mut_ty = rustdoc_types::Type::BorrowedRef {
            lifetime: None,
            is_mutable: true,
            type_: Box::new(simple("User")),
        };
        assert_eq!(format_type(&mut_ty), "&mut User");
    }

    #[test]
    fn format_type_renders_unit_tuple() {
        let ty = rustdoc_types::Type::Tuple(vec![]);
        assert_eq!(format_type(&ty), "()");
    }

    #[test]
    fn format_type_preserves_impl_trait_bound_order_for_schema_export() {
        let ty =
            rustdoc_types::Type::ImplTrait(vec![trait_bound("crate::B"), trait_bound("crate::A")]);
        assert_eq!(format_type(&ty), "impl B + A");
    }

    #[test]
    fn format_type_preserves_dyn_trait_bound_order_for_schema_export() {
        let ty = rustdoc_types::Type::DynTrait(rustdoc_types::DynTrait {
            traits: vec![poly_trait("crate::B"), poly_trait("crate::A")],
            lifetime: Some("'a".to_owned()),
        });
        assert_eq!(format_type(&ty), "dyn B + A");
    }

    #[test]
    fn format_type_ignores_associated_type_constraints_for_schema_export() {
        let ty = rustdoc_types::Type::ResolvedPath(rustdoc_types::Path {
            path: "Iterator".to_owned(),
            id: rustdoc_types::Id(0),
            args: Some(Box::new(GenericArgs::AngleBracketed {
                args: vec![],
                constraints: vec![rustdoc_types::AssocItemConstraint {
                    name: "Item".to_owned(),
                    args: None,
                    binding: rustdoc_types::AssocItemConstraintKind::Equality(
                        rustdoc_types::Term::Type(simple("u8")),
                    ),
                }],
            })),
        });
        assert_eq!(format_type(&ty), "Iterator");
    }

    #[test]
    fn format_type_renders_function_pointer_for_schema_export() {
        let ty = rustdoc_types::Type::FunctionPointer(Box::new(FunctionPointer {
            sig: FunctionSignature {
                inputs: vec![("_".to_string(), simple("Input"))],
                output: Some(simple("Output")),
                is_c_variadic: false,
            },
            header: default_fn_header(),
            generic_params: vec![],
        }));
        assert_eq!(format_type(&ty), "fn(Input)->Output");
    }

    #[test]
    fn format_type_renders_pattern_base_type_for_schema_export() {
        let ty = rustdoc_types::Type::Pat {
            type_: Box::new(simple("NonZero")),
            __pat_unstable_do_not_use: "1..".to_string(),
        };
        assert_eq!(format_type(&ty), "NonZero");
    }

    #[test]
    fn format_type_renders_qualified_path_for_schema_export() {
        let ty = rustdoc_types::Type::QualifiedPath {
            name: "Item".to_string(),
            self_type: Box::new(rustdoc_types::Type::Generic("T".to_string())),
            trait_: Some(rustdoc_types::Path {
                path: "core::iter::Iterator".to_string(),
                id: rustdoc_types::Id(0),
                args: None,
            }),
            args: None,
        };
        assert_eq!(format_type(&ty), "<T as Iterator>::Item");
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
        let self_ty = rustdoc_types::Type::BorrowedRef {
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
        let self_ty = rustdoc_types::Type::BorrowedRef {
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
        let params = extract_params(&sig).unwrap();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name.as_str(), "id");
        assert_eq!(params[0].ty.as_str(), "UserId");
        assert_eq!(params[1].name.as_str(), "name");
        assert_eq!(params[1].ty.as_str(), "String");
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
            for_: rustdoc_types::Type::Primitive("()".to_string()),
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

    /// T006b: build_trait_origins maps local trait def_path → crate_name.
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
        // Key is def_path "my_crate::MyTrait", not the short name "MyTrait".
        assert_eq!(origins.get("my_crate::MyTrait"), Some(&"my_crate".to_string()));
    }

    /// T006b: build_trait_origins maps external trait def_path → external crate name.
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
        // Key is def_path "std::fmt::Display", not the short name "Display".
        assert_eq!(origins.get("std::fmt::Display"), Some(&"std".to_string()));
    }

    // --- T001 P0 fix (follow-up): extract_enum_variants fail-closed test ---

    /// Verify `extract_enum_variants` returns `Err` (fail-closed) when an
    /// entry in `krate.index` that is referenced as a variant id resolves to
    /// a non-`ItemEnum::Variant` item (e.g. `ItemEnum::Impl`).
    ///
    /// The fix added an `else { return Err(...) }` branch to the match in
    /// `extract_enum_variants`. This test proves the branch is in place and
    /// fires before silently pushing garbage into the `out` vec.
    #[test]
    fn test_extract_enum_variants_fail_closed_on_non_variant_item() {
        let variant_id = rustdoc_types::Id(1);

        // Build a non-Variant item (an Impl) and override its id and name so
        // `extract_enum_variants` can reach the inner-kind check rather than
        // failing earlier on a missing name.
        let (_impl_id, mut non_variant_item) = make_impl_item(1, "some::Trait", 99);
        non_variant_item.id = variant_id;
        non_variant_item.name = Some("FakeVariantName".to_owned());

        let mut index = std::collections::HashMap::new();
        index.insert(variant_id, non_variant_item);

        let krate = minimal_krate(
            index,
            std::collections::HashMap::new(),
            std::collections::HashMap::new(),
        );

        let fake_enum = rustdoc_types::Enum {
            generics: rustdoc_types::Generics { params: vec![], where_predicates: vec![] },
            has_stripped_variants: false,
            variants: vec![variant_id],
            impls: vec![],
        };

        let result = extract_enum_variants(&fake_enum, &krate);
        assert!(
            result.is_err(),
            "extract_enum_variants must fail-closed when a variant id resolves to a non-Variant item"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("non-Variant"),
            "error message must mention 'non-Variant'; got: {msg}"
        );
    }

    /// T006b: when two traits share the same short name both get separate entries keyed
    /// by their distinct def_paths — no aliasing, no one-wins-all collapse.
    #[test]
    fn test_build_trait_origins_distinct_def_paths_preserved_for_same_short_name() {
        // local::Display (crate_id 0) and std::fmt::Display (crate_id 1) share "Display"
        // as a short name. In the old short-name-keyed map one entry overwrote the other;
        // now both are preserved under their distinct def_paths.
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
        // Both traits are independently reachable by their def_path.
        assert_eq!(origins.get("local::Display"), Some(&"my_crate".to_string()));
        assert_eq!(origins.get("std::fmt::Display"), Some(&"std".to_string()));
        // The old short-name key is gone — no ambiguous "Display" entry.
        assert!(!origins.contains_key("Display"));
    }
}
