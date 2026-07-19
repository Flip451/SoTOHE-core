<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# codex reviewer runtime の bootstrap 解決リンク（resolve & link）配備

## Summary

GO-01: T001, T002, T003, T004.
GO-02: T001, T002, T003.

## Tasks (4/4 resolved)

### S1 — Provisioning command boundary

> Implement the CLI, cli_driver, and usecase provisioning command path with focused tests (IN-01, IN-02, IN-06, CN-01, AC-01, AC-02, AC-07).

- [x] **T001**: Implement CodexRuntimeProvisionError, CodexRuntimeProjectRootDiscoveryError, CodexRuntimeProvisionPort, CodexRuntimeProjectRootDiscoveryPort, CodexRuntimeProvisionService, and CodexRuntimeProvisionInteractor; CodexRuntimeDriver and CodexRuntimeInput; and the public CLI entries CliCommand, CodexRuntimeCommand, CodexRuntimeProvisionArgs, and cli::commands::codex_runtime::execute, with focused tests (IN-01, IN-02, IN-06, CN-01, AC-01, AC-02, AC-07). (`574821fdd5711a301db5b21a0c4f6d2ecec499d3`)

### S2 — Repository-first runtime resolution for all Codex spawns

> Implement the infrastructure runtime resolver and migrate its Codex spawn-adapter consumers (IN-03, IN-04, IN-05, CN-02, CN-03, AC-03, AC-04, AC-05, AC-06).

- [x] **T002**: Add the shared infrastructure Codex runtime resolver; migrate the review, review-fix, dry-check, dry-fix, and nested capability adapters; remove production CODEX_BIN/asdf resolution; retain the test-only SOTP_CODEX_BIN seam; and add focused resolver and adapter tests (IN-03, IN-04, IN-05, CN-02, CN-03, AC-03, AC-04, AC-05, AC-06). (`574821fdd5711a301db5b21a0c4f6d2ecec499d3`)

### S3 — Verified filesystem link provisioning

> Implement the filesystem provisioner and Git-root discovery adapter behind their usecase ports, then wire both through the composition root and add focused provisioner/composition tests (IN-01, IN-02, IN-06, CN-01, AC-01, AC-02, AC-07).

- [x] **T003**: Add the filesystem Codex runtime provisioner and Git project-root discovery adapter implementations for their usecase ports, wire both through the catalogue-defined composition root after the command path is available, and add focused provisioner/composition tests (IN-01, IN-02, IN-06, CN-01, AC-01, AC-02, AC-07). (`574821fdd5711a301db5b21a0c4f6d2ecec499d3`)

### S4 — Bootstrap configuration and consumer regression alignment

> Update bootstrap Makefile wiring, .gitignore, and the host-first consumer scaffold test (IN-01, IN-04, AC-01, AC-05).

- [x] **T004**: Update source and overlay Makefile bootstrap tasks to invoke the CLI provisioning operation; remove inline CODEX_BIN resolution; add the runtime-link directory to .gitignore; align apps/cli/tests/consumer_scaffold_host_first.rs; and add bootstrap regression coverage (IN-01, IN-04, AC-01, AC-05). (`574821fdd5711a301db5b21a0c4f6d2ecec499d3`)
