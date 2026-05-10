//! JSON codec for [`CatalogueDocument`] (schema_version = 3).
//!
//! `CatalogueDocumentCodec` converts between `CatalogueDocument` domain types
//! and JSON using serde DTO structs defined here in the infrastructure layer.
//! The domain layer is serialization-free (ADR `2026-04-14-1531-domain-serde-ripout.md`).
//!
//! ## Wire format (schema_version = 3)
//!
//! ```json
//! {
//!   "schema_version": 3,
//!   "crate_name": "domain",
//!   "layer": "domain",
//!   "types": { "TypeName": { ... } },
//!   "traits": { "TraitName": { ... } },
//!   "functions": { "crate::fn_name": { ... } }
//! }
//! ```
//!
//! `CatalogueDocument.schema_version` is always `3` (the domain schema_version field
//! carries the wire format version; older v1/v2 formats are rejected by this codec).
//!
//! ## Error variants
//!
//! - `Json(serde_json::Error)` — serde deserialization failed.
//! - `Io(std::io::Error)` — file I/O failed.
//! - `UnsupportedSchemaVersion { actual, expected }` — version mismatch.
//! - `InvalidEntry { entry_name, reason }` — an entry's fields failed validation.
//! - `CrateNameMismatch { expected, actual }` — `crate_name` field vs filename stem.

use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::composite::{CompositePattern, TypeKindV2};
use domain::tddd::catalogue_v2::entries::{FunctionEntry, TraitEntry, TypeEntry};
use domain::tddd::catalogue_v2::identifiers::{FieldName, VariantName};
use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole};
use domain::tddd::catalogue_v2::variants::{FieldDecl, VariantDecl, VariantPayload};
use domain::tddd::catalogue_v2::{
    CatalogueDocument, CrateName, FunctionPath, FunctionRole, GenericArgsError, ItemAction,
    MethodDeclaration, MethodName, ModulePath, ParamDeclaration, ParamName, SelfReceiver,
    TraitImplDeclV2, TraitName, TypeName, TypeRef,
};
use serde::{Deserialize, Serialize, de};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Supported schema version
// ---------------------------------------------------------------------------

/// The schema version this codec reads and writes.
pub const SCHEMA_VERSION: u32 = 3;

// ---------------------------------------------------------------------------
// StrictMap — duplicate-key-rejecting BTreeMap deserializer
// ---------------------------------------------------------------------------

/// A thin newtype over `BTreeMap<K, V>` that rejects duplicate keys during
/// JSON deserialization instead of silently applying last-wins semantics.
///
/// Used for the `types`, `traits`, and `functions` maps in
/// [`CatalogueDocumentDto`] so that a tampered catalogue containing duplicate
/// keys fails closed rather than silently dropping entries.
struct StrictMap<K: Ord, V>(BTreeMap<K, V>);

impl<'de, K, V> Deserialize<'de> for StrictMap<K, V>
where
    K: Deserialize<'de> + Ord + std::fmt::Debug,
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct StrictMapVisitor<K, V>(std::marker::PhantomData<(K, V)>);

        impl<'de, K, V> de::Visitor<'de> for StrictMapVisitor<K, V>
        where
            K: Deserialize<'de> + Ord + std::fmt::Debug,
            V: Deserialize<'de>,
        {
            type Value = StrictMap<K, V>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a map with unique keys")
            }

            fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut map = BTreeMap::new();
                while let Some((key, value)) = access.next_entry::<K, V>()? {
                    if map.insert(key, value).is_some() {
                        return Err(de::Error::custom("duplicate key in catalogue map"));
                    }
                }
                Ok(StrictMap(map))
            }
        }

        deserializer.deserialize_map(StrictMapVisitor(std::marker::PhantomData))
    }
}

// ---------------------------------------------------------------------------
// CatalogueDocumentCodecError
// ---------------------------------------------------------------------------

/// Error type for [`CatalogueDocumentCodec`] operations.
#[derive(Debug, Error)]
pub enum CatalogueDocumentCodecError {
    /// `serde_json` failed to deserialize the JSON content.
    #[error("JSON deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    /// Filesystem I/O error (file not found, permission denied, …).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The JSON file's `schema_version` does not match [`SCHEMA_VERSION`].
    #[error(
        "unsupported catalogue schema_version: file has {actual}, codec expects {expected}. \
         Migrate the catalogue file to schema_version={expected}."
    )]
    UnsupportedSchemaVersion {
        /// Version number found in the JSON file.
        actual: u32,
        /// Version number expected by this codec.
        expected: u32,
    },

    /// An entry field failed identifier or type-reference validation.
    #[error("invalid entry '{entry_name}': {reason}")]
    InvalidEntry {
        /// The entry name (BTreeMap key) that triggered the error.
        entry_name: String,
        /// Human-readable reason for the failure.
        reason: String,
    },

    /// `crate_name` field does not match the expected filename stem.
    #[error(
        "crate_name mismatch: filename stem is '{expected}' but crate_name field is '{actual}'"
    )]
    CrateNameMismatch {
        /// The filename stem (portion before `-types.json`).
        expected: String,
        /// The `crate_name` field value found in the JSON.
        actual: String,
    },
}

