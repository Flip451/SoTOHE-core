# Spec: サブエージェントのデフォルトモデルを Sonnet に変更

## Goal

Claude Code サブエージェントのデフォルトモデルを Opus → Sonnet に変更し、
品質を維持しつつコストを削減する。

## Scope

- `.claude/settings.json` の `CLAUDE_CODE_SUBAGENT_MODEL` を `claude-sonnet-4-6` に変更
- `.claude/rules/11-subagent-model.md` でオーバーライドガイダンスを提供
- `track-plan/SKILL.md`、`codex-system/SKILL.md` の `gpt-5.3-codex` および `.claude/commands/track/review.md` の `default_model` 直参照を `{model}` プレースホルダーに置換（既存の解決ルールに準拠）
- `verify_orchestra_guardrails.py` に `CLAUDE_CODE_SUBAGENT_MODEL` の allowlist 検証を追加 + テスト

## Constraints

- `CLAUDE_CODE_SUBAGENT_MODEL` は Claude Code のネイティブ設定であり、プログラム的にサブエージェントのモデルを制御する
- `agent-profiles.json` の変更は不要（既存の capability → provider マッピングに影響しない）
- `_agent_profiles.py` の変更は不要（Python コードからの消費者がない）
- 個別の Agent ツール呼び出しで `model` パラメータを指定すれば settings.json のデフォルトをオーバーライドできる
- Rust コードの変更なし

## Acceptance Criteria

- [ ] `CLAUDE_CODE_SUBAGENT_MODEL` が `claude-sonnet-4-6` に設定されている
- [ ] `.claude/rules/11-subagent-model.md` がオーバーライドガイダンスを提供する
- [ ] `track-plan/SKILL.md`、`codex-system/SKILL.md` の `gpt-5.3-codex` と `.claude/commands/track/review.md` の `default_model` 直参照が修正されている
- [ ] `verify_orchestra_guardrails.py` が `CLAUDE_CODE_SUBAGENT_MODEL` を allowlist `[claude-sonnet-4-6, claude-opus-4-6, claude-haiku-4-5-20251001]` で検証する
- [ ] `verify_orchestra_guardrails.py` が `.claude/skills/` と `.claude/commands/` 内のハードコード Codex モデルリテラル（regex: `gpt-\d+`）の不在を検証する
- [ ] `verify_orchestra_guardrails.py` が `codex-system/SKILL.md`、`track-plan/SKILL.md`、`.claude/commands/track/review.md`（現在 Codex モデル解決を記述する canonical target 3ファイル）で override-first resolution が参照されていること（positive）、かつ `default_model` のみの古い記述がないこと（negative）を検証する
- [ ] 新規ファイルへの regression 防御: (a) check #2 の regex `gpt-\d+` 全ファイルスキャンがハードコードリテラルを検出、(b) `02-codex-delegation.md` が `provider_model_overrides > default_model` 解決ルールの SSoT として機能し、新規 skill/command 作成時のガイダンスを提供。セマンティックな `default_model`-only 検出の全ファイル拡張は将来課題とする
- [ ] `test_verify_scripts.py` に上記検証の pass/fail テストがある（fixture に `.claude/skills/` `.claude/commands/` を含む）
- [ ] `cargo make ci` が全パス
