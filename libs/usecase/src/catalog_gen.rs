//! Usecase-layer contract for the `sotp catalog` generation surface.
//!
//! Holds the command / query / report DTOs, the [`CatalogError`] failure
//! vocabulary, the driving [`CatalogService`] port and the secondary
//! [`CatalogPort`], plus the [`CatalogInteractor`] that implements the former
//! by delegating to the latter.
//!
//! Design: ADR 2026-07-02-1345 (D2–D8). See spec IN-01–IN-06, IN-10, IN-12,
//! AC-01–AC-04, AC-06, AC-10, AC-11.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::plan_ref::SpecElementId;
use domain::tddd::LayerId;
// Re-export the domain catalog types that appear in this module's public
// command / report / error API so the `cli_driver` primary adapter can name
// them without depending on `domain` directly (same boundary-re-export pattern
// as `usecase::LayerId`; architecture-rules.json: cli_driver may_depend_on
// [usecase]).
pub use domain::tddd::catalog_gen::{
    CatalogEntryKind, CatalogEntryName, CatalogImportAction, DraftHole,
};
use domain::tddd::catalogue_linter::FreeText;

use crate::dry_driver_shared::IoFailureDetail;

// ── Check verdict vocabulary ───────────────────────────────────────────────

/// The three catalogue check verdicts.
///
/// See IN-06, AC-11.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogCheckVerdict {
    /// The catalogue passed completion checks.
    Pass,
    /// The catalogue is blocked by unresolved holes or invalid input.
    Blocked,
    /// The check was skipped because catalogue generation has not started.
    Skipped,
}

// ── Command / query DTOs ───────────────────────────────────────────────────

/// Command carrying the `sotp catalog add` fields.
///
/// See IN-03, IN-08, IN-09, AC-03, AC-07.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogAddCommand {
    /// Target catalogue layer.
    pub layer: LayerId,
    /// Entry kind (struct / enum / type alias / trait / function).
    pub kind: CatalogEntryKind,
    /// Entry name.
    pub name: String,
    /// Role vocabulary value for the entry.
    pub role: String,
    /// Spec anchors to attach.
    pub anchors: Vec<String>,
    /// Field declaration fragments.
    pub fields: Vec<String>,
    /// Method signature fragments.
    pub methods: Vec<String>,
    /// Variant declaration fragments.
    pub variants: Vec<String>,
    /// Trait-impl fragments.
    pub trait_impls: Vec<String>,
    /// Inherent-impl method signature fragments.
    pub inherent_methods: Vec<String>,
    /// Declaration-level generic parameters.
    pub generics: Vec<String>,
    /// Declaration-level where predicates.
    pub where_predicates: Vec<String>,
    /// Impl-block-level generic parameters.
    pub impl_generics: Vec<String>,
    /// Impl-block-level where predicates.
    pub impl_where_predicates: Vec<String>,
    /// Inherent impl-block-level generic parameters.
    pub inherent_impl_generics: Vec<String>,
    /// Inherent impl-block-level where predicates.
    pub inherent_impl_where_predicates: Vec<String>,
}

/// Command carrying the `sotp catalog import` fields.
///
/// See IN-04, AC-04.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogImportCommand {
    /// Target catalogue layer.
    pub layer: LayerId,
    /// Rust path of the type to import.
    pub type_path: String,
    /// Import action (reference / modify / delete).
    pub action: CatalogImportAction,
    /// Spec anchors to attach.
    pub anchors: Vec<String>,
}

/// Command carrying the `sotp catalog cite` fields.
///
/// See IN-05, AC-06.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogCiteCommand {
    /// Target catalogue layer.
    pub layer: LayerId,
    /// Entry to attach anchors to.
    pub entry: String,
    /// Spec anchors to attach.
    pub anchors: Vec<String>,
}

/// Query carrying the `sotp catalog check` inputs.
///
/// See IN-06, AC-11.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogCheckQuery {
    /// Target layer; `None` means all TDDD layers.
    pub layer: Option<LayerId>,
}

// ── Report DTOs ────────────────────────────────────────────────────────────

/// Report of the files created by `sotp catalog init`.
///
/// See IN-02, AC-02.
#[derive(Debug, Clone)]
pub struct CatalogInitReport {
    /// Paths of the created catalogue files.
    pub created_files: Vec<String>,
}

/// Report of a catalogue write (`add` / `import` / `cite`).
///
/// See IN-03, IN-07, AC-03, AC-05.
#[derive(Debug, Clone)]
pub struct CatalogWriteReport {
    /// Path of the written catalogue file.
    pub file_path: String,
    /// Key of the written entry.
    pub entry_key: String,
    /// `$todo` holes present in the written entry.
    pub holes: Vec<DraftHole>,
}

