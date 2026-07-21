<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 20, yellow: 0, red: 0 }
---

# ビルド成果物によるディスク圧迫の解消と dry gate 重量依存の feature flag 化

## Goal

- [GO-01] semantic-dup の重量依存を `semantic-dup` cargo feature の配下に置き、既定ビルドでは当該依存を compile / link しない。 [adr: knowledge/adr/2026-07-20-1608-disk-footprint-and-dry-feature-gating.md#D1]
- [GO-02] `semantic-dup` feature 無効の binary における dry 系コマンドを、feature 無効であることを明示する fail-closed エラー終了とする。 [adr: knowledge/adr/2026-07-20-1608-disk-footprint-and-dry-feature-gating.md#D2]
- [GO-03] sccache のサイズ上限と `target/`・`.cache/` の掃除を、既定値と利用者による変更経路を持つ設定可能な機構として提供する。 [adr: knowledge/adr/2026-07-20-1608-disk-footprint-and-dry-feature-gating.md#D3]
- [GO-04] CI の clippy と test は `semantic-dup` feature を有効にして実行し、sotp binary の既定ビルドは feature 無効の軽量構成とする。 [adr: knowledge/adr/2026-07-20-1608-disk-footprint-and-dry-feature-gating.md#D4]

## Scope

### In Scope
- [IN-01] infrastructure の semantic-dup 実装が使用する重量依存を optional dependency とし、`semantic-dup` cargo feature の配下に置く。既定ビルドは当該依存を compile / link しない。 [adr: knowledge/adr/2026-07-20-1608-disk-footprint-and-dry-feature-gating.md#D1] [tasks: T001, T002]
- [IN-02] `semantic-dup` feature 無効でビルドした binary において、dry 系サブコマンドを利用可能な導線として登録したまま、feature 無効を明示するエラーで fail-closed に終了させる。 [adr: knowledge/adr/2026-07-20-1608-disk-footprint-and-dry-feature-gating.md#D2] [tasks: T002]
- [IN-03] sccache のサイズ上限と、`target/` および `.cache/` を掃除するメンテナンスタスクを機構化する。上限値と掃除対象範囲は `.harness/config/` 配下の既定値を持つ設定ファイルで利用者が変更できる。 [adr: knowledge/adr/2026-07-20-1608-disk-footprint-and-dry-feature-gating.md#D3] [tasks: T003]
- [IN-04] リポジトリ CI の clippy と test を `semantic-dup` feature 有効で実行し、feature gate 内コードを継続して compile・検証する。一方で sotp binary の既定ビルドは軽量な feature 無効のままとする。 [adr: knowledge/adr/2026-07-20-1608-disk-footprint-and-dry-feature-gating.md#D4] [tasks: T004]

### Out of Scope
- [OS-01] semantic-dup 機能を別 binary または別 crate に分離すること。 [adr: knowledge/adr/2026-07-20-1608-disk-footprint-and-dry-feature-gating.md#D1]
- [OS-02] ONNX、lancedb 等の重量依存を軽量な代替実装へ置き換えること。 [adr: knowledge/adr/2026-07-20-1608-disk-footprint-and-dry-feature-gating.md#D1]
- [OS-03] DRY gate の runtime 有効・無効ポリシーを変更すること。既存の repository-wide opt-in 設定は維持する。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D1, knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2]

## Constraints
- [CN-01] cargo feature の名称は `semantic-dup` とし、既定 feature に含めない。 [adr: knowledge/adr/2026-07-20-1608-disk-footprint-and-dry-feature-gating.md#D1] [tasks: T001, T002]
- [CN-02] feature 無効 binary の dry 系コマンドは silent skip または自動 fallback を行わない。 [adr: knowledge/adr/2026-07-20-1608-disk-footprint-and-dry-feature-gating.md#D2] [tasks: T002]
- [CN-03] sccache 上限および掃除対象は暗黙に hard-code せず、`.harness/config/` 配下の設定ファイルに既定値と利用者による変更経路を持たせる。 [adr: knowledge/adr/2026-07-20-1608-disk-footprint-and-dry-feature-gating.md#D3] [tasks: T003]
- [CN-04] CI の clippy / test は `semantic-dup` feature 有効で実行して gate 内コードの腐敗を防ぐ。 [adr: knowledge/adr/2026-07-20-1608-disk-footprint-and-dry-feature-gating.md#D4] [tasks: T004]

## Acceptance Criteria
- [ ] [AC-01] 既定 feature のビルドでは semantic-dup の重量依存が compile / link されず、`semantic-dup` feature を有効にしたビルドでは semantic-dup 機能を含む。 [adr: knowledge/adr/2026-07-20-1608-disk-footprint-and-dry-feature-gating.md#D1] [tasks: T001, T002]
- [ ] [AC-02] feature 無効でビルドした binary から dry 系コマンドを実行すると、feature 無効であることが分かるメッセージを出して非成功終了する。コマンドを黙って省略または代替動作させない。 [adr: knowledge/adr/2026-07-20-1608-disk-footprint-and-dry-feature-gating.md#D2] [tasks: T002]
- [ ] [AC-03] 設定された sccache サイズ上限が利用され、メンテナンスタスクが設定された範囲の `target/` と `.cache/` を掃除できる。 [adr: knowledge/adr/2026-07-20-1608-disk-footprint-and-dry-feature-gating.md#D3] [tasks: T003]
- [ ] [AC-04] キャッシュ上限と掃除対象範囲には設定ファイル内の既定値があり、利用者は同設定ファイルを通じて環境に応じた値へ変更できる。 [adr: knowledge/adr/2026-07-20-1608-disk-footprint-and-dry-feature-gating.md#D3] [tasks: T003]
- [ ] [AC-05] CI の clippy と test は `semantic-dup` feature を有効にして実行され、通常の sotp binary ビルドは feature 無効の軽量構成を既定とする。 [adr: knowledge/adr/2026-07-20-1608-disk-footprint-and-dry-feature-gating.md#D4] [tasks: T004]

## Related Conventions (Required Reading)
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/responsibility-boundary.md#Rules
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/prefer-type-safe-abstractions.md#Make Illegal States Unrepresentable

## Signal Summary

### Stage 1: Spec Signals
🔵 20  🟡 0  🔴 0