// ---------------------------------------------------------------------------
// CatalogueDocumentCodec — stateless codec
// ---------------------------------------------------------------------------

/// Stateless codec that converts between [`CatalogueDocument`] and JSON.
///
/// Construct with [`CatalogueDocumentCodec::new`] or use the static methods
/// [`CatalogueDocumentCodec::decode`] and [`CatalogueDocumentCodec::encode`] directly.
#[derive(Debug, Clone, Default)]
pub struct CatalogueDocumentCodec;

impl CatalogueDocumentCodec {
    /// Creates a new codec instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Decode a JSON string into a [`CatalogueDocument`].
    ///
    /// `filename_stem` is the portion of the filename before `-types.json`
    /// (e.g. `"domain"` for `domain-types.json`). It is used to validate the
    /// `crate_name` field.
    ///
    /// # Errors
    ///
    /// Returns `CatalogueDocumentCodecError::Json` if the JSON is malformed.
    ///
    /// Returns `CatalogueDocumentCodecError::UnsupportedSchemaVersion` if
    /// the `schema_version` field is not [`SCHEMA_VERSION`].
    ///
    /// Returns `CatalogueDocumentCodecError::CrateNameMismatch` if the
    /// `crate_name` field does not match `filename_stem`.
    ///
    /// Returns `CatalogueDocumentCodecError::InvalidEntry` if any entry's
    /// fields fail validation.
    pub fn decode(
        json: &str,
        filename_stem: &str,
    ) -> Result<CatalogueDocument, CatalogueDocumentCodecError> {
        // Phase 1: check schema_version before full parse.
        let version_probe: SchemaVersionProbe = serde_json::from_str(json)?;
        if version_probe.schema_version != SCHEMA_VERSION {
            return Err(CatalogueDocumentCodecError::UnsupportedSchemaVersion {
                actual: version_probe.schema_version,
                expected: SCHEMA_VERSION,
            });
        }

        // Phase 2: full parse.
        let dto: CatalogueDocumentDto = serde_json::from_str(json)?;

        // Validate crate_name vs filename stem.
        if dto.crate_name != filename_stem {
            return Err(CatalogueDocumentCodecError::CrateNameMismatch {
                expected: filename_stem.to_owned(),
                actual: dto.crate_name,
            });
        }

        // Convert DTO → domain.
        dto_to_domain(dto)
    }

    /// Load and decode a `CatalogueDocument` from a file on disk.
    ///
    /// The `filename_stem` is derived from the file's stem portion before
    /// `-types.json` (e.g. `"domain"` for `domain-types.json`).
    ///
    /// # Errors
    ///
    /// Returns `CatalogueDocumentCodecError::Io` if the file cannot be read.
    ///
    /// See [`CatalogueDocumentCodec::decode`] for other error conditions.
    pub fn load(path: &Path) -> Result<CatalogueDocument, CatalogueDocumentCodecError> {
        let content = std::fs::read_to_string(path)?;
        let filename_stem = derive_filename_stem(path);
        Self::decode(&content, &filename_stem)
    }

