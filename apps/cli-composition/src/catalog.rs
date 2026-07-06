//! `catalog` command family — composition root.
//!
//! [`CatalogCompositionRoot`] wires the filesystem catalogue adapter
//! (`FsCatalogAdapter`), the use-case interactor (`CatalogInteractor`), and
//! [`cli_driver::catalog_gen::CatalogDriver`] for the
//! `sotp catalog {init,add,import,cite,check}` subcommands.
//!
//! `handle` accepts the driver input DTO already assembled by the `cli` layer
//! (the CLI performs track-id resolution before constructing it). See IN-01,
//! IN-11, AC-01, AC-09.

use std::sync::Arc;

use cli_driver::CommandOutcome;
use cli_driver::catalog_gen::{CatalogDriver, CatalogInput};

use crate::error::CompositionError;

/// Composition root for the `catalog` command family.
///
/// Wires `FsCatalogAdapter` → `CatalogInteractor` → `CatalogDriver`.
#[derive(Debug, Default)]
pub struct CatalogCompositionRoot;

impl CatalogCompositionRoot {
    /// Create a new `CatalogCompositionRoot`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Build a fully-wired [`CatalogDriver`].
    ///
    /// Wire chain: `FsCatalogAdapter` → `CatalogInteractor` → `CatalogDriver`.
    #[must_use]
    pub fn catalog_driver(&self) -> CatalogDriver {
        use infrastructure::tddd::catalog_gen::FsCatalogAdapter;
        use usecase::catalog_gen::{CatalogInteractor, CatalogPort, CatalogService};

        let adapter = Arc::new(FsCatalogAdapter::new());
        let service: Arc<dyn CatalogService> =
            Arc::new(CatalogInteractor::new(adapter as Arc<dyn CatalogPort>));
        CatalogDriver::new(service)
    }

    /// Wire and dispatch a `catalog` command through the full stack.
    ///
    /// # Errors
    ///
    /// Returns [`CompositionError`] if composition fails. Wiring is currently
    /// infallible; the signature preserves room for future wiring errors (e.g.
    /// config loading), matching the sibling composition roots.
    pub fn handle(&self, input: CatalogInput) -> Result<CommandOutcome, CompositionError> {
        Ok(self.catalog_driver().handle(input))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::{Path, PathBuf};

    use cli_driver::catalog_gen::{
        CatalogAddInput, CatalogCheckInput, CatalogInitInput, CatalogInput, CatalogKindSelect,
    };

    use super::CatalogCompositionRoot;

    /// Minimal `architecture-rules.json` with a single `tddd.enabled` layer.
    const RULES_JSON: &str = r#"{
  "version": 2,
  "layers": [
    {
      "crate": "domain",
      "path": "libs/domain",
      "may_depend_on": [],
      "deny_reason": "",
      "tddd": {
        "enabled": true,
        "catalogue_file": "domain-types.json",
        "schema_export": {"method": "rustdoc", "targets": ["domain"]}
      }
    }
  ]
}"#;

    /// Minimal valid `spec.json` (schema_version 2, no requirements). `add`
    /// requires the spec to parse; empty anchors skip anchor validation.
    const SPEC_JSON: &str = r#"{
  "schema_version": 2,
  "version": "1.0.0",
  "title": "Test spec",
  "scope": { "in_scope": [], "out_of_scope": [] }
}"#;

    /// Build a workspace fixture; returns (`ws`, `items_dir`, `track_id`).
    fn fixture() -> (tempfile::TempDir, PathBuf, String) {
        let ws = tempfile::tempdir().unwrap();
        std::fs::write(ws.path().join("architecture-rules.json"), RULES_JSON).unwrap();
        let track_id = "test-track".to_owned();
        let items_dir = ws.path().join("track").join("items");
        let track_dir = items_dir.join(&track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("spec.json"), SPEC_JSON).unwrap();
        (ws, items_dir, track_id)
    }

    fn add_input(items_dir: &Path, track_id: &str, fields: Vec<String>) -> CatalogInput {
        CatalogInput::Add(CatalogAddInput {
            track_id: track_id.to_owned(),
            items_dir: items_dir.to_path_buf(),
            layer: "domain".to_owned(),
            kind: CatalogKindSelect::Struct,
            name: "Foo".to_owned(),
            role: "ValueObject".to_owned(),
            anchors: vec![],
            fields,
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
        })
    }

    #[test]
    fn add_then_check_blocks_on_residual_holes_via_composition_root() {
        let (_ws, items_dir, track_id) = fixture();
        let root = CatalogCompositionRoot::new();

        let init = root
            .handle(CatalogInput::Init(CatalogInitInput {
                track_id: track_id.clone(),
                items_dir: items_dir.clone(),
            }))
            .unwrap();
        assert_eq!(init.exit_code, 0, "init: {init:?}");

        let add =
            root.handle(add_input(&items_dir, &track_id, vec!["count: u32".to_owned()])).unwrap();
        assert_eq!(add.exit_code, 0, "add: {add:?}");
        assert!(add.stdout.unwrap().contains("Foo"), "add stdout must name the entry");

        // Residual $todo holes block once catalogue generation has started.
        let check = root
            .handle(CatalogInput::Check(CatalogCheckInput { track_id, items_dir, layer: None }))
            .unwrap();
        assert_eq!(check.exit_code, 1, "check blocks holes: {check:?}");
    }

    #[test]
    fn check_blocks_on_residual_holes() {
        let (_ws, items_dir, track_id) = fixture();
        let root = CatalogCompositionRoot::new();

        root.handle(CatalogInput::Init(CatalogInitInput {
            track_id: track_id.clone(),
            items_dir: items_dir.clone(),
        }))
        .unwrap();
        root.handle(add_input(&items_dir, &track_id, vec![])).unwrap();

        // The check blocks while $todo holes remain (exit non-zero).
        let check = root
            .handle(CatalogInput::Check(CatalogCheckInput { track_id, items_dir, layer: None }))
            .unwrap();
        assert_eq!(check.exit_code, 1, "check blocks on holes: {check:?}");
    }
}
