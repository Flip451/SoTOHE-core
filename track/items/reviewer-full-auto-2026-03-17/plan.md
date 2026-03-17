<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Full model reviewer の --full-auto 自動付与

agent-profiles.json に model_profiles を追加し、per-model の振る舞い（--full-auto 等）を設定ファイルで一元管理する。
review.rs は agent-profiles.json を読んで model に応じたフラグを解決する。
未知モデルは fail-closed（full_auto: true）。spark は full_auto: false。

## Phase 1: agent-profiles.json スキーマ拡張

agent-profiles.json の codex provider に model_profiles を追加。
各モデルの full_auto フラグを設定ファイルで一元管理する。
未知モデルのフォールバックは full_auto: true（fail-closed）。

- [ ] agent-profiles.json の codex provider に model_profiles セクションを追加（gpt-5.4: full_auto=true, gpt-5.3-codex: full_auto=true, gpt-5.3-codex-spark: full_auto=false）

## Phase 2: Usecase 層 — モデルプロファイル解決ロジック

domain/usecase 層に agent-profiles.json のモデルプロファイル読み取り型を追加。
ModelProfile 型と resolve_full_auto(model, profiles) 関数を定義。
フォールバック（未知モデル → full_auto: true）のテストを含む。

- [ ] usecase 層に ModelProfile 型と resolve_full_auto() 関数を追加
- [ ] resolve_full_auto のユニットテスト（既知モデル、未知モデルフォールバック、model_profiles 欠如）

## Phase 3: CLI 層 — invocation 構築の統合

build_codex_invocation に full_auto: bool パラメータを追加。
run_codex_local で agent-profiles.json を読み、model_profiles から full_auto を解決。
fake-codex 統合テストで --full-auto の有無を検証。

- [ ] build_codex_invocation に full_auto: bool パラメータを追加しテスト更新
- [ ] run_codex_local で agent-profiles.json を読み model_profiles から full_auto を解決 + fake-codex 統合テスト（full model: --full-auto あり、spark: --full-auto なし、ファイル読み込み失敗: fail-closed）

## Phase 4: クリーンアップ

ワークアラウンドスクリプト削除、ドキュメント・メモリ更新、CI グリーン確認。

- [ ] ワークアラウンドスクリプト (test-full-auto.sh 等) の削除
- [ ] ドキュメント更新 (DESIGN.md reviewer セクション、メモリノート、spec.md)
- [ ] CI グリーン確認
