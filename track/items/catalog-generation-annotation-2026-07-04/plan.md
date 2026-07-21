<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# 型カタログ作成の「生成 + 注釈」への移行 — 意図入力スキャフォールディング API

## Summary

CLI: add `sotp catalog` init/add/import/cite/check surface and dispatch (IN-01/AC-01).
Domain: add catalog_gen draft/value types and catalogue_v2 entry schema/refactor changes (IN-07/IN-09/IN-13/AC-05/AC-07/AC-14/AC-15/AC-16).
Usecase: add catalog_gen commands, reports, errors, ports, and interactor (IN-02/IN-03/IN-04/IN-05/IN-06/IN-10/IN-12/AC-02/AC-03/AC-04/AC-06/AC-10/AC-11).
Infrastructure: add draft scan/completion functions, FsCatalogAdapter helpers, verb helpers, CatalogPort impl, and schema-export JSON preservation for import metadata (IN-02/IN-03/IN-04/IN-05/IN-06/IN-08/IN-09/IN-10/IN-12/AC-02/AC-03/AC-04/AC-05/AC-06/AC-07/AC-08/AC-10/AC-11/AC-12).
CLI driver/composition: add CatalogDriver and CatalogCompositionRoot wiring (IN-01/AC-01).
Gates: update catalog-check implementation, CLI coverage, and shared active-gate invocation (IN-06/AC-11/CN-06/CN-07).
Batching: land T002+T012+T013 in one commit; review T006+T007 in one batch (CN-10/AC-17).

## Tasks (13/13 resolved)

### S1 — Domain foundation: draft value types, entry-type refactor, schema extension

> Targets T001/T002 domain catalog_gen and catalogue_v2 entry changes (IN-07/IN-09/IN-13/AC-05/AC-07/AC-14/AC-15/AC-16).

- [x] **T001**: Target libs/domain tddd::catalog_gen. Add TodoInstruction, DraftHolePath, DraftHole, CatalogEntryName, CatalogImportAction, and CatalogEntryKind with constructors/accessors/derives from domain-types.json (IN-03/IN-04/IN-07/AC-03/AC-04/AC-05/AC-08). Add unit tests for TodoInstruction::try_new, DraftHolePath::try_new, DraftHole::new, CatalogEntryName::try_new, and enum construction. (`f9b46e78`)
- [x] **T002**: Target libs/domain catalogue_v2::identifiers, catalogue_v2::entries, catalogue_linter.rs, catalogue_linter_helpers.rs, catalogue_linter_eval_primitives.rs, catalogue_v2/document.rs, and entries.rs tests. Add DocString; make TypeEntry/TraitEntry/FunctionEntry fields private; add all-fields new constructors and read accessors; retype docs to Option<DocString>; add TypeEntry generics and where_predicates; migrate domain consumers (IN-09/IN-13/CN-09/CN-10/CN-11/AC-07/AC-14/AC-15/AC-16). Batch: T002+T012+T013 same commit. Add unit tests for DocString, constructors/accessors, TypeEntry generics/where, and docs. (`82aff0c7`)

### S6 — Entry-type refactor: cross-crate consumer migration

> Targets T012/T013 cross-crate consumer migration for entry field privatization (IN-14/AC-17/CN-10).

