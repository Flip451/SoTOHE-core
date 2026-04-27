<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 10, yellow: 0, red: 0 }
---

# llvm-cov を nextest 経路に統一

## Goal

- [GO-01] `cargo make llvm-cov` が nextest 経由で実行されるように `Makefile.toml` の `llvm-cov-local` task を切り替え、`cargo make test` (nextest) と test harness を統一することで、harness 差に起因する乖離を解消する [adr: knowledge/adr/2026-04-27-0124-llvm-cov-nextest-harness-alignment.md#D1: `Makefile.toml` の `llvm-cov-local` task で `cargo llvm-cov nextest` 経由に切り替える]

## Scope

### In Scope
- [IN-01] `Makefile.toml` の `llvm-cov-local` task の args を `["llvm-cov", "nextest", "--html", "--all-features", "--locked"]` に変更する。変更対象は L445-449 周辺の 1 task のみ [adr: knowledge/adr/2026-04-27-0124-llvm-cov-nextest-harness-alignment.md#D1: `Makefile.toml` の `llvm-cov-local` task で `cargo llvm-cov nextest` 経由に切り替える] [tasks: T001]

### Out of Scope
- [OS-01] `apps/cli/src/commands/review/tests.rs` の env mutation 方式 (`env_lock()` / `EnvVarGuard`) の DI 化・`serial_test` クレート導入など、env mutation 方式そのものの改善は本 track のスコープ外。ADR D2 で別 track として deferred されている [adr: knowledge/adr/2026-04-27-0124-llvm-cov-nextest-harness-alignment.md#D2: env mutation 方式そのもの (DI 化 / `serial_test`) の対応は別 track として deferred する]
- [OS-02] `RUST_TEST_THREADS=1` で libtest を直列化するアプローチは ADR で却下されており、本 track では採用しない [adr: knowledge/adr/2026-04-27-0124-llvm-cov-nextest-harness-alignment.md#B. `RUST_TEST_THREADS=1` で libtest を直列化する]
- [OS-03] `cargo make llvm-cov` を CI から撤去することは ADR で却下されており、本 track では採用しない。coverage は CI 補助指標として維持する [adr: knowledge/adr/2026-04-27-0124-llvm-cov-nextest-harness-alignment.md#C. `cargo make llvm-cov` を撤去する (coverage を諦める)]

## Constraints
- [CN-01] 変更範囲は `Makefile.toml` の `llvm-cov-local` task のみとする。他の task・crate・テストコードへの変更は行わない [adr: knowledge/adr/2026-04-27-0124-llvm-cov-nextest-harness-alignment.md#D1: `Makefile.toml` の `llvm-cov-local` task で `cargo llvm-cov nextest` 経由に切り替える] [tasks: T001]
- [CN-02] 修正後も `cargo make llvm-cov` が外部インターフェースとして維持される。inner task の実装を切り替えるのみで、呼び出し側の compose ラッパー (`llvm-cov` task) への変更は不要 [adr: knowledge/adr/2026-04-27-0124-llvm-cov-nextest-harness-alignment.md#D1: `Makefile.toml` の `llvm-cov-local` task で `cargo llvm-cov nextest` 経由に切り替える] [tasks: T001]

## Acceptance Criteria
- [ ] [AC-01] `cargo make llvm-cov` が正常終了し、全テスト (2062 件以上) が pass して HTML カバレッジレポートが生成される。`apps/cli/src/commands/review/tests.rs` の 13 テストで失敗が発生しない [adr: knowledge/adr/2026-04-27-0124-llvm-cov-nextest-harness-alignment.md#D1: `Makefile.toml` の `llvm-cov-local` task で `cargo llvm-cov nextest` 経由に切り替える] [tasks: T001]
- [ ] [AC-02] `cargo make test` と `cargo make llvm-cov` が同一テスト群を同一 nextest harness で実行し、両コマンドで pass/fail の結果が一致する [adr: knowledge/adr/2026-04-27-0124-llvm-cov-nextest-harness-alignment.md#D1: `Makefile.toml` の `llvm-cov-local` task で `cargo llvm-cov nextest` 経由に切り替える] [tasks: T001]
- [ ] [AC-03] `cargo make ci` (fmt-check + clippy + nextest + deny + check-layers + verify-*) が全て pass する [adr: knowledge/adr/2026-04-27-0124-llvm-cov-nextest-harness-alignment.md#Positive] [tasks: T001]

## Related Conventions (Required Reading)
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- .claude/rules/07-dev-environment.md#Task Runner: cargo-make
- .claude/rules/05-testing.md#Commands
- .claude/rules/04-coding-principles.md#No Panics in Library Code

## Signal Summary

### Stage 1: Spec Signals
🔵 10  🟡 0  🔴 0

