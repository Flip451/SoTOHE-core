<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# codex reviewer runtime の bootstrap 解決リンク（resolve & link）配備

## Summary

GO-01: T001, T002, T003, T004.
GO-02: T001, T002, T003.

## Tasks (4/4 resolved)

### S1 — Provisioning command boundary

> Implement the CLI, cli_driver, and usecase provisioning command path and its focused tests (IN-01, IN-02, CN-01, AC-01, AC-02).

- [x] **T001**: Add the catalogue-defined Codex runtime provisioning service, port, interactor, error type, cli_driver input/driver, CLI command/arguments, and focused command-path tests (IN-01, IN-02, CN-01, AC-01, AC-02).

### S2 — Repository-first runtime resolution for all Codex spawns

> Implement the infrastructure runtime resolver and migrate its Codex spawn-adapter consumers (IN-03, IN-04, IN-05, CN-02, CN-03, AC-03, AC-04, AC-05, AC-06).

- [x] **T002**: Add the shared infrastructure Codex runtime resolver; migrate the review, review-fix, dry-check, dry-fix, and nested capability adapters; remove production CODEX_BIN/asdf resolution; retain the test-only SOTP_CODEX_BIN seam; and add focused resolver and adapter tests (IN-03, IN-04, IN-05, CN-02, CN-03, AC-03, AC-04, AC-05, AC-06).

### S3 — Verified filesystem link provisioning

> Implement the filesystem provisioner behind its usecase port, then wire it through the composition root and add focused provisioner/composition tests (IN-01, IN-02, CN-01, AC-01, AC-02).

- [x] **T003**: Add the filesystem Codex runtime provisioner implementation for the usecase port, wire it through the catalogue-defined composition root after the command path is available, and add focused provisioner/composition tests (IN-01, IN-02, CN-01, AC-01, AC-02).

### S4 — Bootstrap configuration and consumer regression alignment

> Update bootstrap Makefile wiring, .gitignore, and the host-first consumer scaffold test (IN-01, IN-04, AC-01, AC-05).

- [x] **T004**: Update source and overlay Makefile bootstrap tasks to invoke the CLI provisioning operation; remove inline CODEX_BIN resolution; add the runtime-link directory to .gitignore; align apps/cli/tests/consumer_scaffold_host_first.rs; and add bootstrap regression coverage (IN-01, IN-04, AC-01, AC-05).