/// Report of a catalogue completion check.
///
/// See IN-06, AC-11.
#[derive(Debug, Clone)]
pub struct CatalogCheckReport {
    /// Resolved verdict.
    pub verdict: CatalogCheckVerdict,
    /// Human-readable findings.
    pub findings: Vec<String>,
    /// `$todo` holes still remaining.
    pub remaining_holes: Vec<DraftHole>,
}

// ── Error vocabulary ───────────────────────────────────────────────────────

/// Failure vocabulary for the catalogue operations.
///
/// See IN-10, IN-12, AC-06, AC-08, AC-10.
#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    /// A catalogue file that must not exist already does.
    #[error("catalogue file already exists: {}", path.display())]
    FileExists {
        /// The offending file path.
        path: PathBuf,
    },
    /// A required catalogue file is missing.
    #[error("catalogue file not found: {}; run `sotp catalog init` first", path.display())]
    FileMissing {
        /// The missing file path.
        path: PathBuf,
    },
    /// An entry with the same key already exists.
    #[error("duplicate catalogue entry: {}", entry_key.as_str())]
    DuplicateEntry {
        /// The duplicated entry key.
        entry_key: CatalogEntryName,
    },
    /// A cited spec anchor does not exist.
    #[error("spec anchor not found: {anchor}")]
    AnchorNotFound {
        /// The unresolved anchor.
        anchor: SpecElementId,
    },
    /// A role value is outside the section's role vocabulary.
    #[error("invalid role: {role}")]
    InvalidRole {
        /// The rejected role text.
        role: FreeText,
    },
    /// A Rust declaration / signature fragment failed to parse.
    #[error("failed to parse Rust fragment: {message}")]
    ParseFragment {
        /// The parse-failure diagnostic.
        message: FreeText,
    },
    /// The catalogue document violates its schema.
    #[error("catalogue schema invalid: {message}")]
    SchemaInvalid {
        /// The schema-violation diagnostic.
        message: FreeText,
    },
    /// The draft catalogue still has unfilled `$todo` holes.
    #[error("draft catalogue has {} unfilled hole(s)", holes.len())]
    DraftIncomplete {
        /// The remaining holes.
        holes: Vec<DraftHole>,
    },
    /// A secondary-port (filesystem) operation failed.
    #[error("catalogue port failure: {detail}")]
    Port {
        /// The rendered I/O diagnostic.
        detail: IoFailureDetail,
    },
}

// ── Ports ──────────────────────────────────────────────────────────────────

/// Driving application-service port for catalogue operations.
///
/// See IN-01, AC-01.
pub trait CatalogService: Send + Sync {
    /// Generate the empty catalogue skeletons for the track's TDDD layers.
    ///
    /// # Errors
    ///
    /// Returns a [`CatalogError`] (`FileExists` / `FileMissing` / `Port`) on
    /// pre-existing files or filesystem failure.
    fn init(&self, track_id: &str, items_dir: &Path) -> Result<CatalogInitReport, CatalogError>;

    /// Add a new entry to a layer's catalogue.
    ///
    /// # Errors
    ///
    /// Returns a [`CatalogError`] on invalid input, duplicate entry, missing
    /// file, or filesystem failure.
    fn add(
        &self,
        track_id: &str,
        items_dir: &Path,
        command: CatalogAddCommand,
    ) -> Result<CatalogWriteReport, CatalogError>;

    /// Import an existing type into a layer's catalogue.
    ///
    /// # Errors
    ///
    /// Returns a [`CatalogError`] on invalid input, missing file, or
    /// filesystem failure.
    fn import(
        &self,
        track_id: &str,
        items_dir: &Path,
        command: CatalogImportCommand,
    ) -> Result<CatalogWriteReport, CatalogError>;

    /// Attach spec anchors to an existing entry.
    ///
    /// # Errors
    ///
    /// Returns a [`CatalogError`] on an unresolved anchor, missing file, or
    /// filesystem failure.
    fn cite(
        &self,
        track_id: &str,
        items_dir: &Path,
        command: CatalogCiteCommand,
    ) -> Result<CatalogWriteReport, CatalogError>;

    /// Check catalogue completion.
    ///
    /// # Errors
    ///
    /// Returns a [`CatalogError`] on missing file, schema violation, or
    /// filesystem failure.
    fn check(
        &self,
        track_id: &str,
        items_dir: &Path,
        query: CatalogCheckQuery,
    ) -> Result<CatalogCheckReport, CatalogError>;
}

