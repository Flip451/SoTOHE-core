<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# sotp 開発領域と汎用テンプレートの分離境界・切り出し方式の実装

## Summary

T001: libs/domain/src/template_export + libs/domain/src/lib.rs — add boundary-manifest symbols, wiring, and tests (GO-04, IN-02, IN-12, AC-02, CN-02).
T002: libs/usecase/src/template_export + libs/usecase/src/lib.rs — add command/report/error/port/interactor symbols, wiring, and tests (IN-01, IN-02, IN-03, AC-01, AC-03, CN-02, CN-03).
T003-T004: libs/infrastructure/src/template_export + libs/infrastructure/src/lib.rs — add codec/adapter symbols, wiring, and tests (GO-01, GO-04, IN-01, IN-02, IN-03, IN-12, OS-05, OS-06, AC-01, AC-02, AC-03, CN-02, CN-03).
T005-T006: apps/cli-driver, apps/cli-composition, apps/cli — add TemplateDriver, TemplateCompositionRoot, CliCommand::Template, wiring, and tests (IN-01, AC-01).
T007: .harness/config/template-boundary.json + .harness/config/sotp-version.json (include-shipped) + overlay/Makefile.toml + overlay/knowledge/{adr,research}/* + codec tests — author boundary/bootstrap artifacts, overlay knowledge curation, manifest/schema tests, schema-application test, and scope-boundary assertions (GO-01, GO-02, GO-04, IN-02, IN-05, IN-06, IN-12, OS-01, OS-02, OS-03, OS-04, OS-06, OS-08, OS-11, AC-02, AC-03, AC-06, AC-07, CN-01, CN-02, CN-05, CN-06, CN-10).
T008-T009: overlay/ + codec/doc-link tests — author placeholder workspace and supporting overlay files (GO-02, GO-03, IN-03, IN-04, IN-10, OS-07, OS-09, AC-03, AC-04, AC-06, AC-10, CN-04, CN-05).
T010: Makefile.toml + smoke-gate tests — add template-export-smoke and ci-container wiring (GO-01, GO-03, GO-04, IN-07, OS-10, AC-05, AC-12, CN-04, CN-09, CN-11).
T011: architecture-rules.json + libs/infrastructure/src/verify/{domain_purity,domain_strings,usecase_purity}.rs + verifier tests — add arch-rules-driven verifier dispatch (GO-05, IN-08, AC-08, CN-07, CN-10, AC-12).
T012: libs/infrastructure/src/review_v2/scope_config_loader.rs::load_scope_config + domain::review_v2::ReviewScopeConfig::new + harness config/docs — implement arch-rules-derived review scope loading and aligned harness config (GO-05, IN-09, AC-09, CN-08, CN-10, AC-12).
T013: harness doc set — remove provenance references and add final grep verification (GO-05, IN-10, AC-10, AC-12).
T014: .tool-versions + .gitignore — remove Python leftover entries and add final verification (GO-05, IN-11, AC-11, AC-12).
Goal traceability: GO-01 -> T002/T004/T005/T006/T007/T010; GO-02 -> T007/T008; GO-03 -> T008/T009/T010; GO-04 -> T001/T003/T004/T007/T010; GO-05 -> T011/T012/T013/T014.

## Tasks (14/14 resolved)

### S1 — Domain: boundary manifest value objects

> libs/domain/src/template_export + libs/domain/src/lib.rs — run T001; add boundary-manifest symbols, wiring, and tests (GO-04, IN-02, IN-12, AC-02, CN-02).

- [x] **T001**: libs/domain/src/template_export/mod.rs and libs/domain/src/lib.rs — add TemplatePathPattern, TemplatePathPatternError, TemplatePathClassification, TemplatePathEntry, TemplateBoundaryManifest, TemplateBoundaryManifestError, module wiring, and unit tests for the listed symbols (GO-04, IN-02, IN-12, AC-02, CN-02). (`6bc1e74fa8efe85ad027d9ec9125893b9c59900e`)

### S2 — Usecase: interactor + ports + errors

> libs/usecase/src/template_export + libs/usecase/src/lib.rs — run T002; add command/report/error/port/interactor symbols, wiring, and tests (GO-01, IN-01, IN-02, IN-03, AC-01, AC-03, CN-02, CN-03).

- [x] **T002**: libs/usecase/src/template_export/mod.rs and libs/usecase/src/lib.rs — add TemplateExportCommand, TemplateExportReport, TemplateExportError, TemplateBoundaryManifestReadError, TemplateExportPortError, TemplateExportService, TemplateBoundaryManifestPort, TemplateExportPort, TemplateExportInteractor, module wiring, and mock-port unit tests (GO-01, IN-01, IN-02, IN-03, AC-01, AC-03, CN-02, CN-03). (`6bc1e74fa8efe85ad027d9ec9125893b9c59900e`)

### S3 — Infrastructure: codec + fs adapters

> libs/infrastructure/src/template_export + libs/infrastructure/src/lib.rs — run T003 (codec symbols and tests) then T004 (adapter symbols, wiring, tests) (GO-01, GO-04, IN-01, IN-02, IN-03, IN-12, OS-05, OS-06, AC-01, AC-02, AC-03, CN-02, CN-03).

- [x] **T003**: libs/infrastructure/src/template_export/codec.rs and libs/infrastructure/src/template_export/mod.rs — add TemplateBoundaryManifestDto, TemplatePathEntryDto, TemplatePathClassificationDto, TemplateBoundaryManifestCodecError, decode_manifest, module wiring, and codec unit tests (GO-04, IN-02, IN-12, AC-02, CN-02). (`6bc1e74fa8efe85ad027d9ec9125893b9c59900e`)
- [x] **T004**: libs/infrastructure/src/template_export/mod.rs and libs/infrastructure/src/lib.rs — add FsTemplateBoundaryManifestAdapter, FsTemplateExportAdapter, module wiring, and tempdir-backed adapter tests (GO-01, GO-04, IN-01, IN-02, IN-03, IN-12, OS-05, OS-06, AC-01, AC-02, AC-03, CN-02, CN-03). (`8bdd919b6b8181ec50b6484a9a414284a2612bc0`)

### S4 — CLI wiring: cli-driver + cli-composition + cli

> apps/cli-driver — run T005; add TemplateDriver/input symbols, wiring, and tests (GO-01, IN-01, AC-01).
> apps/cli-composition + apps/cli — run T006; add TemplateCompositionRoot/TemplateCommand/CliCommand symbols, wiring, and tests (GO-01, IN-01, AC-01).

- [x] **T005**: apps/cli-driver/src/template_export/mod.rs and apps/cli-driver/src/lib.rs — add TemplateExportInput, TemplateInput, TemplateDriver, module wiring, and TemplateDriver unit tests (GO-01, IN-01, AC-01). (`8bdd919b6b8181ec50b6484a9a414284a2612bc0`)
- [x] **T006**: apps/cli-composition/src/template_export/mod.rs, apps/cli/src/commands/template/mod.rs, and apps/cli/src/lib.rs — add TemplateCompositionRoot, TemplateCommand, TemplateExportArgs, CliCommand::Template, cli::commands::template::execute, cli::commands::template::dispatch, composition wiring, clap parse tests, and composition-root smoke test (GO-01, IN-01, AC-01). (`8bdd919b6b8181ec50b6484a9a414284a2612bc0`)

### S5 — Boundary SSoT + overlay authoring

> .harness/config/template-boundary.json + .harness/config/sotp-version.json (include-shipped) + overlay/Makefile.toml + overlay/knowledge/{adr,research}/* + codec tests — run T007; author boundary/bootstrap artifacts, overlay knowledge curation, manifest/schema tests, schema-application test, and scope-boundary assertions (GO-01, GO-02, GO-04, IN-02, IN-05, IN-06, IN-12, OS-01, OS-02, OS-03, OS-04, OS-06, OS-08, OS-11, AC-02, AC-03, AC-06, AC-07, CN-01, CN-02, CN-05, CN-06, CN-10).
> overlay/ + codec/doc-link tests — run T008 (placeholder workspace files and overlay-file-existence test) then T009 (supporting overlay files and doc-link check) (GO-02, GO-03, IN-03, IN-04, IN-10, OS-07, OS-09, AC-03, AC-04, AC-06, AC-10, CN-04, CN-05).

- [x] **T007**: .harness/config/template-boundary.json, .harness/config/sotp-version.json (include-shipped), overlay/Makefile.toml, overlay/knowledge/adr/README.md (empty index), overlay/knowledge/research/README.md, overlay/knowledge/research/version-baseline-template.md, overlay/knowledge/research/.gitignore (manifest entries for knowledge/adr + knowledge/research flipped from exclude to overlay), and libs/infrastructure/src/template_export/codec.rs tests — author boundary/bootstrap artifacts, overlay knowledge curation, manifest/schema tests, schema-application test, and scope-boundary assertions (GO-01, GO-02, GO-04, IN-02, IN-05, IN-06, IN-12, OS-01, OS-02, OS-03, OS-04, OS-06, OS-08, OS-11, AC-02, AC-03, AC-06, AC-07, CN-01, CN-02, CN-05, CN-06, CN-10). (`864456deb49d3965bff3f60f358f1f37b00aacb5`)
- [x] **T008**: overlay/Cargo.toml, overlay/deny.toml, overlay/libs/{domain,usecase,infrastructure}/Cargo.toml, overlay/libs/{domain,usecase,infrastructure}/src/lib.rs, overlay/apps/{cli-driver,cli-composition,cli}/Cargo.toml, overlay/apps/{cli-driver,cli-composition,cli}/src/{lib.rs,main.rs}, root Cargo.toml (add "overlay/*" to workspace.exclude), and libs/infrastructure/src/template_export/codec.rs tests — author placeholder workspace files and add an overlay-file-existence test (GO-02, GO-03, IN-03, IN-04, OS-07, OS-09, AC-03, AC-04, AC-06, CN-04, CN-05). (`864456deb49d3965bff3f60f358f1f37b00aacb5`)
- [x] **T009**: overlay/Dockerfile, overlay/architecture-rules.json, overlay/track/tech-stack.md, overlay/track/registry.md, overlay/track/product-*.md, and doc-links regression check — author supporting overlay files and the overlay track-doc check (GO-03, IN-03, IN-10, AC-03, AC-10, CN-04). (`864456deb49d3965bff3f60f358f1f37b00aacb5`)

### S6 — Export smoke CI gate

> Makefile.toml + smoke-gate unit tests — run T010; add template-export-smoke and ci-container wiring (GO-01, GO-03, GO-04, IN-07, OS-10, AC-05, AC-12, CN-04, CN-09, CN-11).

- [x] **T010**: Makefile.toml and smoke-gate unit tests — add template-export-smoke, register it in the ci-container chain, add smoke-gate regression and OS-10/CN-09 scope-boundary tests, and run the final cargo make ci check (GO-01, GO-03, GO-04, IN-07, OS-10, AC-05, AC-12, CN-04, CN-09, CN-11). (`864456deb49d3965bff3f60f358f1f37b00aacb5`)

### S7 — D5 genericity prerequisite fixes

> architecture-rules.json + libs/infrastructure/src/verify/{domain_purity,domain_strings,usecase_purity}.rs + verifier tests — run T011; add arch-rules-driven verifier dispatch (GO-05, IN-08, AC-08, CN-07, CN-10, AC-12).
> libs/infrastructure/src/review_v2/scope_config_loader.rs::load_scope_config + domain::review_v2::ReviewScopeConfig::new + harness config/docs — run T012; implement arch-rules-derived review scope loading and aligned harness config (GO-05, IN-09, AC-09, CN-08, CN-10, AC-12).
> harness doc set — run T013; remove provenance references and add final grep verification (GO-05, IN-10, AC-10, AC-12).
> .tool-versions + .gitignore — run T014; remove Python leftover entries and add final verification (GO-05, IN-11, AC-11, AC-12).

- [x] **T011**: architecture-rules.json, libs/infrastructure/src/verify/domain_purity.rs, libs/infrastructure/src/verify/domain_strings.rs, libs/infrastructure/src/verify/usecase_purity.rs, and verifier tests — add arch-rules-driven verifier dispatch and run the final cargo make ci check (GO-05, IN-08, AC-08, CN-07, CN-10, AC-12). (`35aac7a5f651322cab7023897a933ae1b1f114fb`)
- [x] **T012**: libs/infrastructure/src/review_v2/scope_config_loader.rs::load_scope_config, domain::review_v2::ReviewScopeConfig::new, .harness/config/review-scope.json, .harness/catalogue-lint/config.json, .harness/catalogue-lint/presets/ddd-strict.json, .harness/capabilities/dry-fix-lead.md, .harness/capabilities/review-fix-lead.md, .harness/capabilities/review-fix-lead-codex.md, and .harness/workflows/track/init.md — implement arch-rules-derived review scope loading, align catalogue-lint / capability / workflow config, and run final verification (GO-05, IN-09, AC-09, CN-08, CN-10, AC-12). (`35aac7a5f651322cab7023897a933ae1b1f114fb`)
- [x] **T013**: .harness/workflows/track/done.md, .harness/capabilities/adr-editor.md, .claude/commands/track/diagnose.md, .claude/rules/07-dev-environment.md, and .codex/config.toml — remove harness doc provenance references and add final grep verification (GO-05, IN-10, AC-10, AC-12). (`35aac7a5f651322cab7023897a933ae1b1f114fb`)
- [x] **T014**: .tool-versions and .gitignore — remove Python leftover entries and add final grep / cargo make ci verification (GO-05, IN-11, AC-11, AC-12). (`864456deb49d3965bff3f60f358f1f37b00aacb5`)
