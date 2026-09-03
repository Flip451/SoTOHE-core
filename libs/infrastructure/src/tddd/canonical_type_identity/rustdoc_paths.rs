//! Rustdoc-path identity and crate-root canonicalization helpers.

use std::collections::HashMap;

use domain::tddd::NewTypeGraphCodecError;
use domain::tddd::catalogue_v2::identifiers::{
    CrateName, FullyQualifiedItemPath, Identifier, ModulePath, TypeRef,
};
use domain::tddd::catalogue_v2::identity_resolution::CatalogueIdentityResolutionError;
use domain::tddd::test_obligation::ids::{DiagnosticMessage, unavailable_diagnostic_message};
use rustdoc_types::{Id, ItemKind, ItemSummary};

use super::super::type_ref_parser::STD_PRELUDE_TYPES;
use super::{DefinitionPathAuthority, SYNTHETIC_UNPLACED_CRATE_ID};

/// Rewrites one rustdoc path's local root to the catalogue package root.
pub(crate) fn canonicalize_rustdoc_root_path(
    path: &[String],
    package_name: &CrateName,
    rustdoc_root_name: Option<&CrateName>,
) -> Vec<String> {
    let Some(root) = path.first() else {
        return Vec::new();
    };
    let Some(rustdoc_root_name) = rustdoc_root_name else {
        return path.to_vec();
    };
    if root != rustdoc_root_name.as_str() || root == package_name.as_str() {
        return path.to_vec();
    }
    let mut canonical = path.to_vec();
    if let Some(first) = canonical.first_mut() {
        *first = package_name.as_str().to_owned();
    }
    canonical
}

/// Canonicalizes a rustdoc function identity through the shared root alias
/// boundary.
pub(crate) fn canonicalize_function_identity_path(
    path: &[String],
    package_name: Option<&CrateName>,
    rustdoc_root_name: Option<&CrateName>,
) -> String {
    let canonical_path = match package_name {
        Some(package_name) => canonicalize_rustdoc_root_path(path, package_name, rustdoc_root_name),
        None => path.to_vec(),
    };
    canonical_path.join("::")
}

pub(super) fn canonicalize_path(
    raw_path: &str,
    source: &TypeRef,
    catalogue_crate: &CrateName,
    authority: &DefinitionPathAuthority,
    path_id: Option<Id>,
    rustdoc_paths: Option<&HashMap<Id, ItemSummary>>,
) -> Result<String, NewTypeGraphCodecError> {
    if raw_path.strip_prefix("::").unwrap_or(raw_path) == "Self" {
        return Ok(raw_path.to_owned());
    }
    let path = TypeRef::new(raw_path.to_owned())
        .map_err(|_| invalid_type_ref(source, "path must not be empty"))?;
    let identity = if let (Some(path_id), Some(rustdoc_paths)) = (path_id, rustdoc_paths) {
        match rustdoc_paths.get(&path_id) {
            Some(summary) if path_id != Id(0) => {
                let summary_identity = summary_identity(summary).ok_or_else(|| {
                    invalid_type_ref(
                        source,
                        format!(
                            "rustdoc path `{raw_path}` (id {}) has no authoritative type identity",
                            path_id.0
                        ),
                    )
                })?;
                let summary_ref = TypeRef::new(summary_identity.to_string())
                    .map_err(|_| invalid_type_ref(source, "rustdoc path identity is invalid"))?;
                match authority.resolve(&summary_ref, catalogue_crate) {
                    Ok(identity) => identity,
                    Err(CatalogueIdentityResolutionError::UnresolvedIdentifier(_)) => {
                        summary_identity
                    }
                    Err(error) => return Err(map_identity_resolution_error(error)),
                }
            }
            Some(_) | None => {
                authority.resolve(&path, catalogue_crate).map_err(map_identity_resolution_error)?
            }
        }
    } else {
        authority.resolve(&path, catalogue_crate).map_err(map_identity_resolution_error)?
    };
    let bare_name = raw_path.strip_prefix("::").unwrap_or(raw_path);
    if rustdoc_paths.is_none()
        && !bare_name.contains("::")
        && identity.crate_name() != catalogue_crate
        && !STD_PRELUDE_TYPES.contains(&bare_name)
    {
        return Err(NewTypeGraphCodecError::UnresolvedIdentifier(path));
    }
    Ok(identity.to_string())
}

fn map_identity_resolution_error(
    error: CatalogueIdentityResolutionError,
) -> NewTypeGraphCodecError {
    match error {
        CatalogueIdentityResolutionError::AmbiguousIdentifier(identifier, candidates) => {
            NewTypeGraphCodecError::AmbiguousIdentifier(identifier, candidates)
        }
        CatalogueIdentityResolutionError::UnresolvedIdentifier(type_ref) => {
            NewTypeGraphCodecError::UnresolvedIdentifier(type_ref)
        }
        CatalogueIdentityResolutionError::ClassificationFailed { location } => {
            NewTypeGraphCodecError::UnresolvedIdentifier(location)
        }
    }
}

pub(super) fn summary_identity(summary: &ItemSummary) -> Option<FullyQualifiedItemPath> {
    if !is_type_identity_kind(summary.kind) {
        return None;
    }
    let (crate_name, rest) = summary.path.split_first()?;
    let (name, module_segments) = rest.split_last()?;
    let crate_name = CrateName::new(crate_name.clone()).ok()?;
    let name = Identifier::new(name.clone()).ok()?;
    if summary.crate_id == SYNTHETIC_UNPLACED_CRATE_ID && !module_segments.is_empty() {
        return None;
    }
    let module_path = ModulePath::from_segments(module_segments.to_vec()).ok()?;
    if matches!(summary.kind, ItemKind::Trait | ItemKind::TraitAlias) {
        Some(if summary.crate_id == SYNTHETIC_UNPLACED_CRATE_ID {
            FullyQualifiedItemPath::new_unplaced_trait(crate_name, name)
        } else {
            FullyQualifiedItemPath::new_trait(crate_name, module_path, name)
        })
    } else {
        Some(if summary.crate_id == SYNTHETIC_UNPLACED_CRATE_ID {
            FullyQualifiedItemPath::new_unplaced_type(crate_name, name)
        } else {
            FullyQualifiedItemPath::new_type(crate_name, module_path, name)
        })
    }
}

fn is_type_identity_kind(kind: ItemKind) -> bool {
    matches!(
        kind,
        ItemKind::Struct
            | ItemKind::Union
            | ItemKind::Enum
            | ItemKind::TypeAlias
            | ItemKind::Trait
            | ItemKind::TraitAlias
            | ItemKind::ExternType
            | ItemKind::Primitive
    )
}

pub(super) fn invalid_type_ref(
    type_ref: impl std::fmt::Display,
    reason: impl Into<String>,
) -> NewTypeGraphCodecError {
    let mut raw_type_ref = type_ref.to_string();
    let type_ref = loop {
        match TypeRef::new(raw_type_ref.clone()) {
            Ok(type_ref) => break type_ref,
            Err(_) => raw_type_ref = "<invalid TypeRef>".to_owned(),
        }
    };
    let diagnostic = DiagnosticMessage::try_new(reason.into())
        .unwrap_or_else(|_| unavailable_diagnostic_message());
    NewTypeGraphCodecError::InvalidTypeRef(type_ref, diagnostic)
}