- [x] **T012**: Target infrastructure consumers: catalogue_document_codec/*, catalogue_to_extended_crate_codec/*, signal_evaluator_v2/structural_eq.rs, baseline_graph_renderer_adapter/*, contract_map_renderer_adapter/*, type_catalogue_render.rs, type_catalogue_render/entry_details.rs, and tests. Migrate TypeEntry/TraitEntry/FunctionEntry construction/reads to T002 constructors/accessors; thread DocString and TypeEntry generics/where through codecs/renderers (IN-09/IN-14/OS-07/AC-07/AC-17). Batch: T002+T012+T013 same commit. Add/keep codec, evaluator, renderer, and round-trip tests. (`82aff0c7`)
- [x] **T013**: Target usecase consumers catalogue_lint_workflow.rs, catalogue_spec_refs.rs, catalogue_spec_signals.rs, catalogue_traversal.rs, contract_map_workflow.rs, merge_gate.rs, and pre_review_gate.rs. Migrate TypeEntry/TraitEntry/FunctionEntry construction/reads to T002 constructors/accessors with DocString handling; add DeletionRecord tombstone traversal/spec-ref/signal/per-entry-hash coverage (IN-14/AC-17). Batch: T002+T012+T013 same commit. Keep workflow/gate tests green. (`82aff0c7`)

### S2 — Usecase contract: data types, ports, and interactor

> Targets T003/T004 usecase catalog_gen data, ports, and interactor (IN-01/IN-02/IN-03/IN-04/IN-05/IN-06/IN-10/IN-12/AC-01/AC-02/AC-03/AC-04/AC-06/AC-10/AC-11).

- [x] **T003**: Target libs/usecase catalog_gen. Add CatalogCheckVerdict, CatalogAddCommand, CatalogImportCommand, CatalogCiteCommand, CatalogCheckQuery, CatalogInitReport, CatalogWriteReport, CatalogCheckReport, and CatalogError with shapes/variants from usecase-types.json; CatalogAddCommand includes raw shape fragments for trait_impls, inherent_methods, declaration-level generics/where, trait impl-level generics/where, and inherent impl-level generics/where; CatalogError variants are FileExists/FileMissing/DuplicateEntry/AnchorNotFound/InvalidRole/ParseFragment/SchemaInvalid/DraftIncomplete/Port only (IN-02/IN-03/IN-04/IN-05/IN-06/IN-10/IN-12/AC-02/AC-03/AC-04/AC-06/AC-10/AC-11). Add unit tests for CatalogError Display/Error over all variants and command/query/report construction. (`f9b46e78`)
- [x] **T004**: Target libs/usecase catalog_gen. Add CatalogService trait, CatalogPort trait, and CatalogInteractor with signatures from usecase-types.json; implement CatalogService by delegating to injected CatalogPort (IN-01/AC-01). Add unit tests with a CatalogPort double for all five methods. (`f9b46e78`)

### S3 — Infrastructure: draft layer and filesystem adapter

> Targets T005/T006/T007 infrastructure draft layer and FsCatalogAdapter implementation (IN-02/IN-03/IN-04/IN-05/IN-06/IN-07/IN-08/IN-09/IN-10/IN-12/AC-02/AC-03/AC-04/AC-05/AC-06/AC-07/AC-08/AC-10/AC-11/AC-12).

- [x] **T005**: Target libs/infrastructure tddd::catalog_gen. Add scan_todo_holes, try_complete, and CatalogDraftError with shape from infrastructure-types.json, including Codec { source: CatalogueDocumentCodecError } and From<CatalogueDocumentCodecError> (IN-07/CN-01/AC-05). Add unit tests for $todo locations, dotted paths, hole-free draft, incomplete draft, typed completion, and codec error. (`5c013f77`)
- [x] **T006**: Target libs/infrastructure tddd::catalog_gen::FsCatalogAdapter helper layer. Add FsCatalogAdapter new/Default without CatalogPort impl; add private helpers for skeleton generation, schema-field shape decomposition, anchor/role validation, role payload $todo emission, trait-impl for_type derivation, inherent-impl type_name derivation, impl-level generic/where attachment, and draft scan/completion integration (IN-03/IN-07/IN-08/IN-09/IN-10/CN-01/CN-02/CN-03/CN-05/AC-03/AC-05/AC-06/AC-07). Extend helper coverage for payload-bearing roles, TypeRef/FieldName/MethodName/ParamName/VariantName/SelfReceiver parser inputs, trait-impl for_type derivation, inherent-impl method decomposition, impl-level generic/where attachment, and unmatched impl-level flag errors. Add unit tests for helper outputs and errors. Batch review with T007. (`5c013f77`)
- [x] **T007**: Target libs/infrastructure tddd::catalog_gen::FsCatalogAdapter and schema_export_codec. Add verb helpers for init/add/import/cite/check and the complete impl usecase::catalog_gen::CatalogPort for FsCatalogAdapter; add appending of generated top-level trait_impls and inherent_impls from add; add delete-tombstone grounding support for import/cite; preserve rustdoc alias target, struct shape, and impl target module path in schema export JSON (IN-02/IN-03/IN-04/IN-05/IN-06/IN-12/CN-05/CN-06/CN-07/CN-08/AC-02/AC-03/AC-04/AC-08/AC-10/AC-11/AC-12). Extend import/cite and schema-export preservation coverage for IN-04/IN-05/AC-04/AC-06/AC-12. Add unit tests for init, add/import/cite including delete grounding, inherent impl appending, duplicate entry, missing file, schema export JSON preservation, and AC-11 check outcomes. Batch review with T006. (`5c013f77`)

### S4 — Presentation wiring: driver, composition, CLI

> Targets T008/T009/T010 cli_driver, cli_composition, and cli catalog command surface (IN-01/IN-03/IN-04/IN-05/IN-06/IN-11/AC-01/AC-03/AC-04/AC-06/AC-09/AC-11).

- [x] **T008**: Target apps/cli-driver catalog_gen. Add CatalogKindSelect, CatalogImportSelect, CatalogInitInput, CatalogAddInput, CatalogImportInput, CatalogCiteInput, CatalogCheckInput, CatalogInput, and CatalogDriver; CatalogAddInput carries raw trait/inherent impl shape flags and impl-level generics/where fields through to CatalogAddCommand; implement handle(input) -> CommandOutcome mapping to CatalogService commands/queries (IN-01/IN-03/IN-04/IN-05/IN-06/AC-01/AC-03/AC-04/AC-06/AC-11). Add unit tests with a CatalogService double. (`74edb0d8`)
- [x] **T009**: Target apps/cli-composition catalog module. Add CatalogCompositionRoot new/Default, catalog_driver(), and handle(CatalogInput) wiring FsCatalogAdapter -> CatalogInteractor -> CatalogDriver (IN-01/AC-01). Add integration test for a composition-root catalog operation. (`74edb0d8`)
- [x] **T010**: Target apps/cli commands::catalog and apps/cli/src/main.rs. Add CatalogKindArg, CatalogActionArg, CatalogInitArgs, CatalogAddArgs, CatalogImportArgs, CatalogCiteArgs, CatalogCheckArgs, CatalogCommand, execute(CatalogCommand) -> ExitCode, and top-level registration (IN-01/IN-03/IN-04/IN-05/IN-06/IN-11/OS-02/AC-01/AC-03/AC-04/AC-06/AC-09/AC-11). Add CLI integration tests for parsing, dispatch, add output, schema-field decomposition coverage, inherent-method / inherent-impl-generics writing, TypeRef add-input coverage, catalog check exit-code mapping, duplicate entry, and track-id cases. (`74edb0d8`)

### S5 — Gate enforcement

> Targets T011 catalog-check implementation, CLI coverage, and active-gate invocation (IN-06/CN-06/CN-07/AC-11).

- [x] **T011**: Target libs/infrastructure tddd::catalog_gen::verb_check and apps/cli/tests/cli_catalog.rs. Keep catalog check input limited to track and optional layer selection; verify shared active-gate catalog check wiring; add AC-11 check-outcome integration coverage for file-presence, draft, schema, anchor, and Reference-grounding cases (IN-06/CN-06/CN-07/AC-11). Leave Makefile and merge workflow files untouched. (`74edb0d8`)