/// Secondary port backing the catalogue operations with storage.
///
/// See IN-01, AC-01, CN-02.
pub trait CatalogPort: Send + Sync {
    /// Generate the empty catalogue skeletons for the track's TDDD layers.
    ///
    /// # Errors
    ///
    /// Returns a [`CatalogError`] (`FileExists` / `FileMissing` / `Port`) on
    /// pre-existing files or filesystem failure.
    fn init(&self, track_id: &str, items_dir: &Path) -> Result<CatalogInitReport, CatalogError>;

    /// Add a new entry to a layer's catalogue.
    ///
    /// # Errors
    ///
    /// Returns a [`CatalogError`] on invalid input, duplicate entry, missing
    /// file, or filesystem failure.
    fn add(
        &self,
        track_id: &str,
        items_dir: &Path,
        command: CatalogAddCommand,
    ) -> Result<CatalogWriteReport, CatalogError>;

    /// Import an existing type into a layer's catalogue.
    ///
    /// # Errors
    ///
    /// Returns a [`CatalogError`] on invalid input, missing file, or
    /// filesystem failure.
    fn import(
        &self,
        track_id: &str,
        items_dir: &Path,
        command: CatalogImportCommand,
    ) -> Result<CatalogWriteReport, CatalogError>;

    /// Attach spec anchors to an existing entry.
    ///
    /// # Errors
    ///
    /// Returns a [`CatalogError`] on an unresolved anchor, missing file, or
    /// filesystem failure.
    fn cite(
        &self,
        track_id: &str,
        items_dir: &Path,
        command: CatalogCiteCommand,
    ) -> Result<CatalogWriteReport, CatalogError>;

    /// Check catalogue completion.
    ///
    /// # Errors
    ///
    /// Returns a [`CatalogError`] on missing file, schema violation, or
    /// filesystem failure.
    fn check(
        &self,
        track_id: &str,
        items_dir: &Path,
        query: CatalogCheckQuery,
    ) -> Result<CatalogCheckReport, CatalogError>;
}

// ── Interactor ─────────────────────────────────────────────────────────────

/// Implements [`CatalogService`] by delegating to an injected [`CatalogPort`].
///
/// See IN-01, AC-01.
pub struct CatalogInteractor {
    port: Arc<dyn CatalogPort>,
}

impl CatalogInteractor {
    /// Construct a [`CatalogInteractor`] over the given secondary port.
    #[must_use]
    pub fn new(port: Arc<dyn CatalogPort>) -> Self {
        Self { port }
    }
}

impl CatalogService for CatalogInteractor {
    fn init(&self, track_id: &str, items_dir: &Path) -> Result<CatalogInitReport, CatalogError> {
        self.port.init(track_id, items_dir)
    }

    fn add(
        &self,
        track_id: &str,
        items_dir: &Path,
        command: CatalogAddCommand,
    ) -> Result<CatalogWriteReport, CatalogError> {
        self.port.add(track_id, items_dir, command)
    }

    fn import(
        &self,
        track_id: &str,
        items_dir: &Path,
        command: CatalogImportCommand,
    ) -> Result<CatalogWriteReport, CatalogError> {
        self.port.import(track_id, items_dir, command)
    }

    fn cite(
        &self,
        track_id: &str,
        items_dir: &Path,
        command: CatalogCiteCommand,
    ) -> Result<CatalogWriteReport, CatalogError> {
        self.port.cite(track_id, items_dir, command)
    }

