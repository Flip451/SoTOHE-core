# Spec: STRAT-03 Phase 5 — verify script 群の Rust 移行

## Feature Goal

残存する Python verify スクリプト群（`verify_tech_stack_ready.py`, `verify_latest_track_files.py`, `verify_architecture_docs.py`, `verify_orchestra_guardrails.py`）および `check_layers.py` を `sotp verify` サブコマンドに移行し、CI 検証パスから Python 依存を削減する。

## Scope

### In Scope

- `toml`, `regex` クレートの workspace 追加
- Domain 層: 検証結果型 (`VerifyOutcome`, `Finding`, `Severity`)
- Infrastructure 層: 5 つの検証ロジックモジュール
  - `architecture_rules` — `docs/architecture-rules.json` パース + `Cargo.toml`/`deny.toml` 同期検証
  - `convention_docs` — `project-docs/conventions/README.md` インデックス検証
  - `tech_stack` — `track/tech-stack.md` の未解決マーカー検出（テンプレートdev/planning-phase バイパス付き）
  - `latest_track` — 最新トラックの `spec.md`/`plan.md`/`verification.md` 完全性検証
  - `orchestra` — `.claude/settings.json` 構造検証（hooks, permissions, env, agents, model resolution）
- CLI 層: `sotp verify` サブコマンドグループに 5 サブコマンド追加
  - `sotp verify tech-stack`
  - `sotp verify latest-track`
  - `sotp verify arch-docs`
  - `sotp verify layers` (cargo metadata 解析)
  - `sotp verify orchestra`
- `Makefile.toml`: verify `-local` タスクを Rust 版に切替
- 移行済み Python スクリプト 5 ファイルの削除
- `scripts-selftest` テストリスト更新

### Out of Scope

- `architecture_rules.py` の CLI モード（`workspace-tree`, `direct-checks` 等）— 別フェーズ
- `convention_docs.py` の `add` CLI — 別フェーズ
- `track_schema.py`, `track_markdown.py` 等のライブラリ Python 削除（テストスイート依存）— 別フェーズ
- advisory hooks (`.claude/hooks/*.py`) の Rust 移行（Phase 6）
- `check_layers.py` が依存する `architecture_rules.py` 本体の削除（非 verify CLI モードが残るため）

## Constraints

- 各 Rust 実装は対応する Python 版と同等の検証項目を網羅すること
- `verify_orchestra_guardrails.py` の検証定数（`EXPECTED_HOOK_PATHS`, `FORBIDDEN_ALLOW`, `EXPECTED_DENY` 等）を Rust 側に正確に移植すること
- `check_layers.py` の `cargo metadata` 呼び出しは subprocess 経由を維持（`cargo_metadata` クレート不要）
- `architecture_rules.py` の TOML パースは `toml` クレートで代替
- テンプレート dev モード (`TRACK_TEMPLATE_DEV=1` / `.track-template-dev`) バイパスを `verify tech-stack` に実装
- planning-phase バイパス（全 active トラックが planned の場合）を `verify tech-stack` に実装
- `verify latest-track` の track 選択アルゴリズム（priority + timestamp）を Python 版と一致させる
- fenced code block 内の placeholder は無視する（`verify latest-track`）
- v3 branch field セマンティクスの検証を維持する（`verify latest-track`）
- 既存の `sotp track views validate` との機能重複なし（plan/metadata/registry 検証は既存コマンドが担当）

## Acceptance Criteria

- [ ] `cargo make verify-tech-stack` が `sotp verify tech-stack` を実行する（Python 非依存）
- [ ] `cargo make verify-latest-track` が `sotp verify latest-track` を実行する（Python 非依存）
- [ ] `cargo make verify-arch-docs` が `sotp verify arch-docs` を実行する（Python 非依存）
- [ ] `cargo make check-layers` が `sotp verify layers` を実行する（Python 非依存）
- [ ] `cargo make verify-orchestra` が `sotp verify orchestra` を実行する（Python 非依存）
- [ ] `sotp verify tech-stack` が未解決マーカー検出・テンプレートdev バイパス・planning バイパスを正しく処理する
- [ ] `sotp verify latest-track` が最新トラックの spec/plan/verification 完全性を検証する
- [ ] `sotp verify arch-docs` が architecture-rules.json / Cargo.toml / deny.toml 同期を検証する
- [ ] `sotp verify arch-docs` が convention docs インデックス同期を検証する
- [ ] `sotp verify arch-docs` が必須テキストパターンの存在を検証する（T004: _require_file/_require_line チェック群）
- [ ] `sotp verify layers` が cargo metadata からレイヤー依存違反を検出する（T009: infra layers モジュール）
- [ ] `sotp verify orchestra` が settings.json の hook/permission/env/agent 構造を検証する
- [ ] 移行済み Python スクリプト 5 ファイルが削除されている
- [ ] `cargo make ci` が通る
- [ ] `cargo make scripts-selftest` が通る（削除済みテスト除去後）
