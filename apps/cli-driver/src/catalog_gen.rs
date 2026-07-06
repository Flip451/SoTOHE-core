//! `catalog` command family — primary adapter driver.
//!
//! `CatalogDriver` holds a single injected `CatalogService` and exposes
//! `handle(input) -> CommandOutcome`. One injected interactor — no per-service
//! fields (D3/D4 cli_driver policy).
//!
//! The input DTOs mirror the `sotp catalog {init,add,import,cite,check}` verbs.
//! Driver-local `*Select` enums decouple the `cli` clap surface from the
//! `usecase` / `domain` vocabulary; `handle` converts them into the use-case
//! command / query DTOs before dispatch. See spec IN-01–IN-06, AC-01–AC-11.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::LayerId;
use usecase::catalog_gen::{
    CatalogAddCommand, CatalogCheckQuery, CatalogCheckReport, CatalogCheckVerdict,
    CatalogCiteCommand, CatalogEntryKind, CatalogImportAction, CatalogImportCommand,
    CatalogInitReport, CatalogService, CatalogWriteReport,
};

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Select enums (driver-local mirrors of the domain / usecase vocabulary)
// ---------------------------------------------------------------------------

/// DTO enum of the five catalogue entry kinds. See IN-03, AC-03.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogKindSelect {
    /// A `struct` entry.
    Struct,
    /// An `enum` entry.
    Enum,
    /// A `type` alias entry.
    TypeAlias,
    /// A `trait` entry.
    Trait,
    /// A free `fn` entry.
    Function,
}

/// DTO enum of the three import actions. See IN-04, AC-04.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogImportSelect {
    /// Reference an existing type unchanged.
    Reference,
    /// Take the current shape as a baseline to modify.
    Modify,
    /// Import identity only to declare a deletion.
    Delete,
}

// ---------------------------------------------------------------------------
// Input DTOs
// ---------------------------------------------------------------------------

/// Input DTO carrying track_id and items_dir. See IN-02, AC-02.
#[derive(Debug, Clone)]
pub struct CatalogInitInput {
    /// Resolved track identifier.
    pub track_id: String,
    /// Path to the track items directory (e.g. `track/items`).
    pub items_dir: PathBuf,
}

/// Input DTO carrying the add-entry fields. See IN-03, AC-03.
#[derive(Debug, Clone)]
pub struct CatalogAddInput {
    /// Resolved track identifier.
    pub track_id: String,
    /// Path to the track items directory.
    pub items_dir: PathBuf,
    /// Target catalogue layer.
    pub layer: String,
    /// Entry kind.
    pub kind: CatalogKindSelect,
    /// Entry name.
    pub name: String,
    /// DDD role vocabulary value.
    pub role: String,
    /// Spec anchors to cite.
    pub anchors: Vec<String>,
    /// Field declaration fragments.
    pub fields: Vec<String>,
    /// Method signature fragments.
    pub methods: Vec<String>,
    /// Variant declaration fragments.
    pub variants: Vec<String>,
    /// Trait-impl declaration fragments.
    pub trait_impls: Vec<String>,
    /// Inherent-impl method signature fragments.
    pub inherent_methods: Vec<String>,
    /// Declaration-level generics.
    pub generics: Vec<String>,
    /// Declaration-level where predicates.
    pub where_predicates: Vec<String>,
    /// Impl-block-level generics.
    pub impl_generics: Vec<String>,
    /// Impl-block-level where predicates.
    pub impl_where_predicates: Vec<String>,
    /// Inherent impl-block-level generics.
    pub inherent_impl_generics: Vec<String>,
    /// Inherent impl-block-level where predicates.
    pub inherent_impl_where_predicates: Vec<String>,
}

/// Input DTO carrying layer, type_path, action, and anchors. See IN-04, AC-04.
#[derive(Debug, Clone)]
pub struct CatalogImportInput {
    /// Resolved track identifier.
    pub track_id: String,
    /// Path to the track items directory.
    pub items_dir: PathBuf,
    /// Target catalogue layer.
    pub layer: String,
    /// Rust path of the type to import (rustdoc-resolved).
    pub type_path: String,
    /// Import action.
    pub action: CatalogImportSelect,
    /// Spec anchors to cite.
    pub anchors: Vec<String>,
}

/// Input DTO carrying layer, entry, and anchors. See IN-05, AC-06.
#[derive(Debug, Clone)]
pub struct CatalogCiteInput {
    /// Resolved track identifier.
    pub track_id: String,
    /// Path to the track items directory.
    pub items_dir: PathBuf,
    /// Target catalogue layer.
    pub layer: String,
    /// Entry name to append anchors to.
    pub entry: String,
    /// Spec anchors to add.
    pub anchors: Vec<String>,
}

