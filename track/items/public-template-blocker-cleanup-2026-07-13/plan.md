<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# 公開テンプレート配布前の阻害要因解消

## Summary

Run T001 before T002; T003 can proceed independently once its codec callers are identified.
Run T004-T011 in layer order: verifier modules, usecase, filesystem adapter, primary adapter, CLI, then composition coverage.
Run T012, then T014, then T013 so the manifest-resolved review sees the completed shipped set; execute T015-T020 as successive bounded finding batches over one inventory.
Run T022 after T002; it is independent of T003 and precedes final enablement in T021.
T021 is final enablement and is blocked on T012, T013, and T015-T020.
Goal traceability: GO-01 → T001; GO-02 → T004/T007-T011/T021; GO-03 → T006/T012/T013/T021; GO-04 → T014; GO-05 → T002/T003/T005/T015-T022; GO-06 → T002/T003/T005/T007-T011/T015-T022; GO-07 → T006/T007-T011/T021.

## Tasks (15/22 resolved)

### S1 — Template export classification and output protection

> Run T001 before T002 because both change `FsTemplateExportAdapter`; run T022 after T002.
> Run T003 independently after locating every structured-artifact codec caller of `FilePath`.

- [x] **T001**: In `libs/infrastructure/src/template_export/mod.rs`, update `FsTemplateExportAdapter::export` and its walk helpers to classify ignorable worktree entries before manifest traversal; add focused cases in `libs/infrastructure/src/template_export/tests.rs` (spec.json#GO-01, #IN-01, #CN-01, #AC-01, #AC-02, #OS-01). (`753b3d23d32e018c5d7ac00c265cfa8c522d0cc1`)
- [x] **T002**: In `libs/infrastructure/src/template_export/mod.rs::FsTemplateExportAdapter::export` and `libs/usecase/src/template_export/mod.rs::{TemplateExportError, TemplateExportPortError}`, add exported-output machine-path validation, error propagation, and focused acceptance/rejection tests (spec.json#GO-05, #GO-06, #IN-06, #CN-03, #AC-13, #AC-14). (`753b3d23d32e018c5d7ac00c265cfa8c522d0cc1`)
- [x] **T022**: In `libs/infrastructure/src/template_export/mod.rs::FsTemplateExportAdapter` and the `usecase::template_export::TemplateExportPort` composition binding in `apps/cli-composition/src/template_export/mod.rs`, replace the adapter's ambient machine-home environment lookup with composition-root `HOME`/`USERPROFILE` resolution injected through its constructor, and add focused adapter/composition tests (spec.json#IN-06, #CN-03, #AC-13). (`753b3d23d32e018c5d7ac00c265cfa8c522d0cc1`)
- [x] **T003**: At `libs/domain/src/review_v2/types.rs::FilePath::new` and its structured-artifact codec callers, route persisted path fields through the validated value object and add codec acceptance/rejection tests (spec.json#GO-05, #GO-06, #IN-05, #IN-06, #CN-03, #CN-04, #AC-11, #AC-14). (`753b3d23d32e018c5d7ac00c265cfa8c522d0cc1`)

### S2 — Verifier implementations

> Implement the independent fixed-version-tag, tracked-machine-path, and manifest-derived template-reference verifier functions with their focused fixtures before connecting any delivery route.

- [x] **T004**: Add `libs/infrastructure/src/verify/sotp_version_tag.rs::verify`, export it from `libs/infrastructure/src/verify/mod.rs`, and cover deterministic remote-success and unavailable-tag fixtures (spec.json#GO-02, #IN-02, #CN-02, #AC-03, #OS-02). (`6a0ee5eaab20ebcdd3f0db162138af3fd808698d`)
- [x] **T005**: Add `libs/infrastructure/src/verify/machine_paths.rs::verify`, export it from `libs/infrastructure/src/verify/mod.rs`, and add Git-index fixture coverage for machine-path and retained system-path cases (spec.json#GO-05, #GO-06, #IN-06, #CN-02, #CN-03, #AC-10, #AC-12, #OS-05). (`6a0ee5eaab20ebcdd3f0db162138af3fd808698d`)
- [x] **T006**: Add `libs/infrastructure/src/verify/template_refs.rs::verify`, export it from `libs/infrastructure/src/verify/mod.rs`, and add the manifest-derived ADR/track name-key allow and violation fixtures (spec.json#GO-03, #GO-07, #IN-07, #CN-01, #CN-02, #CN-05, #AC-15, #AC-16, #AC-17, #AC-18, #AC-19, #OS-06). (`6a0ee5eaab20ebcdd3f0db162138af3fd808698d`)

### S3 — Verification routes from application to CLI

> After the verifier functions are in place, run T007 through T011 in hexagonal dependency order: usecase, filesystem adapter, primary adapter, CLI surface, then composition-level smoke coverage.

- [x] **T007**: Extend `libs/usecase/src/verify.rs::{VerifyPort::verify_catalogue_spec_refs, VerifyService::verify_catalogue_spec_refs, VerifyInteractor}` with the three verifier routes, propagate `track_id: Option<TrackId>` across the catalogue-spec-refs port/service delegation, and add interaction/error-transport tests (spec.json#GO-02, #GO-06, #GO-07, #IN-02, #IN-06, #IN-07, #IN-08, #CN-02, #AC-03, #AC-12, #AC-15, #AC-20). (`3b61b828e620c72c5ebe10f98dc0226a14ca6c67`)
- [x] **T008**: Extend `libs/infrastructure/src/verify_adapter.rs::FsVerifyAdapter` with delegations to the three verifier functions and adapter-route tests (spec.json#GO-02, #GO-06, #GO-07, #IN-02, #IN-06, #IN-07, #CN-02, #AC-03, #AC-12, #AC-15). (`3b61b828e620c72c5ebe10f98dc0226a14ca6c67`)
- [x] **T009**: Extend `apps/cli-driver/src/verify.rs::{VerifyInput::CatalogueSpecRefs, VerifyDriver::handle}` with the three verification variants, propagate `track_id: Option<TrackId>` to `VerifyService::verify_catalogue_spec_refs`, and add input-to-usecase route tests (spec.json#GO-02, #GO-06, #GO-07, #IN-02, #IN-06, #IN-07, #IN-08, #CN-02, #AC-03, #AC-12, #AC-15, #AC-20). (`3b61b828e620c72c5ebe10f98dc0226a14ca6c67`)
- [x] **T010**: Extend `apps/cli/src/commands/verify.rs::{VerifyCommand, dispatch_to_outcome}` with the three subcommands using `VerifyArgs` and `CatalogueSpecRefsArgs`, including `parse_track_id`, and add parser/dispatch tests in `apps/cli/src/commands/verify_tests.rs` (spec.json#GO-02, #GO-06, #GO-07, #IN-02, #IN-06, #IN-07, #CN-02, #AC-03, #AC-12, #AC-15, #AC-20). (`3b61b828e620c72c5ebe10f98dc0226a14ca6c67`)
- [x] **T011**: Wire the three routes in `apps/cli-composition/src/verify.rs::VerifyCompositionRoot::verify_driver` and add controlled command-smoke coverage at that composition boundary (spec.json#GO-02, #GO-06, #GO-07, #IN-02, #IN-06, #IN-07, #CN-02, #AC-03, #AC-12, #AC-15). (`3b61b828e620c72c5ebe10f98dc0226a14ca6c67`)

### S4 — Shipped-document and archive workflow cleanup

> Run T012, then T014, then T013; T013 reviews the manifest-resolved shipped set after the archive workflow and command edits (spec.json#AC-05, #AC-06).

- [x] **T012**: Rewrite the manifest-resolved `knowledge/conventions/**` shipped documents, using `knowledge/adr/README.md` for generic ADR navigation where needed, and add targeted document checks (spec.json#GO-03, #IN-03, #CN-01, #CN-05, #AC-05, #AC-06, #OS-03).
- [x] **T014**: Create `.harness/workflows/track/archive.md`, reduce `.claude/commands/track/archive.md` to its invocation/reporting bridge, and add directory-derived archive/registry coverage for `apps/cli/src/commands/track/archive.rs::execute_archive` and `libs/infrastructure/src/track/render/{snapshot.rs,registry.rs}` (spec.json#GO-04, #IN-04, #AC-07, #AC-08, #OS-04).
- [x] **T013**: Use the include/overlay file set resolved from `.harness/config/template-boundary.json`, excluding the T012 convention set, to rewrite remaining shipped documents and configuration; perform the required semantic review over that completed resolved set (spec.json#GO-03, #IN-03, #CN-01, #CN-05, #AC-05, #AC-06, #OS-03, #OS-06).

### S5 — Tracked-artifact machine-path cleanup

> T015-T020 each rerun the machine-path verifier and resolve the next bounded finding batch over one complete `git ls-files` inventory.

- [ ] **T015**: From the deterministic `infrastructure::verify::machine_paths::verify` finding order over the `git ls-files` inventory, resolve the first 450 source-line findings and retain the batch evidence (spec.json#GO-05, #IN-05, #CN-03, #AC-09, #AC-10, #AC-11, #OS-05).
- [ ] **T016**: Rerun `infrastructure::verify::machine_paths::verify` over the `git ls-files` inventory, resolve the next 450 remaining source-line findings, and retain the batch evidence (spec.json#GO-05, #IN-05, #CN-03, #AC-09, #AC-10, #AC-11, #OS-05).
- [ ] **T017**: Rerun `infrastructure::verify::machine_paths::verify` over the `git ls-files` inventory, resolve the next 450 remaining source-line findings, and retain the batch evidence (spec.json#GO-05, #IN-05, #CN-03, #AC-09, #AC-10, #AC-11, #OS-05).
- [ ] **T018**: Rerun `infrastructure::verify::machine_paths::verify` over the `git ls-files` inventory, resolve the next 450 remaining source-line findings, and retain the batch evidence (spec.json#GO-05, #IN-05, #CN-03, #AC-09, #AC-10, #AC-11, #OS-05).
- [ ] **T019**: Rerun `infrastructure::verify::machine_paths::verify` over the `git ls-files` inventory, resolve the next 450 remaining source-line findings, and retain the batch evidence (spec.json#GO-05, #IN-05, #CN-03, #AC-09, #AC-10, #AC-11, #OS-05).
- [ ] **T020**: Rerun `infrastructure::verify::machine_paths::verify` over the `git ls-files` inventory, resolve the final at-most-84 remaining source-line findings, and record the zero-findings result (spec.json#GO-05, #IN-05, #CN-03, #AC-09, #AC-10, #AC-11, #OS-05).

### S6 — Final CI and export-smoke enablement

> After T012, T013, T015-T020, and the verifier-route sections, run T021 against `Makefile.toml` CI and export-smoke tasks (spec.json#AC-03, #AC-12, #AC-13, #AC-15).

- [ ] **T021**: After T012, T013, and T015-T020, add all three verifier invocations to `Makefile.toml` CI tasks `ci-local` and `ci-container`; add only the machine-path and template-reference invocations to `template-export-smoke-local`, with gate-specific coverage (spec.json#GO-02, #GO-03, #GO-05, #GO-06, #GO-07, #IN-02, #IN-03, #IN-06, #IN-07, #CN-01, #CN-02, #CN-03, #CN-05, #AC-03, #AC-04, #AC-12, #AC-13, #AC-15).