    /// Encode a [`CatalogueDocument`] as a pretty-printed JSON string.
    ///
    /// # Errors
    ///
    /// Returns `CatalogueDocumentCodecError::Json` if serialization fails
    /// (this is extremely unlikely for valid domain types, but the error variant
    /// is kept for API completeness).
    pub fn encode(doc: &CatalogueDocument) -> Result<String, CatalogueDocumentCodecError> {
        let dto = domain_to_dto(doc);
        let json = serde_json::to_string_pretty(&dto)?;
        Ok(json)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the filename stem (part before `-types.json`).
///
/// For `domain-types.json`, returns `"domain"`.
/// For files not matching the pattern, returns the full file stem.
fn derive_filename_stem(path: &Path) -> String {
    let stem = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .strip_suffix("-types.json")
        .map(str::to_owned);

    stem.unwrap_or_else(|| path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_owned())
}

// ---------------------------------------------------------------------------
// Minimal version probe DTO
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SchemaVersionProbe {
    schema_version: u32,
}

// ---------------------------------------------------------------------------
// Serde DTO types (infrastructure only)
// ---------------------------------------------------------------------------

/// Top-level catalogue document wire format (schema_version = 3).
///
/// The `types`, `traits`, and `functions` fields are **required** (they must be
/// present as JSON objects even when empty). Omitting any of them is a decode
/// error, not a silent empty-map default. This prevents a truncated catalogue
/// from decoding as a valid zero-entry document.
///
/// Each map uses [`StrictMap`] so that duplicate JSON keys are rejected rather
/// than silently collapsed via last-wins semantics.
#[derive(Debug, Serialize)]
struct CatalogueDocumentDto {
    schema_version: u32,
    crate_name: String,
    layer: String,
    types: BTreeMap<String, TypeEntryDto>,
    traits: BTreeMap<String, TraitEntryDto>,
    functions: BTreeMap<String, FunctionEntryDto>,
}

/// Manual `Deserialize` for [`CatalogueDocumentDto`] that uses [`StrictMap`]
/// for the three entry maps instead of the derived `BTreeMap` deserializer.
///
/// The three entry maps (`types`, `traits`, `functions`) are required fields
/// and reject duplicate keys.
impl<'de> Deserialize<'de> for CatalogueDocumentDto {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            SchemaVersion,
            CrateName,
            Layer,
            Types,
            Traits,
            Functions,
            #[serde(other)]
            Unknown,
        }

        struct DtoVisitor;

        impl<'de> de::Visitor<'de> for DtoVisitor {
            type Value = CatalogueDocumentDto;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("CatalogueDocumentDto")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut schema_version: Option<u32> = None;
                let mut crate_name: Option<String> = None;
                let mut layer: Option<String> = None;
                let mut types: Option<StrictMap<String, TypeEntryDto>> = None;
                let mut traits: Option<StrictMap<String, TraitEntryDto>> = None;
                let mut functions: Option<StrictMap<String, FunctionEntryDto>> = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::SchemaVersion => {
                            if schema_version.is_some() {
                                return Err(de::Error::duplicate_field("schema_version"));
                            }
                            schema_version = Some(map.next_value()?);
                        }
                        Field::CrateName => {
                            if crate_name.is_some() {
                                return Err(de::Error::duplicate_field("crate_name"));
                            }
                            crate_name = Some(map.next_value()?);
                        }
                        Field::Layer => {
                            if layer.is_some() {
                                return Err(de::Error::duplicate_field("layer"));
                            }
                            layer = Some(map.next_value()?);
                        }
                        Field::Types => {
                            if types.is_some() {
                                return Err(de::Error::duplicate_field("types"));
                            }
                            types = Some(map.next_value()?);
                        }
                        Field::Traits => {
                            if traits.is_some() {
                                return Err(de::Error::duplicate_field("traits"));
                            }
                            traits = Some(map.next_value()?);
                        }
                        Field::Functions => {
                            if functions.is_some() {
                                return Err(de::Error::duplicate_field("functions"));
                            }
                            functions = Some(map.next_value()?);
                        }
                        Field::Unknown => {
                            // Consume the value so the deserializer is in a clean
                            // state, then reject the unknown field to fail closed.
                            // `#[serde(other)]` does not preserve the original key
                            // name; FIELDS lists the accepted keys so the error
                            // message is actionable.
                            let _ = map.next_value::<de::IgnoredAny>()?;
                            return Err(de::Error::unknown_field("(unrecognised field)", FIELDS));
                        }
                    }
                }

                let schema_version =
                    schema_version.ok_or_else(|| de::Error::missing_field("schema_version"))?;
                let crate_name =
                    crate_name.ok_or_else(|| de::Error::missing_field("crate_name"))?;
                let layer = layer.ok_or_else(|| de::Error::missing_field("layer"))?;
                let types = types.ok_or_else(|| de::Error::missing_field("types"))?;
                let traits = traits.ok_or_else(|| de::Error::missing_field("traits"))?;
                let functions = functions.ok_or_else(|| de::Error::missing_field("functions"))?;

                Ok(CatalogueDocumentDto {
                    schema_version,
                    crate_name,
                    layer,
                    types: types.0,
                    traits: traits.0,
                    functions: functions.0,
                })
            }
        }

        const FIELDS: &[&str] =
            &["schema_version", "crate_name", "layer", "types", "traits", "functions"];
        deserializer.deserialize_struct("CatalogueDocumentDto", FIELDS, DtoVisitor)
    }
}