/// Input DTO carrying track_id, items_dir, and optional layer. See IN-06, AC-11.
#[derive(Debug, Clone)]
pub struct CatalogCheckInput {
    /// Resolved track identifier.
    pub track_id: String,
    /// Path to the track items directory.
    pub items_dir: PathBuf,
    /// Optional layer filter; `None` checks every TDDD layer.
    pub layer: Option<String>,
}

/// DTO enum of the five catalog driver inputs. See IN-01, AC-01.
// Variant shapes are fixed by the Phase-2 catalogue contract (unboxed tuple
// payloads); this DTO is built once per CLI invocation and moved straight into
// `handle`, so the inter-variant size spread has no runtime cost and boxing
// would only diverge from the contract.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum CatalogInput {
    /// `sotp catalog init`.
    Init(CatalogInitInput),
    /// `sotp catalog add`.
    Add(CatalogAddInput),
    /// `sotp catalog import`.
    Import(CatalogImportInput),
    /// `sotp catalog cite`.
    Cite(CatalogCiteInput),
    /// `sotp catalog check`.
    Check(CatalogCheckInput),
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `catalog` command family.
///
/// Holds a single injected `CatalogService`; exposes
/// `handle(input) -> CommandOutcome`. One injected interactor — no per-service
/// fields (D3/D4 cli_driver policy).
pub struct CatalogDriver {
    service: Arc<dyn CatalogService>,
}

impl CatalogDriver {
    /// Create a new `CatalogDriver` with a single injected catalog service.
    pub fn new(service: Arc<dyn CatalogService>) -> Self {
        Self { service }
    }

    /// Handle a catalog command.
    pub fn handle(&self, input: CatalogInput) -> CommandOutcome {
        match input {
            CatalogInput::Init(input) => self.catalog_init(input),
            CatalogInput::Add(input) => self.catalog_add(input),
            CatalogInput::Import(input) => self.catalog_import(input),
            CatalogInput::Cite(input) => self.catalog_cite(input),
            CatalogInput::Check(input) => self.catalog_check(input),
        }
    }

