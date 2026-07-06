//! `sotp catalog` subcommand — catalogue generation + annotation surface.
//!
//! Defines the clap arg surface for `init` / `add` / `import` / `cite` /
//! `check`, converts the parsed args into `cli_driver::catalog_gen` input DTOs
//! (via driver-local `*Select` enums, so `cli` never names `usecase`/`domain`
//! types), resolves the active track (WRITE fail-closed / READ override), and
//! dispatches through `CatalogCompositionRoot`. See IN-01, IN-11, AC-01, AC-09.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Subcommand, ValueEnum};
use cli_composition::CatalogCompositionRoot;
use cli_driver::catalog_gen::{
    CatalogAddInput, CatalogCheckInput, CatalogCiteInput, CatalogImportInput, CatalogImportSelect,
    CatalogInitInput, CatalogInput, CatalogKindSelect,
};

// ---------------------------------------------------------------------------
// Value enums
// ---------------------------------------------------------------------------

/// Value-enum of the five catalogue entry kinds. See IN-03, AC-03.
#[derive(Debug, Clone, PartialEq, Eq, ValueEnum)]
pub enum CatalogKindArg {
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

/// Value-enum of the three import actions. See IN-04, AC-04.
#[derive(Debug, Clone, PartialEq, Eq, ValueEnum)]
pub enum CatalogActionArg {
    /// Reference an existing type unchanged.
    Reference,
    /// Take the current shape as a baseline to modify.
    Modify,
    /// Import identity only to declare a deletion.
    Delete,
}

// ---------------------------------------------------------------------------
// Args
// ---------------------------------------------------------------------------

/// Arguments for `sotp catalog init`.
#[derive(Debug, Args)]
pub struct CatalogInitArgs {
    /// Track ID. When omitted, resolved from the current git branch.
    #[arg(long)]
    pub track_id: Option<String>,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,
}

/// Arguments for `sotp catalog add`.
#[derive(Debug, Args)]
pub struct CatalogAddArgs {
    /// Track ID. When omitted, resolved from the current git branch.
    #[arg(long)]
    pub track_id: Option<String>,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,

    /// Target catalogue layer.
    #[arg(long)]
    pub layer: String,

    /// Entry kind.
    #[arg(long, value_enum)]
    pub kind: CatalogKindArg,

    /// Entry name.
    #[arg(long)]
    pub name: String,

    /// DDD role vocabulary value.
    #[arg(long)]
    pub role: String,

    /// Spec anchor to cite (repeatable).
    #[arg(long = "anchor")]
    pub anchors: Vec<String>,

    /// Field declaration fragment (repeatable).
    #[arg(long = "field")]
    pub fields: Vec<String>,

    /// Method signature fragment (repeatable).
    #[arg(long = "method")]
    pub methods: Vec<String>,

    /// Variant declaration fragment (repeatable).
    #[arg(long = "variant")]
    pub variants: Vec<String>,

    /// Trait-impl declaration fragment (repeatable).
    #[arg(long = "trait-impl")]
    pub trait_impls: Vec<String>,

    /// Inherent-impl method signature fragment (repeatable).
    #[arg(long = "inherent-method")]
    pub inherent_methods: Vec<String>,

    /// Declaration-level generic parameter (repeatable).
    #[arg(long = "generic")]
    pub generics: Vec<String>,

    /// Declaration-level where predicate (repeatable).
    #[arg(long = "where")]
    pub where_predicates: Vec<String>,

    /// Impl-block-level generic parameter (repeatable).
    #[arg(long = "impl-generic")]
    pub impl_generics: Vec<String>,

    /// Impl-block-level where predicate (repeatable).
    #[arg(long = "impl-where")]
    pub impl_where_predicates: Vec<String>,

    /// Inherent impl-block-level generic parameter (repeatable).
    #[arg(long = "inherent-impl-generic")]
    pub inherent_impl_generics: Vec<String>,

    /// Inherent impl-block-level where predicate (repeatable).
    #[arg(long = "inherent-impl-where")]
    pub inherent_impl_where_predicates: Vec<String>,
}

/// Arguments for `sotp catalog import`.
#[derive(Debug, Args)]
pub struct CatalogImportArgs {
    /// Track ID. When omitted, resolved from the current git branch.
    #[arg(long)]
    pub track_id: Option<String>,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,

    /// Target catalogue layer.
    #[arg(long)]
    pub layer: String,

    /// Rust path of the type to import (rustdoc-resolved).
    #[arg(long = "type")]
    pub type_path: String,

    /// Import action.
    #[arg(long, value_enum, default_value = "reference")]
    pub action: CatalogActionArg,

    /// Spec anchor to cite (repeatable).
    #[arg(long = "anchor")]
    pub anchors: Vec<String>,
}

/// Arguments for `sotp catalog cite`.
#[derive(Debug, Args)]
pub struct CatalogCiteArgs {
    /// Track ID. When omitted, resolved from the current git branch.
    #[arg(long)]
    pub track_id: Option<String>,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,

    /// Target catalogue layer.
    #[arg(long)]
    pub layer: String,

    /// Entry name to append anchors to.
    #[arg(long)]
    pub entry: String,

    /// Spec anchor to add (repeatable).
    #[arg(long = "anchor")]
    pub anchors: Vec<String>,
}

/// Arguments for `sotp catalog check`.
#[derive(Debug, Args)]
pub struct CatalogCheckArgs {
    /// Track ID. When omitted, resolved from the current git branch; may be an
    /// explicit READ override for `check`.
    #[arg(long)]
    pub track_id: Option<String>,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,