// --- TypeEntry DTO ---

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TypeEntryDto {
    #[serde(default = "default_action")]
    action: String,
    role: String,
    kind: TypeKindDto,
    #[serde(default)]
    methods: Vec<MethodDeclarationDto>,
    #[serde(default)]
    trait_impls: Vec<TraitImplDto>,
    #[serde(default)]
    module_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    docs: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum TypeKindDto {
    #[serde(rename = "struct")]
    Struct {
        pattern: PatternDto,
        #[serde(default)]
        fields: Vec<FieldDeclDto>,
    },
    #[serde(rename = "enum")]
    Enum {
        #[serde(default)]
        variants: Vec<VariantDeclDto>,
    },
    #[serde(rename = "type_alias")]
    TypeAlias { target: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "pattern", deny_unknown_fields)]
enum PatternDto {
    #[serde(rename = "plain")]
    Plain,
    #[serde(rename = "typestate_state")]
    TypestateState {
        of: String,
        #[serde(default)]
        transition_methods: Vec<String>,
    },
    #[serde(rename = "newtype")]
    Newtype { inner: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FieldDeclDto {
    name: String,
    ty: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VariantDeclDto {
    name: String,
    #[serde(default)]
    payload: VariantPayloadDto,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(tag = "kind", deny_unknown_fields)]
enum VariantPayloadDto {
    #[default]
    #[serde(rename = "unit")]
    Unit,
    #[serde(rename = "tuple")]
    Tuple { fields: Vec<String> },
    #[serde(rename = "struct")]
    Struct { fields: Vec<FieldDeclDto> },
}

// --- TraitEntry DTO ---

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TraitEntryDto {
    #[serde(default = "default_action")]
    action: String,
    role: String,
    #[serde(default)]
    methods: Vec<MethodDeclarationDto>,
    #[serde(default)]
    module_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    docs: Option<String>,
}

// --- FunctionEntry DTO ---

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FunctionEntryDto {
    #[serde(default = "default_action")]
    action: String,
    role: String,
    #[serde(default)]
    params: Vec<ParamDto>,
    returns: String,
    #[serde(default)]
    is_async: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    docs: Option<String>,
}

// --- Shared sub-types ---

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MethodDeclarationDto {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    receiver: Option<String>,
    #[serde(default)]
    params: Vec<ParamDto>,
    returns: String,
    #[serde(default)]
    is_async: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    docs: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParamDto {
    name: String,
    ty: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TraitImplDto {
    trait_name: String,
    origin_crate: String,
    /// Generic argument string (e.g. `"CatalogueLoaderError"` for `From<CatalogueLoaderError>`).
    /// Optional — absent in catalogues that predate the `generic_args` field extension.
    /// Defaults to `None` for backward compatibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    generic_args: Option<String>,
}

fn default_action() -> String {
    "add".to_owned()
}

// ---------------------------------------------------------------------------
// DTO → Domain conversion
// ---------------------------------------------------------------------------

fn dto_to_domain(
    dto: CatalogueDocumentDto,
) -> Result<CatalogueDocument, CatalogueDocumentCodecError> {
    let err = |name: &str, reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: name.to_owned(),
        reason,
    };

    let crate_name = CrateName::new(&dto.crate_name)
        .map_err(|e| err(&dto.crate_name, format!("invalid crate_name: {e}")))?;

    let layer =
        LayerId::try_new(&dto.layer).map_err(|e| err(&dto.layer, format!("invalid layer: {e}")))?;

    let mut doc = CatalogueDocument::new(dto.schema_version, crate_name, layer);

    // Types
    for (type_name_str, entry_dto) in dto.types {
        let type_name = TypeName::new(&type_name_str)
            .map_err(|e| err(&type_name_str, format!("invalid type name: {e}")))?;
        let entry = type_entry_from_dto(&type_name_str, entry_dto)?;
        doc.types.insert(type_name, entry);
    }

    // Traits
    for (trait_name_str, entry_dto) in dto.traits {
        let trait_name = TraitName::new(&trait_name_str)
            .map_err(|e| err(&trait_name_str, format!("invalid trait name: {e}")))?;
        let entry = trait_entry_from_dto(&trait_name_str, entry_dto)?;
        doc.traits.insert(trait_name, entry);
    }

    // Functions
    for (fn_path_str, entry_dto) in dto.functions {
        let fn_path = FunctionPath::from_str(&fn_path_str)
            .map_err(|e| err(&fn_path_str, format!("invalid function path: {e}")))?;
        let entry = function_entry_from_dto(&fn_path_str, entry_dto)?;
        doc.functions.insert(fn_path, entry);
    }

    Ok(doc)
}

fn type_entry_from_dto(
    name: &str,
    dto: TypeEntryDto,
) -> Result<TypeEntry, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: name.to_owned(),
        reason,
    };

    let action = ItemAction::from_str(&dto.action)
        .map_err(|e| err(format!("invalid action '{}': {e}", dto.action)))?;

    let role = DataRole::from_str(&dto.role)
        .map_err(|e| err(format!("invalid data role '{}': {e}", dto.role)))?;

    let kind = type_kind_from_dto(name, dto.kind)?;

    let methods = dto
        .methods
        .into_iter()
        .map(|m| method_decl_from_dto(name, m))
        .collect::<Result<Vec<_>, _>>()?;

    let trait_impls = dto
        .trait_impls
        .into_iter()
        .map(|t| trait_impl_from_dto(name, t))
        .collect::<Result<Vec<_>, _>>()?;

    let module_path = if dto.module_path.is_empty() {
        ModulePath::root()
    } else {
        ModulePath::from_str(&dto.module_path)
            .map_err(|e| err(format!("invalid module_path '{}': {e}", dto.module_path)))?
    };

    Ok(TypeEntry { action, role, kind, methods, trait_impls, module_path, docs: dto.docs })
}

fn type_kind_from_dto(
    name: &str,
    dto: TypeKindDto,
) -> Result<TypeKindV2, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: name.to_owned(),
        reason,
    };

    match dto {
        TypeKindDto::Struct { pattern, fields } => {
            let pattern = composite_pattern_from_dto(name, pattern)?;
            let fields = fields
                .into_iter()
                .map(|f| field_decl_from_dto(name, f))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(TypeKindV2::Struct { pattern, fields })
        }
        TypeKindDto::Enum { variants } => {
            let variants = variants
                .into_iter()
                .map(|v| variant_decl_from_dto(name, v))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(TypeKindV2::Enum { variants })
        }
        TypeKindDto::TypeAlias { target } => {
            let target = TypeRef::new(target.clone())
                .map_err(|e| err(format!("invalid type_alias target '{}': {e}", target)))?;
            Ok(TypeKindV2::TypeAlias { target })
        }
    }
}

fn composite_pattern_from_dto(
    name: &str,
    dto: PatternDto,
) -> Result<CompositePattern, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: name.to_owned(),
        reason,
    };

    match dto {
        PatternDto::Plain => Ok(CompositePattern::Plain),
        PatternDto::TypestateState { of, transition_methods } => {
            let of = TypeName::new(&of)
                .map_err(|e| err(format!("invalid typestate 'of' name '{}': {e}", of)))?;
            let transition_methods = transition_methods
                .into_iter()
                .map(|m| {
                    MethodName::new(&m)
                        .map_err(|e| err(format!("invalid transition method name '{}': {e}", m)))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(CompositePattern::TypestateState { of, transition_methods })
        }
        PatternDto::Newtype { inner } => {
            let inner = TypeRef::new(inner.clone())
                .map_err(|e| err(format!("invalid newtype inner '{}': {e}", inner)))?;
            Ok(CompositePattern::Newtype { inner })
        }
    }
}

fn field_decl_from_dto(
    entry_name: &str,
    dto: FieldDeclDto,
) -> Result<FieldDecl, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: entry_name.to_owned(),
        reason,
    };

    let name = FieldName::new(&dto.name)
        .map_err(|e| err(format!("invalid field name '{}': {e}", dto.name)))?;
    let ty = TypeRef::new(dto.ty.clone())
        .map_err(|e| err(format!("invalid field type '{}': {e}", dto.ty)))?;
    Ok(FieldDecl::new(name, ty))
}

