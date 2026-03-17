# Spec: Full model reviewer の --full-auto 自動付与

## Goal

`agent-profiles.json` に `model_profiles` を追加し、per-model の振る舞い（`--full-auto` 等）を設定ファイルで一元管理する。`cargo make track-local-review` は設定を読んで model に応じたフラグを自動的に Codex CLI 呼び出しに付与する。

## Background

- Full model Codex reviewer（gpt-5.4, gpt-5.3-codex）は `--full-auto` なしでは JSON verdict を返せない（GitHub Issue #4181）
- Fast model（gpt-5.3-codex-spark）は `--full-auto` なしで正常動作する
- 現状は `tmp/reviewer-runtime/test-full-auto.sh` を手動実行するワークアラウンドが必要
- `--output-schema` は既に `build_codex_invocation` で渡されている
- `agent-profiles.json` は既にプロファイルレベルで reviewer provider を切替可能（codex / claude）

## Design

### 設定の階層構造

| 層 | 何を決めるか | 設定場所 |
|---|---|---|
| Profile | reviewer は codex か claude か | `profiles.{name}.reviewer` |
| Provider | model 名、invoke パターン | `providers.codex` / `providers.claude` |
| Model Profile | `--full-auto` 等の per-model フラグ | `providers.codex.model_profiles` |

### agent-profiles.json の拡張

```json
"codex": {
  "default_model": "gpt-5.4",
  "fast_model": "gpt-5.3-codex-spark",
  "model_profiles": {
    "gpt-5.4": { "full_auto": true },
    "gpt-5.3-codex": { "full_auto": true },
    "gpt-5.3-codex-spark": { "full_auto": false }
  },
  ...
}
```

### フォールバック戦略

- `model_profiles` に該当モデルがない → `full_auto: true`（fail-closed: 安全側に倒す）
- `model_profiles` セクション自体がない → `full_auto: true`
- `agent-profiles.json` の読み込みに失敗 → `full_auto: true`

## Scope

### In Scope

- `agent-profiles.json` の codex provider に `model_profiles` セクション追加
- usecase 層に `ModelProfile` 型と `resolve_full_auto()` 関数を追加
- `build_codex_invocation` への `full_auto: bool` パラメータ追加
- `run_codex_local` で設定ファイルを読みフラグを解決
- ワークアラウンドスクリプトの削除
- 関連ドキュメント・メモリノートの更新

### Out of Scope

- CLI フラグ (`--full-auto` / `--no-full-auto`) の追加（設定ファイルで管理するため不要）
- `--reasoning-effort` 対応（Codex CLI が未サポート）
- claude provider の model_profiles（Codex CLI を使わないため不要）

## Constraints

- fail-closed: 未知モデル・読み込み失敗時は `--full-auto` を付与
- `gpt-5.3-codex` はフォールバックと同じ挙動（`full_auto: true`）だが、明示エントリとして登録する（将来の挙動変更に備えた文書化目的）。テストは `resolve_full_auto` のユニットテストで明示エントリの存在を検証する
- 既存の verdict パース・検証ロジックに変更なし
- `agent-profiles.json` のスキーマは後方互換（`model_profiles` はオプショナル）

## Acceptance Criteria

1. `agent-profiles.json` に `model_profiles` が定義されている（`gpt-5.4`, `gpt-5.3-codex`, `gpt-5.3-codex-spark` の3エントリ）
2. `cargo make track-local-review -- --model gpt-5.4 ...` が `--full-auto` 付きで Codex を起動する
3. `cargo make track-local-review -- --model gpt-5.3-codex-spark ...` が `--full-auto` なしで Codex を起動する
4. `resolve_full_auto` のユニットテストが既知モデル・未知モデルフォールバック・`model_profiles` 欠如をカバーする
5. `build_codex_invocation` のテストが full_auto=true/false の両方を検証する
6. fake-codex 統合テストで full model 時に `--full-auto` が引数に含まれ、spark 時に含まれないことを検証する
7. `agent-profiles.json` 読み込み失敗時に fail-closed（`--full-auto` 付与）になることがテストで検証されている
8. `test-full-auto.sh` 等のワークアラウンドが削除されている
9. `cargo make ci` がグリーンである