    /// Optional layer filter; when omitted, every TDDD layer is checked.
    #[arg(long)]
    pub layer: Option<String>,
}

/// Subcommands for `sotp catalog`.
// Variant shapes are fixed by the Phase-2 catalogue contract (unboxed clap
// `Args` payloads); the parsed command is dispatched once and immediately
// converted, so the inter-variant size spread has no runtime cost and boxing
// would only diverge from the contract.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
pub enum CatalogCommand {
    /// Generate empty per-layer catalogue skeletons for the track.
    Init(CatalogInitArgs),
    /// Append an annotated entry skeleton for a new type.
    Add(CatalogAddArgs),
    /// Import an existing type from rustdoc (reference / modify / delete).
    Import(CatalogImportArgs),
    /// Append spec anchors to a generated entry.
    Cite(CatalogCiteArgs),
    /// Re-validate catalogue completion.
    Check(CatalogCheckArgs),
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

/// Execute `sotp catalog <subcommand>`.
pub fn execute(cmd: CatalogCommand) -> ExitCode {
    match cmd {
        CatalogCommand::Init(args) => execute_init(args),
        CatalogCommand::Add(args) => execute_add(args),
        CatalogCommand::Import(args) => execute_import(args),
        CatalogCommand::Cite(args) => execute_cite(args),
        CatalogCommand::Check(args) => execute_check(args),
    }
}

fn execute_init(args: CatalogInitArgs) -> ExitCode {
    let track_id = match resolve_for_write(args.track_id.clone(), &args.items_dir) {
        Ok(id) => id,
        Err(code) => return code,
    };
    dispatch(CatalogInput::Init(CatalogInitInput { track_id, items_dir: args.items_dir }))
}

fn execute_add(args: CatalogAddArgs) -> ExitCode {
    let track_id = match resolve_for_write(args.track_id.clone(), &args.items_dir) {
        Ok(id) => id,
        Err(code) => return code,
    };
    let CatalogAddArgs {
        track_id: _,
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
    } = args;
    dispatch(CatalogInput::Add(CatalogAddInput {
        track_id,
        items_dir,
        layer,
        kind: kind_to_select(kind),
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
    }))
}

fn execute_import(args: CatalogImportArgs) -> ExitCode {
    let track_id = match resolve_for_write(args.track_id.clone(), &args.items_dir) {
        Ok(id) => id,
        Err(code) => return code,
    };
    let CatalogImportArgs { track_id: _, items_dir, layer, type_path, action, anchors } = args;
    dispatch(CatalogInput::Import(CatalogImportInput {
        track_id,
        items_dir,
        layer,
        type_path,
        action: action_to_select(action),
        anchors,
    }))
}

fn execute_cite(args: CatalogCiteArgs) -> ExitCode {
    let track_id = match resolve_for_write(args.track_id.clone(), &args.items_dir) {
        Ok(id) => id,
        Err(code) => return code,
    };
    let CatalogCiteArgs { track_id: _, items_dir, layer, entry, anchors } = args;
    dispatch(CatalogInput::Cite(CatalogCiteInput { track_id, items_dir, layer, entry, anchors }))
}

fn execute_check(args: CatalogCheckArgs) -> ExitCode {
    let track_id = match resolve_for_read(args.track_id.clone(), &args.items_dir) {
        Ok(id) => id,
        Err(code) => return code,
    };
    let CatalogCheckArgs { track_id: _, items_dir, layer } = args;
    dispatch(CatalogInput::Check(CatalogCheckInput { track_id, items_dir, layer }))
}

/// Dispatch an assembled input through the composition root and map the outcome.
fn dispatch(input: CatalogInput) -> ExitCode {
    crate::commands::outcome_to_exit(CatalogCompositionRoot::new().handle(input))
}

/// Resolve the track for a WRITE verb (fail-closed on branch/id mismatch, D8).
fn resolve_for_write(explicit: Option<String>, items_dir: &Path) -> Result<String, ExitCode> {
    crate::commands::track::resolve_track_id_for_write(explicit, items_dir).map_err(|e| {
        eprintln!("{e}");
        ExitCode::FAILURE
    })
}

/// Resolve the track for the READ `check` verb (`--track-id` override allowed).
fn resolve_for_read(explicit: Option<String>, items_dir: &Path) -> Result<String, ExitCode> {
    crate::commands::track::resolve_track_id(explicit, items_dir).map_err(|e| {
        eprintln!("{e}");
        ExitCode::FAILURE
    })
}

fn kind_to_select(kind: CatalogKindArg) -> CatalogKindSelect {
    match kind {
        CatalogKindArg::Struct => CatalogKindSelect::Struct,
        CatalogKindArg::Enum => CatalogKindSelect::Enum,
        CatalogKindArg::TypeAlias => CatalogKindSelect::TypeAlias,
        CatalogKindArg::Trait => CatalogKindSelect::Trait,
        CatalogKindArg::Function => CatalogKindSelect::Function,
    }
}

fn action_to_select(action: CatalogActionArg) -> CatalogImportSelect {
    match action {
        CatalogActionArg::Reference => CatalogImportSelect::Reference,
        CatalogActionArg::Modify => CatalogImportSelect::Modify,
        CatalogActionArg::Delete => CatalogImportSelect::Delete,
    }
}