fn variant_decl_from_dto(
    entry_name: &str,
    dto: VariantDeclDto,
) -> Result<VariantDecl, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: entry_name.to_owned(),
        reason,
    };

    let name = VariantName::new(&dto.name)
        .map_err(|e| err(format!("invalid variant name '{}': {e}", dto.name)))?;
    let payload = variant_payload_from_dto(entry_name, dto.payload)?;
    Ok(VariantDecl { name, payload })
}

fn variant_payload_from_dto(
    entry_name: &str,
    dto: VariantPayloadDto,
) -> Result<VariantPayload, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: entry_name.to_owned(),
        reason,
    };

    match dto {
        VariantPayloadDto::Unit => Ok(VariantPayload::Unit),
        VariantPayloadDto::Tuple { fields } => {
            let type_refs = fields
                .into_iter()
                .map(|f| {
                    TypeRef::new(f.clone())
                        .map_err(|e| err(format!("invalid tuple field type '{}': {e}", f)))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(VariantPayload::Tuple(type_refs))
        }
        VariantPayloadDto::Struct { fields } => {
            let field_decls = fields
                .into_iter()
                .map(|f| field_decl_from_dto(entry_name, f))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(VariantPayload::Struct(field_decls))
        }
    }
}

fn method_decl_from_dto(
    entry_name: &str,
    dto: MethodDeclarationDto,
) -> Result<MethodDeclaration, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: entry_name.to_owned(),
        reason,
    };

    let name = MethodName::new(&dto.name)
        .map_err(|e| err(format!("invalid method name '{}': {e}", dto.name)))?;

    let receiver = match dto.receiver.as_deref() {
        None | Some("") => None,
        Some(r) => {
            let recv = SelfReceiver::from_str(r)
                .map_err(|e| err(format!("invalid self receiver '{}': {e}", r)))?;
            Some(recv)
        }
    };

    let params = dto
        .params
        .into_iter()
        .map(|p| param_decl_from_dto(entry_name, p))
        .collect::<Result<Vec<_>, _>>()?;

    let returns = TypeRef::new(dto.returns.clone())
        .map_err(|e| err(format!("invalid returns type '{}': {e}", dto.returns)))?;

    Ok(MethodDeclaration::new(name, receiver, params, returns, dto.is_async, dto.docs))
}

fn param_decl_from_dto(
    entry_name: &str,
    dto: ParamDto,
) -> Result<ParamDeclaration, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: entry_name.to_owned(),
        reason,
    };

    let name = ParamName::new(&dto.name)
        .map_err(|e| err(format!("invalid param name '{}': {e}", dto.name)))?;
    let ty = TypeRef::new(dto.ty.clone())
        .map_err(|e| err(format!("invalid param type '{}': {e}", dto.ty)))?;
    Ok(ParamDeclaration::new(name, ty))
}