    fn check(
        &self,
        track_id: &str,
        items_dir: &Path,
        query: CatalogCheckQuery,
    ) -> Result<CatalogCheckReport, CatalogError> {
        self.port.check(track_id, items_dir, query)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn sample_add_command() -> CatalogAddCommand {
        CatalogAddCommand {
            layer: LayerId::try_new("domain").unwrap(),
            kind: CatalogEntryKind::Struct,
            name: "Foo".to_owned(),
            role: "ValueObject".to_owned(),
            anchors: vec!["IN-01".to_owned()],
            fields: vec![],
            methods: vec![],
            variants: vec![],
            trait_impls: vec![],
            inherent_methods: vec![],
            generics: vec![],
            where_predicates: vec![],
            impl_generics: vec![],
            impl_where_predicates: vec![],
            inherent_impl_generics: vec![],
            inherent_impl_where_predicates: vec![],
        }
    }

    #[test]
    fn test_command_and_query_construction() {
        let add = sample_add_command();
        assert_eq!(add.kind, CatalogEntryKind::Struct);

        let import = CatalogImportCommand {
            layer: LayerId::try_new("usecase").unwrap(),
            type_path: "std::path::PathBuf".to_owned(),
            action: CatalogImportAction::Reference,
            anchors: vec![],
        };
        assert_eq!(import.action, CatalogImportAction::Reference);

        let cite = CatalogCiteCommand {
            layer: LayerId::try_new("domain").unwrap(),
            entry: "Foo".to_owned(),
            anchors: vec!["AC-01".to_owned()],
        };
        assert_eq!(cite.entry, "Foo");

        let query = CatalogCheckQuery { layer: None };
        assert!(query.layer.is_none());
    }

    #[test]
    fn test_report_construction() {
        let init = CatalogInitReport { created_files: vec!["domain-types.json".to_owned()] };
        assert_eq!(init.created_files.len(), 1);

        let write = CatalogWriteReport {
            file_path: "f".to_owned(),
            entry_key: "Foo".to_owned(),
            holes: vec![],
        };
        assert_eq!(write.entry_key, "Foo");

        let check = CatalogCheckReport {
            verdict: CatalogCheckVerdict::Pass,
            findings: vec![],
            remaining_holes: vec![],
        };
        assert_eq!(check.verdict, CatalogCheckVerdict::Pass);
    }

    #[test]
    fn test_catalog_error_display_over_all_variants() {
        let variants: Vec<CatalogError> = vec![
            CatalogError::FileExists { path: PathBuf::from("a.json") },
            CatalogError::FileMissing { path: PathBuf::from("b.json") },
            CatalogError::DuplicateEntry {
                entry_key: CatalogEntryName::try_new("Foo".to_owned()).unwrap(),
            },
            CatalogError::AnchorNotFound { anchor: SpecElementId::try_new("IN-01").unwrap() },
            CatalogError::InvalidRole { role: FreeText::new("Bogus") },
            CatalogError::ParseFragment { message: FreeText::new("bad fragment") },
            CatalogError::SchemaInvalid { message: FreeText::new("bad schema") },
            CatalogError::DraftIncomplete { holes: vec![] },
            CatalogError::Port { detail: IoFailureDetail::new("io boom") },
        ];
        for err in &variants {
            assert!(!err.to_string().is_empty());
            let _as_error: &dyn std::error::Error = err;
        }
    }

    struct StubPort;

    impl CatalogPort for StubPort {
        fn init(
            &self,
            _track_id: &str,
            _items_dir: &Path,
        ) -> Result<CatalogInitReport, CatalogError> {
            Ok(CatalogInitReport { created_files: vec!["init.json".to_owned()] })
        }

        fn add(
            &self,
            _track_id: &str,
            _items_dir: &Path,
            _command: CatalogAddCommand,
        ) -> Result<CatalogWriteReport, CatalogError> {
            Ok(CatalogWriteReport {
                file_path: "add.json".to_owned(),
                entry_key: "added".to_owned(),
                holes: vec![],
            })
        }

        fn import(
            &self,
            _track_id: &str,
            _items_dir: &Path,
            _command: CatalogImportCommand,
        ) -> Result<CatalogWriteReport, CatalogError> {
            Ok(CatalogWriteReport {
                file_path: "import.json".to_owned(),
                entry_key: "imported".to_owned(),
                holes: vec![],
            })
        }

        fn cite(
            &self,
            _track_id: &str,
            _items_dir: &Path,
            _command: CatalogCiteCommand,
        ) -> Result<CatalogWriteReport, CatalogError> {
            Ok(CatalogWriteReport {
                file_path: "cite.json".to_owned(),
                entry_key: "cited".to_owned(),
                holes: vec![],
            })
        }

        fn check(
            &self,
            _track_id: &str,
            _items_dir: &Path,
            _query: CatalogCheckQuery,
        ) -> Result<CatalogCheckReport, CatalogError> {
            Ok(CatalogCheckReport {
                verdict: CatalogCheckVerdict::Blocked,
                findings: vec!["stub".to_owned()],
                remaining_holes: vec![],
            })
        }
    }

    #[test]
    fn test_interactor_delegates_to_port() {
        let interactor = CatalogInteractor::new(Arc::new(StubPort));
        let dir = Path::new("track/items");

        assert_eq!(interactor.init("t", dir).unwrap().created_files, vec!["init.json".to_owned()]);
        assert_eq!(interactor.add("t", dir, sample_add_command()).unwrap().entry_key, "added");

        let import = CatalogImportCommand {
            layer: LayerId::try_new("domain").unwrap(),
            type_path: "Foo".to_owned(),
            action: CatalogImportAction::Modify,
            anchors: vec![],
        };
        assert_eq!(interactor.import("t", dir, import).unwrap().entry_key, "imported");

        let cite = CatalogCiteCommand {
            layer: LayerId::try_new("domain").unwrap(),
            entry: "Foo".to_owned(),
            anchors: vec![],
        };
        assert_eq!(interactor.cite("t", dir, cite).unwrap().entry_key, "cited");

        let query = CatalogCheckQuery { layer: None };
        assert_eq!(
            interactor.check("t", dir, query).unwrap().verdict,
            CatalogCheckVerdict::Blocked
        );
    }
}
