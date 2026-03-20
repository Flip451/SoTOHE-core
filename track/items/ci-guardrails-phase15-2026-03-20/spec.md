# Spec: CI Guardrails Phase 1.5

## Goal

CI の検知網を敷設し、Phase 1.5 以降のリファクタリングで発生しうる再発パターンを自動検出する。
併せて、planning artifacts 初回コミット時の review guard ブロック問題 (WF-54) を修正し、
plan.md の metadata.json SSoT との乖離を CI で検出する (WF-55 Phase 1)。
registry.md は git 管理から外し、完全生成ビューに移行する (STRAT-04)。

## Scope

### In Scope

1. **WF-54**: metadata.json 作成時に review state を初期化。`check-approved` が `NotStarted` を許可
2. **module-size**: `.rs` ファイル行数の CI 検査 (warn 400 / error 700)
3. **domain-strings**: `libs/domain/src/` 内の `pub String` フィールドを `syn` AST パースで検出
4. **clippy::too_many_lines**: CLI crate に属性追加
5. **view-freshness**: `plan.md` が metadata.json からの最新レンダリングと一致するか検証
6. **STRAT-04**: `registry.md` を `.gitignore` に追加し git 管理対象外にする（完全生成ビュー化）

### Out of Scope

- DM-01/02/03 の String 型化（別トラック）
- WF-55 Phase 2/3（verification.md / spec.md 統合）
- `pub(crate)` 構造的ロック（1.5-17）

## Constraints

- 既存の `sotp verify` パターンに従う（infrastructure で実装、CLI で dispatch）
- `architecture-rules.json` に `module_limits` セクションを追加（SSoT）
- vendor/ ディレクトリは module-size / domain-strings の対象外
- module-size / domain-strings は既存の超過ファイルが多数あるため **warning-only**（CI を fail させない）。リファクタリング完了後に error に切り替え
- domain-strings は行マッチングで実装（syn 依存追加は過剰。複数行宣言は rustfmt で単一行に正規化されるため実用上十分）

## Acceptance Criteria

- [ ] `/track:plan` で作成した metadata.json に review section が含まれる [source: WF-54]
- [ ] review section が absent の metadata.json で `check-approved` がエラーを返す（fail-closed） [source: WF-54]
- [ ] review state が NotStarted の track で `track-commit-message` が成功する [source: WF-54]
- [ ] `sotp verify module-size` が 400行超の .rs ファイルを WARNING で報告する [source: refactoring-plan §9-1]
- [ ] `sotp verify module-size` が 700行超の .rs ファイルを ERROR で報告する [source: refactoring-plan §9-1]
- [ ] `sotp verify module-size` が vendor/ 配下のファイルを除外する [source: refactoring-plan §9-1]
- [ ] `sotp verify domain-strings` が pub String フィールドを検出する（newtype 除外） [source: refactoring-plan §9-6]
- [ ] `#![warn(clippy::too_many_lines)]` が CLI crate に設定済み [source: refactoring-plan §9-2]
- [ ] `sotp verify view-freshness` が plan.md の乖離を検出する [source: WF-55]
- [ ] `registry.md` が `.gitignore` に含まれ、git tracking から除外されている [source: STRAT-04]
- [ ] `cargo make track-sync-views` で registry.md が生成される（動作は既存通り） [source: STRAT-04]
- [ ] `verify-track-registry` が registry.md のファイル存在ではなく metadata.json ベースで検証する [source: STRAT-04]
- [ ] registry.md をハードコード参照しているワークフローコード（planning-only commit 検証、activation dirty-path 等）が untrack 後も正常動作する [source: STRAT-04]
- [ ] `cargo make ci` が新しい verify サブコマンドを含む [source: CI gate requirement]
- [ ] 全テストが TDD (Red -> Green -> Refactor) で作成される [source: track/workflow.md]