fn trait_impl_from_dto(
    entry_name: &str,
    dto: TraitImplDto,
) -> Result<TraitImplDeclV2, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: entry_name.to_owned(),
        reason,
    };

    let trait_name = TraitName::new(&dto.trait_name)
        .map_err(|e| err(format!("invalid trait name '{}': {e}", dto.trait_name)))?;
    let origin_crate = CrateName::new(&dto.origin_crate)
        .map_err(|e| err(format!("invalid origin_crate '{}': {e}", dto.origin_crate)))?;
    match dto.generic_args {
        None => Ok(TraitImplDeclV2::new(trait_name, origin_crate)),
        Some(args) => TraitImplDeclV2::new_with_generic_args(trait_name, origin_crate, args)
            .map_err(|e: GenericArgsError| err(format!("invalid generic_args: {e}"))),
    }
}

fn trait_entry_from_dto(
    name: &str,
    dto: TraitEntryDto,
) -> Result<TraitEntry, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: name.to_owned(),
        reason,
    };

    let action = ItemAction::from_str(&dto.action)
        .map_err(|e| err(format!("invalid action '{}': {e}", dto.action)))?;

    let role = ContractRole::from_str(&dto.role)
        .map_err(|e| err(format!("invalid contract role '{}': {e}", dto.role)))?;

    let methods = dto
        .methods
        .into_iter()
        .map(|m| method_decl_from_dto(name, m))
        .collect::<Result<Vec<_>, _>>()?;

    let module_path = if dto.module_path.is_empty() {
        ModulePath::root()
    } else {
        ModulePath::from_str(&dto.module_path)
            .map_err(|e| err(format!("invalid module_path '{}': {e}", dto.module_path)))?
    };

    Ok(TraitEntry { action, role, methods, module_path, docs: dto.docs })
}

fn function_entry_from_dto(
    name: &str,
    dto: FunctionEntryDto,
) -> Result<FunctionEntry, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: name.to_owned(),
        reason,
    };

    let action = ItemAction::from_str(&dto.action)
        .map_err(|e| err(format!("invalid action '{}': {e}", dto.action)))?;

    let role = FunctionRole::from_str(&dto.role)
        .map_err(|e| err(format!("invalid function role '{}': {e}", dto.role)))?;

    let params = dto
        .params
        .into_iter()
        .map(|p| param_decl_from_dto(name, p))
        .collect::<Result<Vec<_>, _>>()?;

    let returns = TypeRef::new(dto.returns.clone())
        .map_err(|e| err(format!("invalid returns type '{}': {e}", dto.returns)))?;

    Ok(FunctionEntry { action, role, params, returns, is_async: dto.is_async, docs: dto.docs })
}

// ---------------------------------------------------------------------------
// Domain → DTO conversion (for encode)
// ---------------------------------------------------------------------------

fn domain_to_dto(doc: &CatalogueDocument) -> CatalogueDocumentDto {
    CatalogueDocumentDto {
        schema_version: doc.schema_version,
        crate_name: doc.crate_name.as_str().to_owned(),
        layer: doc.layer.as_ref().to_owned(),
        types: doc
            .types
            .iter()
            .map(|(k, v)| (k.as_str().to_owned(), type_entry_to_dto(v)))
            .collect(),
        traits: doc
            .traits
            .iter()
            .map(|(k, v)| (k.as_str().to_owned(), trait_entry_to_dto(v)))
            .collect(),
        functions: doc
            .functions
            .iter()
            .map(|(k, v)| (k.to_string(), function_entry_to_dto(v)))
            .collect(),
    }
}

fn type_entry_to_dto(entry: &TypeEntry) -> TypeEntryDto {
    TypeEntryDto {
        action: entry.action.to_string(),
        role: entry.role.to_string(),
        kind: type_kind_to_dto(&entry.kind),
        methods: entry.methods.iter().map(method_decl_to_dto).collect(),
        trait_impls: entry.trait_impls.iter().map(trait_impl_to_dto).collect(),
        module_path: entry.module_path.to_string(),
        docs: entry.docs.clone(),
    }
}

fn type_kind_to_dto(kind: &TypeKindV2) -> TypeKindDto {
    match kind {
        TypeKindV2::Struct { pattern, fields } => TypeKindDto::Struct {
            pattern: composite_pattern_to_dto(pattern),
            fields: fields.iter().map(field_decl_to_dto).collect(),
        },
        TypeKindV2::Enum { variants } => {
            TypeKindDto::Enum { variants: variants.iter().map(variant_decl_to_dto).collect() }
        }
        TypeKindV2::TypeAlias { target } => {
            TypeKindDto::TypeAlias { target: target.as_str().to_owned() }
        }
    }
}

fn composite_pattern_to_dto(pattern: &CompositePattern) -> PatternDto {
    match pattern {
        CompositePattern::Plain => PatternDto::Plain,
        CompositePattern::TypestateState { of, transition_methods } => PatternDto::TypestateState {
            of: of.as_str().to_owned(),
            transition_methods: transition_methods.iter().map(|m| m.as_str().to_owned()).collect(),
        },
        CompositePattern::Newtype { inner } => {
            PatternDto::Newtype { inner: inner.as_str().to_owned() }
        }
    }
}

