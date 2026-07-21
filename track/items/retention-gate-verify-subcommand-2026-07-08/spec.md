<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 29, yellow: 0, red: 0 }
---

# retention gate の verify サブコマンド化

## Goal

- [GO-01] 既存の retention gate を `apps/cli/tests/retention_gate.rs` の integration test から撤去し、`sotp verify` 配下の通常の verifier として再実装する。repo tree scan 型ゲートはテストスイート内の特殊ケースではなく、infrastructure checker、usecase port/service、CLI-driver input、CLI subcommand、composition wiring を通る established verify chain で実行される状態にする。 [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1]
- [GO-02] retention gate の検査意味論を維持する。live surface の存在ベース scan、scanner I/O 問題での fail-closed、廃止識別子・廃止 path の検出、M token と readiness/blocking gate word が同一行に併存する state-expression の両順序検出、clean layout と gate word を伴わない M-token note の pass を維持する。 [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1, knowledge/adr/2026-07-08-1020-retire-todo-marker-state-and-track-docs.md#D4]
- [GO-03] retention gate を maintainer repository 限定の regression guard として root `Makefile.toml` の maintainer CI へ接続し、exported template 側には継承させない。配布除外は overlay Makefile に gate task を接続しないことで実現し、`apps/` 配下の per-file boundary-manifest exclude には依存しない。 [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D2]

## Scope

### In Scope
- [IN-01] `apps/cli/tests/retention_gate.rs` を削除し、retention gate の本体を integration test として残さない。既存 test file が担っていた scan capability は verifier 実装とその単体/境界テストに移り、whole-repo scan を cargo test の副作用として発火させない。 [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1] [tasks: T004]
- [IN-02] retention gate を `sotp verify` のサブコマンドとして実行可能にする。CLI entrypoint、CLI-driver input、usecase service/port、infrastructure checker、composition root wiring が established verify pattern と整合し、scanner が isolated helper や test-only code path に閉じない状態を要件とする。 [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1] [conv: knowledge/conventions/hexagonal-architecture.md#Layer Dependencies] [tasks: T001, T002, T003]
- [IN-03] scan 対象は現行 gate の live surface と同等にし、root/pattern set は `CLAUDE.md`, `README.md`, `.claude/rules/**`, `.claude/commands/**`, `.claude/agents/**`, `.claude/skills/**`, `.harness/workflows/**`, `.harness/config/**`, `.harness/capabilities/**`, `Makefile.toml`, `libs/**`, `apps/**`, `knowledge/conventions/**` とする。これらの存在する root/pattern を存在ベースで scan し、存在しない root は violation ではなく absence として扱う。 [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1, knowledge/adr/2026-07-08-1020-retire-todo-marker-state-and-track-docs.md#D4] [tasks: T001]
- [IN-04] retired identifier / retired path 検出を維持する。対象には `verify-tech-stack`, `verify-tech-stack-local`, `verify tech-stack`, `VerifyCommand::TechStack`, `VerifyInput::TechStack`, `verify_tech_stack`, `TECH_STACK_FILE`, `verify::tech_stack`, `pub mod tech_stack;`, `track/tech-stack.md`, `track/product.md`, `track/product-guidelines.md` と同等の現行 forbidden surface が含まれる。 [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1, knowledge/adr/2026-07-08-1020-retire-todo-marker-state-and-track-docs.md#D4] [tasks: T001]
- [IN-05] M token と readiness/blocking gate word が同一行に併存する state-expression 検出を維持する。M token が先に現れる行と gate word が先に現れる行のどちらも violation になり、gate word を伴わない plain M-token note は violation にならない。 [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1, knowledge/adr/2026-07-08-1020-retire-todo-marker-state-and-track-docs.md#D4] [tasks: T001]
- [IN-06] root `Makefile.toml` に maintainer-only gate task を追加し、maintainer CI dependencies から実行される状態にする。gate の単発実行も可能にし、diagnostic surface は `sotp verify` の通常 verifier と同じ非ゼロ exit / finding 表示で扱う。 [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1, knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D2] [conv: knowledge/conventions/enforce-by-mechanism.md#Rules] [tasks: T004]

### Out of Scope
- [OS-01] `overlay/Makefile.toml` への retention gate task 追加、または overlay CI dependencies への接続。exported template は maintainer-only retention gate を継承しない。 [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D2] [tasks: T004]
- [OS-02] `apps/` 配下の per-file boundary-manifest exclude による配布除外。boundary manifest の prefix-free invariant と整合しない rejected path であり、本 track では採らない。 [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D2] [tasks: T004]
- [OS-03] runtime skip、環境変数 opt-out、overlay presence 判定などによって integration test を残す経路。D1 は test file の削除と verify subcommand 化を決定しているため、test 内部の条件分岐で配布問題を避ける方式は扱わない。 [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1, knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D2] [tasks: T004]
- [OS-04] scan semantics の拡張または緩和。新しい retired token family の追加、gate word set の再設計、live surface policy の縮小/拡大は本 track の目的ではなく、現行 gate の意味論を verify chain に移すことに限定する。 [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1] [tasks: T001, T004]
- [OS-05] 他の repo-tree scan 型 tests や shipped-config consistency tests の verify 化。ADR は同形式の別 test について将来の再評価対象として切り出しており、本 track は retention gate だけを対象にする。 [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1] [tasks: T004]

## Constraints
- [CN-01] scanner I/O problem は fail-closed とする。directory listing / file read / path traversal に失敗した場合、verifier は success として扱わず、library/usecase production code では panic ではなく `Result` error として伝搬し、CLI で non-zero exit に変換する。 [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1] [conv: knowledge/conventions/coding-principles.md#No Panics in Library Code, knowledge/conventions/enforce-by-mechanism.md#Rules] [tasks: T001, T002, T003]
- [CN-02] distribution boundary は Makefile 接続で表現する。maintainer repository では root `Makefile.toml` の CI dependency から gate が実行され、exported template では `overlay/Makefile.toml` がその dependency を持たないため gate が実行されない。 [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D2] [conv: knowledge/conventions/responsibility-boundary.md#Rules] [tasks: T004]
- [CN-03] boundary manifest の `apps/` subtree に per-file exclude を追加して distribution を解決しない。final state は prefix-free invariant に依存する manifest workaround を持たず、Makefile 接続の差分だけで maintainer-only 性を成立させる。 [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D2] [tasks: T004]
- [CN-04] track artifact と implementation-facing diagnostic は retired marker literal を M token として扱う。M token の存在は gate word との same-line co-existence を検査するための content signal であり、状態 field や readiness declaration として読み取る別 mechanism を導入しない。 [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1, knowledge/adr/2026-07-08-1020-retire-todo-marker-state-and-track-docs.md#D1, knowledge/adr/2026-07-08-1020-retire-todo-marker-state-and-track-docs.md#D4] [conv: knowledge/conventions/workflow-ceremony-minimization.md#Rules] [tasks: T001]
- [CN-05] new verifier tests must cover both detection and non-detection behavior. Negative tests prove forbidden identifier/path injection and M-token state-expression injection fail the scanner; positive tests prove a clean synthetic live surface and an M-token note without readiness/blocking gate words pass. [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1, knowledge/adr/2026-07-08-1020-retire-todo-marker-state-and-track-docs.md#D4] [conv: knowledge/conventions/testing.md#Rules] [tasks: T001]

## Acceptance Criteria
- [ ] [AC-01] `apps/cli/tests/retention_gate.rs` no longer exists, and no equivalent integration test remains that scans the maintainer repository live surface as part of cargo test. Retention checking is reachable through the new `sotp verify` path instead. [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1] [tasks: T004]
- [ ] [AC-02] `sotp verify --help` exposes a retention gate subcommand, and invoking that subcommand against the clean maintainer repository exits 0 with the same pass meaning as the removed integration test's clean-workspace assertion. Source inspection and `cargo make check-layers` show the execution path is wired through the established verify chain: infrastructure checker, usecase port/service, CLI-driver input, CLI subcommand, and composition root wiring. The CLI path invokes the usecase service through that chain rather than calling a scanner helper directly. [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1] [conv: knowledge/conventions/hexagonal-architecture.md#Layer Dependencies] [tasks: T001, T002, T003]
- [ ] [AC-03] A deterministic scanner test asserts the configured live-surface root/pattern set equals IN-03. Injecting any retired identifier or retired path from IN-04 into a representative file for each IN-03 root/pattern that exists in the fixture causes the verifier/scanner to fail non-zero and report a finding that identifies the path, line, and retired token category. [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1, knowledge/adr/2026-07-08-1020-retire-todo-marker-state-and-track-docs.md#D4] [conv: knowledge/conventions/testing.md#Rules] [tasks: T001, T002, T003]
- [ ] [AC-04] Injecting an M-token state expression with a readiness/blocking gate word before the M token causes verifier failure, and injecting the reversed order on the same line also causes verifier failure. [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1, knowledge/adr/2026-07-08-1020-retire-todo-marker-state-and-track-docs.md#D4] [tasks: T001]
- [ ] [AC-05] A clean synthetic live-surface layout passes the scanner, and a line containing an M token without any readiness/blocking gate word on the same line also passes. This verifies the gate detects state expressions rather than plain task notes. [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1, knowledge/adr/2026-07-08-1020-retire-todo-marker-state-and-track-docs.md#D4] [tasks: T001]
- [ ] [AC-06] A scanner I/O failure scenario is covered by a test and produces an error result / non-zero verifier outcome rather than success, silent skip, or a production-code panic. [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1] [conv: knowledge/conventions/coding-principles.md#No Panics in Library Code, knowledge/conventions/testing.md#Rules] [tasks: T001, T002, T003]
- [ ] [AC-07] root `Makefile.toml` defines a maintainer retention gate task and the maintainer CI dependency chain invokes it. The task can also be run directly for local diagnosis. [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1, knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D2] [tasks: T004]
- [ ] [AC-08] `overlay/Makefile.toml` does not define or depend on the maintainer retention gate task. Exported templates therefore do not run the maintainer-only retention gate in their default CI path. [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D2] [tasks: T004]
- [ ] [AC-09] The final diff does not add any `apps/` per-file boundary-manifest exclude to make the retention gate disappear from exports. The distribution exclusion is explained by the root/overlay Makefile connection difference alone. [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D2] [tasks: T004]
- [ ] [AC-10] `cargo make ci` in the maintainer repository includes the new retention gate and exits 0 on a clean tree, while the verifier's targeted negative tests demonstrate non-zero behavior for injected retired identifiers and M-token state expressions. [adr: knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D1, knowledge/adr/2026-07-08-2306-retention-gate-verify-subcommand.md#D2, knowledge/adr/2026-07-08-1020-retire-todo-marker-state-and-track-docs.md#D4] [conv: knowledge/conventions/enforce-by-mechanism.md#Rules] [tasks: T001, T004]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/testing.md#Rules
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/responsibility-boundary.md#Rules
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/track-lifecycle.md#Generated Views

## Signal Summary

### Stage 1: Spec Signals
🔵 29  🟡 0  🔴 0