    fn catalog_init(&self, input: CatalogInitInput) -> CommandOutcome {
        let CatalogInitInput { track_id, items_dir } = input;
        match self.service.init(&track_id, &items_dir) {
            Ok(report) => CommandOutcome::success(Some(render_init_report(&report))),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn catalog_add(&self, input: CatalogAddInput) -> CommandOutcome {
        let CatalogAddInput {
            track_id,
            items_dir,
            layer,
            kind,
            name,
            role,
            anchors,
            fields,
            methods,
            variants,
            trait_impls,
            inherent_methods,
            generics,
            where_predicates,
            impl_generics,
            impl_where_predicates,
            inherent_impl_generics,
            inherent_impl_where_predicates,
        } = input;
        let layer = match LayerId::try_new(layer) {
            Ok(layer) => layer,
            Err(e) => return CommandOutcome::failure(Some(format!("invalid layer: {e}"))),
        };
        let command = CatalogAddCommand {
            layer,
            kind: kind_to_domain(kind),
            name,
            role,
            anchors,
            fields,
            methods,
            variants,
            trait_impls,
            inherent_methods,
            generics,
            where_predicates,
            impl_generics,
            impl_where_predicates,
            inherent_impl_generics,
            inherent_impl_where_predicates,
        };
        match self.service.add(&track_id, &items_dir, command) {
            Ok(report) => CommandOutcome::success(Some(render_write_report(&report))),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn catalog_import(&self, input: CatalogImportInput) -> CommandOutcome {
        let CatalogImportInput { track_id, items_dir, layer, type_path, action, anchors } = input;
        let layer = match LayerId::try_new(layer) {
            Ok(layer) => layer,
            Err(e) => return CommandOutcome::failure(Some(format!("invalid layer: {e}"))),
        };
        let command =
            CatalogImportCommand { layer, type_path, action: action_to_domain(action), anchors };
        match self.service.import(&track_id, &items_dir, command) {
            Ok(report) => CommandOutcome::success(Some(render_write_report(&report))),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn catalog_cite(&self, input: CatalogCiteInput) -> CommandOutcome {
        let CatalogCiteInput { track_id, items_dir, layer, entry, anchors } = input;
        let layer = match LayerId::try_new(layer) {
            Ok(layer) => layer,
            Err(e) => return CommandOutcome::failure(Some(format!("invalid layer: {e}"))),
        };
        let command = CatalogCiteCommand { layer, entry, anchors };
        match self.service.cite(&track_id, &items_dir, command) {
            Ok(report) => CommandOutcome::success(Some(render_write_report(&report))),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn catalog_check(&self, input: CatalogCheckInput) -> CommandOutcome {
        let CatalogCheckInput { track_id, items_dir, layer } = input;
        let layer = match layer.map(LayerId::try_new).transpose() {
            Ok(layer) => layer,
            Err(e) => return CommandOutcome::failure(Some(format!("invalid layer: {e}"))),
        };
        let query = CatalogCheckQuery { layer };
        match self.service.check(&track_id, &items_dir, query) {
            Ok(report) => render_check_report(&report),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }
}

// ---------------------------------------------------------------------------
// Select → vocabulary conversions
// ---------------------------------------------------------------------------

fn kind_to_domain(kind: CatalogKindSelect) -> CatalogEntryKind {
    match kind {
        CatalogKindSelect::Struct => CatalogEntryKind::Struct,
        CatalogKindSelect::Enum => CatalogEntryKind::Enum,
        CatalogKindSelect::TypeAlias => CatalogEntryKind::TypeAlias,
        CatalogKindSelect::Trait => CatalogEntryKind::Trait,
        CatalogKindSelect::Function => CatalogEntryKind::Function,
    }
}

fn action_to_domain(action: CatalogImportSelect) -> CatalogImportAction {
    match action {
        CatalogImportSelect::Reference => CatalogImportAction::Reference,
        CatalogImportSelect::Modify => CatalogImportAction::Modify,
        CatalogImportSelect::Delete => CatalogImportAction::Delete,
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Format a [`CatalogInitReport`] into a human-readable summary string.
fn render_init_report(report: &CatalogInitReport) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("Initialised {} catalogue file(s):", report.created_files.len()));
    for file in &report.created_files {
        lines.push(format!("  {file}"));
    }
    lines.join("\n")
}

/// Format a [`CatalogWriteReport`] into a human-readable summary that lists any
/// remaining `$todo` holes (path + fill instruction).
fn render_write_report(report: &CatalogWriteReport) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("Wrote entry `{}` to {}", report.entry_key, report.file_path));
    if report.holes.is_empty() {
        lines.push("No $todo holes remaining.".to_owned());
    } else {
        lines.push(format!("{} $todo hole(s) to fill:", report.holes.len()));
        for hole in &report.holes {
            lines.push(format!("  {} — {}", hole.path().as_str(), hole.instruction().as_str()));
        }
    }
    lines.join("\n")
}

/// Map a [`CatalogCheckReport`] to a [`CommandOutcome`].
///
/// Pass / Skipped are non-blocking (exit 0); Blocked exits non-zero.
fn render_check_report(report: &CatalogCheckReport) -> CommandOutcome {
    let mut lines: Vec<String> = Vec::new();
    for finding in &report.findings {
        lines.push(finding.clone());
    }
    if !report.remaining_holes.is_empty() {
        lines.push(format!("{} $todo hole(s) remaining:", report.remaining_holes.len()));
        for hole in &report.remaining_holes {
            lines.push(format!("  {} — {}", hole.path().as_str(), hole.instruction().as_str()));
        }
    }
    let detail = if lines.is_empty() { None } else { Some(lines.join("\n")) };
    match report.verdict {
        CatalogCheckVerdict::Pass => CommandOutcome::success(Some(
            detail.unwrap_or_else(|| "catalog check passed".to_owned()),
        )),
        CatalogCheckVerdict::Skipped => CommandOutcome::success(Some(
            detail.unwrap_or_else(|| "catalog check skipped".to_owned()),
        )),
        CatalogCheckVerdict::Blocked => CommandOutcome {
            stdout: None,
            stderr: Some(detail.unwrap_or_else(|| "catalog check blocked".to_owned())),
            exit_code: 1,
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use usecase::catalog_gen::{
        CatalogAddCommand, CatalogCheckQuery, CatalogCheckReport, CatalogCheckVerdict,
        CatalogCiteCommand, CatalogError, CatalogImportCommand, CatalogInitReport, CatalogService,
        CatalogWriteReport,
    };

    use super::{
        CatalogAddInput, CatalogCheckInput, CatalogCiteInput, CatalogDriver, CatalogImportInput,
        CatalogImportSelect, CatalogInitInput, CatalogInput, CatalogKindSelect,
    };

    /// Hand-rolled `CatalogService` double (cli_driver carries no mock deps).
    struct FakeService {
        verdict: CatalogCheckVerdict,
        fail: bool,
    }

    impl FakeService {
        fn ok(verdict: CatalogCheckVerdict) -> Self {
            Self { verdict, fail: false }
        }

        fn failing() -> Self {
            Self { verdict: CatalogCheckVerdict::Pass, fail: true }
        }
    }

    impl CatalogService for FakeService {
        fn init(
            &self,
            _track_id: &str,
            _items_dir: &Path,
        ) -> Result<CatalogInitReport, CatalogError> {
            if self.fail {
                return Err(CatalogError::FileExists { path: PathBuf::from("domain-types.json") });
            }
            Ok(CatalogInitReport { created_files: vec!["domain-types.json".to_owned()] })
        }

        fn add(
            &self,
            _track_id: &str,
            _items_dir: &Path,
            _command: CatalogAddCommand,
        ) -> Result<CatalogWriteReport, CatalogError> {
            if self.fail {
                return Err(CatalogError::FileMissing { path: PathBuf::from("domain-types.json") });
            }
            Ok(CatalogWriteReport {
                file_path: "domain-types.json".to_owned(),
                entry_key: "Foo".to_owned(),
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
                file_path: "usecase-types.json".to_owned(),
                entry_key: "Bar".to_owned(),
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
                file_path: "domain-types.json".to_owned(),
                entry_key: "Baz".to_owned(),
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
                verdict: self.verdict,
                findings: vec![],
                remaining_holes: vec![],
            })
        }
    }

    fn driver(service: FakeService) -> CatalogDriver {
        CatalogDriver::new(Arc::new(service))
    }

    fn add_input() -> CatalogAddInput {
        CatalogAddInput {
            track_id: "t".to_owned(),
            items_dir: PathBuf::from("track/items"),
            layer: "domain".to_owned(),
            kind: CatalogKindSelect::Struct,
            name: "Foo".to_owned(),
            role: "ValueObject".to_owned(),
            anchors: vec![],
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

    fn check_input() -> CatalogInput {
        CatalogInput::Check(CatalogCheckInput {
            track_id: "t".to_owned(),
            items_dir: PathBuf::from("track/items"),
            layer: None,
        })
    }

    #[test]
    fn init_success_renders_created_files() {
        let outcome = driver(FakeService::ok(CatalogCheckVerdict::Pass)).handle(
            CatalogInput::Init(CatalogInitInput {
                track_id: "t".to_owned(),
                items_dir: PathBuf::from("track/items"),
            }),
        );
        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.stdout.unwrap().contains("domain-types.json"));
    }

    #[test]
    fn init_error_maps_to_failure() {
        let outcome = driver(FakeService::failing()).handle(CatalogInput::Init(CatalogInitInput {
            track_id: "t".to_owned(),
            items_dir: PathBuf::from("track/items"),
        }));
        assert_eq!(outcome.exit_code, 1);
        assert!(outcome.stderr.unwrap().contains("already exists"));
    }

    #[test]
    fn add_success_renders_entry_key() {
        let outcome = driver(FakeService::ok(CatalogCheckVerdict::Pass))
            .handle(CatalogInput::Add(add_input()));
        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.stdout.unwrap().contains("Foo"));
    }

    #[test]
    fn add_invalid_layer_fails_closed() {
        let mut input = add_input();
        input.layer = String::new();
        let outcome =
            driver(FakeService::ok(CatalogCheckVerdict::Pass)).handle(CatalogInput::Add(input));
        assert_eq!(outcome.exit_code, 1);
    }

    #[test]
    fn add_error_maps_to_failure() {
        let outcome = driver(FakeService::failing()).handle(CatalogInput::Add(add_input()));
        assert_eq!(outcome.exit_code, 1);
        assert!(outcome.stderr.unwrap().contains("not found"));
    }

    #[test]
    fn import_success_renders_entry_key() {
        let outcome = driver(FakeService::ok(CatalogCheckVerdict::Pass)).handle(
            CatalogInput::Import(CatalogImportInput {
                track_id: "t".to_owned(),
                items_dir: PathBuf::from("track/items"),
                layer: "usecase".to_owned(),
                type_path: "usecase::Foo".to_owned(),
                action: CatalogImportSelect::Reference,
                anchors: vec![],
            }),
        );
        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.stdout.unwrap().contains("Bar"));
    }

    #[test]
    fn cite_success_exits_zero() {
        let outcome = driver(FakeService::ok(CatalogCheckVerdict::Pass)).handle(
            CatalogInput::Cite(CatalogCiteInput {
                track_id: "t".to_owned(),
                items_dir: PathBuf::from("track/items"),
                layer: "domain".to_owned(),
                entry: "Foo".to_owned(),
                anchors: vec!["GO-01".to_owned()],
            }),
        );
        assert_eq!(outcome.exit_code, 0);
    }

    #[test]
    fn check_pass_exits_zero() {
        let outcome = driver(FakeService::ok(CatalogCheckVerdict::Pass)).handle(check_input());
        assert_eq!(outcome.exit_code, 0);
    }

    #[test]
    fn check_skipped_exits_zero() {
        let outcome = driver(FakeService::ok(CatalogCheckVerdict::Skipped)).handle(check_input());
        assert_eq!(outcome.exit_code, 0);
    }

    #[test]
    fn check_blocked_exits_nonzero() {
        let outcome = driver(FakeService::ok(CatalogCheckVerdict::Blocked)).handle(check_input());
        assert_eq!(outcome.exit_code, 1);
    }
}