fn field_decl_to_dto(f: &FieldDecl) -> FieldDeclDto {
    FieldDeclDto { name: f.name.as_str().to_owned(), ty: f.ty.as_str().to_owned() }
}

fn variant_decl_to_dto(v: &VariantDecl) -> VariantDeclDto {
    VariantDeclDto { name: v.name.as_str().to_owned(), payload: variant_payload_to_dto(&v.payload) }
}

fn variant_payload_to_dto(payload: &VariantPayload) -> VariantPayloadDto {
    match payload {
        VariantPayload::Unit => VariantPayloadDto::Unit,
        VariantPayload::Tuple(fields) => VariantPayloadDto::Tuple {
            fields: fields.iter().map(|f| f.as_str().to_owned()).collect(),
        },
        VariantPayload::Struct(fields) => {
            VariantPayloadDto::Struct { fields: fields.iter().map(field_decl_to_dto).collect() }
        }
    }
}

fn method_decl_to_dto(m: &MethodDeclaration) -> MethodDeclarationDto {
    MethodDeclarationDto {
        name: m.name.as_str().to_owned(),
        receiver: m.receiver.map(|r| r.to_string()),
        params: m.params.iter().map(param_decl_to_dto).collect(),
        returns: m.returns.as_str().to_owned(),
        is_async: m.is_async,
        docs: m.docs.clone(),
    }
}

fn param_decl_to_dto(p: &ParamDeclaration) -> ParamDto {
    ParamDto { name: p.name.as_str().to_owned(), ty: p.ty.as_str().to_owned() }
}

fn trait_impl_to_dto(t: &TraitImplDeclV2) -> TraitImplDto {
    TraitImplDto {
        trait_name: t.trait_name.as_str().to_owned(),
        origin_crate: t.origin_crate.as_str().to_owned(),
        generic_args: t.generic_args().map(str::to_owned),
    }
}

fn trait_entry_to_dto(entry: &TraitEntry) -> TraitEntryDto {
    TraitEntryDto {
        action: entry.action.to_string(),
        role: entry.role.to_string(),
        methods: entry.methods.iter().map(method_decl_to_dto).collect(),
        module_path: entry.module_path.to_string(),
        docs: entry.docs.clone(),
    }
}

