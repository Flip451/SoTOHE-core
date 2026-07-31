<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Remote Strict CI Merge Gate

## Summary

GO-01 → T001, T002, T004.
GO-02 → T003.

## Tasks (0/4 resolved)

### S1 — Remote strict merge gate

> `.github/workflows/ci.yml` `check` job: `Recreate local track branch on PR merge ref`; `Run CI suite (track-aware gates)`. IN-01、CN-01、AC-01、AC-02。

- [ ] **T001**: `.github/workflows/ci.yml` の `check` job にある `Recreate local track branch on PR merge ref` と `Run CI suite (track-aware gates)` を更新し、同 job に CI run evidence matrix と remote strict-gate regression cases を追加する。IN-01、OUT-01、OUT-02、CN-01、AC-01、AC-02。

### S2 — Local early-detection path

> `Makefile.toml` `[tasks.ci-track]` / `[tasks.ci-track-local]`; `apps/cli/tests/consumer_scaffold_host_first.rs` `test_exported_ci_track_*`. IN-03、AC-04。

- [ ] **T004**: `Makefile.toml` の `[tasks.ci-track]` / `[tasks.ci-track-local]` と `apps/cli/tests/consumer_scaffold_host_first.rs` の `test_exported_ci_track_*` regression cases を更新し、local early-detection path の検証を追加する。IN-03、AC-04。

### S3 — Branch protection enforcement

> GitHub repository settings: `develop` branch-protection rule; `track/items/remote-strict-ci-merge-gate-2026-07-31/observations.md` headings `## Branch-protection configuration handoff` / `## Branch-protection evidence`. IN-02、CN-01、AC-03。

- [ ] **T002**: GitHub repository settings の `develop` branch-protection rule に対する `CI / check` required status check の repository-administrator handoff を実施し、`track/items/remote-strict-ci-merge-gate-2026-07-31/observations.md` の `## Branch-protection configuration handoff` と `## Branch-protection evidence` に rule response / failed-PR result の evidence references を追加する。IN-02、CN-01、AC-03。

### S4 — Unattended merge completion

> `.harness/workflows/track/merge.md` Step 1–2; `.claude/commands/track/merge.md` Invocation / constraints. IN-04、CN-02、AC-05。

- [ ] **T003**: `.harness/workflows/track/merge.md` の Step 1–2 と `.claude/commands/track/merge.md` の Invocation / Claude Code invocation constraints を更新し、invocation 後の confirmation branch を削除する。`apps/cli/src/commands/pr_tests.rs` の `wait_and_merge_with_*` regression cases を追加する。IN-04、OUT-03、CN-02、AC-05。