fn function_entry_to_dto(entry: &FunctionEntry) -> FunctionEntryDto {
    FunctionEntryDto {
        action: entry.action.to_string(),
        role: entry.role.to_string(),
        params: entry.params.iter().map(param_decl_to_dto).collect(),
        returns: entry.returns.as_str().to_owned(),
        is_async: entry.is_async,
        docs: entry.docs.clone(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    fn minimal_v3_json(crate_name: &str, layer: &str) -> String {
        format!(
            r#"{{
  "schema_version": 3,
  "crate_name": "{crate_name}",
  "layer": "{layer}",
  "types": {{}},
  "traits": {{}},
  "functions": {{}}
}}"#
        )
    }

    #[test]
    fn test_decode_minimal_v3_json_succeeds() {
        let json = minimal_v3_json("domain", "domain");
        let doc = CatalogueDocumentCodec::decode(&json, "domain").unwrap();
        assert_eq!(doc.schema_version, 3);
        assert_eq!(doc.crate_name.as_str(), "domain");
        assert!(doc.types.is_empty());
    }

    #[test]
    fn test_decode_wrong_schema_version_returns_unsupported_schema_version() {
        let json = r#"{"schema_version": 2, "crate_name": "domain", "layer": "domain"}"#;
        let err = CatalogueDocumentCodec::decode(json, "domain").unwrap_err();
        assert!(
            matches!(
                err,
                CatalogueDocumentCodecError::UnsupportedSchemaVersion { actual: 2, expected: 3 }
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn test_decode_crate_name_mismatch_returns_error() {
        let json = minimal_v3_json("domain", "domain");
        let err = CatalogueDocumentCodec::decode(&json, "usecase").unwrap_err();
        assert!(matches!(err, CatalogueDocumentCodecError::CrateNameMismatch { .. }), "{err:?}");
    }

    #[test]
    fn test_decode_invalid_json_returns_json_error() {
        let err = CatalogueDocumentCodec::decode("{not json}", "domain").unwrap_err();
        assert!(matches!(err, CatalogueDocumentCodecError::Json(_)), "{err:?}");
    }

    #[test]
    fn test_decode_type_entry_with_struct_kind() {
        let json = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "UserId": {
      "action": "add",
      "role": "ValueObject",
      "kind": { "kind": "struct", "pattern": { "pattern": "plain" }, "fields": [] }
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        assert_eq!(doc.types.len(), 1);
        let entry = doc.types.values().next().unwrap();
        assert_eq!(entry.action, ItemAction::Add);
        assert_eq!(entry.role, DataRole::ValueObject);
    }

    #[test]
    fn test_decode_trait_entry_with_secondary_port_role() {
        let json = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "UserRepository": {
      "action": "add",
      "role": "SecondaryPort",
      "methods": []
    }
  },
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        assert_eq!(doc.traits.len(), 1);
        let entry = doc.traits.values().next().unwrap();
        assert_eq!(entry.role, ContractRole::SecondaryPort);
    }

    #[test]
    fn test_encode_decode_round_trip_preserves_data() {
        let json = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "UserId": {
      "action": "add",
      "role": "ValueObject",
      "kind": { "kind": "struct", "pattern": { "pattern": "newtype", "inner": "String" }, "fields": [] },
      "methods": [],
      "trait_impls": [],
      "module_path": ""
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn test_decode_function_entry() {
        let json = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {
    "domain::register_user": {
      "action": "add",
      "role": "FreeFunction",
      "params": [],
      "returns": "()"
    }
  }
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        assert_eq!(doc.functions.len(), 1);
    }

    #[test]
    fn test_derive_filename_stem_with_types_suffix() {
        use std::path::Path;
        let path = Path::new("/track/items/my-track/domain-types.json");
        assert_eq!(derive_filename_stem(path), "domain");
    }

    #[test]
    fn test_derive_filename_stem_without_types_suffix() {
        use std::path::Path;
        let path = Path::new("/track/items/my-track/domain.json");
        assert_eq!(derive_filename_stem(path), "domain");
    }

    #[test]
    fn test_decode_trait_impl_with_generic_args_present() {
        let json = r#"{
  "schema_version": 3,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {
    "RenderContractMapError": {
      "action": "modify",
      "role": "ErrorType",
      "kind": { "kind": "enum", "variants": [] },
      "trait_impls": [
        { "trait_name": "From", "origin_crate": "core", "generic_args": "CatalogueLoaderError" },
        { "trait_name": "From", "origin_crate": "core", "generic_args": "ContractMapWriterError" },
        { "trait_name": "Display", "origin_crate": "core" }
      ]
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "usecase").unwrap();
        let entry = doc.types.values().next().unwrap();
        assert_eq!(entry.trait_impls.len(), 3);
        assert_eq!(entry.trait_impls[0].generic_args(), Some("CatalogueLoaderError"));
        assert_eq!(entry.trait_impls[1].generic_args(), Some("ContractMapWriterError"));
        assert_eq!(entry.trait_impls[2].generic_args(), None);
    }

    #[test]
    fn test_decode_trait_impl_missing_generic_args_defaults_to_none() {
        // Catalogues that predate generic_args omit the field; default to None for compat.
        let json = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "OldErrorType": {
      "action": "add",
      "role": "ErrorType",
      "kind": { "kind": "enum", "variants": [] },
      "trait_impls": [
        { "trait_name": "From", "origin_crate": "core" }
      ]
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.types.values().next().unwrap();
        assert_eq!(entry.trait_impls.len(), 1);
        assert_eq!(entry.trait_impls[0].trait_name.as_str(), "From");
        assert_eq!(entry.trait_impls[0].generic_args(), None);
    }

    #[test]
    fn test_encode_decode_round_trip_preserves_generic_args() {
        let json = r#"{
  "schema_version": 3,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {
    "MyError": {
      "action": "add",
      "role": "ErrorType",
      "kind": { "kind": "enum", "variants": [] },
      "trait_impls": [
        { "trait_name": "From", "origin_crate": "core", "generic_args": "IoError" },
        { "trait_name": "Display", "origin_crate": "core" }
      ]
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "usecase").unwrap();
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "usecase").unwrap();
        assert_eq!(doc, doc2);
        let entry = doc2.types.values().next().unwrap();
        assert_eq!(entry.trait_impls[0].generic_args(), Some("IoError"));
        assert_eq!(entry.trait_impls[1].generic_args(), None);
    }

    #[test]
    fn test_decode_trait_impl_with_empty_generic_args_returns_invalid_entry_error() {
        // `generic_args: ""` must be rejected — empty string is not a valid type argument.
        let json = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "BadError": {
      "action": "add",
      "role": "ErrorType",
      "kind": { "kind": "enum", "variants": [] },
      "trait_impls": [
        { "trait_name": "From", "origin_crate": "core", "generic_args": "" }
      ]
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(
            matches!(result, Err(CatalogueDocumentCodecError::InvalidEntry { .. })),
            "expected InvalidEntry for empty generic_args, got: {result:?}"
        );
    }

    #[test]
    fn test_decode_trait_impl_with_angle_bracketed_generic_args_returns_invalid_entry_error() {
        // `generic_args: "<T>"` must be rejected — the caller must pass the bare type name.
        let json = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "BadError": {
      "action": "add",
      "role": "ErrorType",
      "kind": { "kind": "enum", "variants": [] },
      "trait_impls": [
        { "trait_name": "From", "origin_crate": "core", "generic_args": "<T>" }
      ]
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(
            matches!(result, Err(CatalogueDocumentCodecError::InvalidEntry { .. })),
            "expected InvalidEntry for angle-bracketed generic_args, got: {result:?}"
        );
    }
}
