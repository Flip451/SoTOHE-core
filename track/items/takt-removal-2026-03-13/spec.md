# Spec: takt 廃止計画

## Goal

`takt` をこの repo の正式な実行レイヤから外し、`/track:*`、Rust CLI、Claude Code / Agent Teams
だけで planning / implementation / review / commit / PR / archive が閉じる運用へ移行する。

## Scope

- `.takt/` 配下と `cargo make takt-*` wrapper の撤去計画
- `scripts/takt_profile.py` と関連 Python test 群の廃止または置換計画
- pending add/message/note artifact の保存場所と commit traceability の再設計
- workflow / orchestration / traceability 文書の takt 非依存化
- `.claude/commands/**`, `.claude/rules/**`, `.claude/docs/WORKFLOW.md`, `START_HERE_HUMAN.md` の takt 前提除去
- `.claude/hooks/agent-router.py`, `.claude/hooks/_agent_profiles.py`, `.claude/agent-profiles.json` の takt host / routing 前提除去
- `libs/usecase/src/git_workflow.rs`, `scripts/git_ops.py`, `scripts/test_git_ops.py` の transient automation path 契約見直し
- guardrail / CI / selftest の takt 前提除去
- takt 廃止後の rollout 順と Definition of Done の明文化

## Non-Goals

- `takt` と同等の新しい自律キューシステムを作ること
- すべての Python utility をこのトラックだけで削除すること
- 既存 track の履歴や git notes を書き換えること

## Constraints

- 公開 UI は引き続き `/track:*` を維持する
- `metadata.json` を SSoT とする track workflow は維持する
- `/track:commit` の note 適用、`track/registry.md` 更新、PR workflow は takt なしでも壊してはいけない
- `cargo make ci` の最終ゲートを常に保つ
- security-critical hook と branch guard の fail-closed 契約は維持する
- `takt` 廃止は docs / guardrails / wrappers / tests を同時に揃えて進める

## Acceptance Criteria

- [ ] takt 依存の inventory と cutover 原則が fixed artifact として記録されている
- [ ] workflow / orchestration / traceability の説明が takt 非依存の記述に置き換わる計画がある
- [ ] `.takt/pending-*` と handoff/debug scratch の代替経路が明示されている
- [ ] `.claude/commands/**`, `.claude/rules/**`, `.claude/docs/WORKFLOW.md`, `START_HERE_HUMAN.md` の更新対象が明示されている
- [ ] `.claude/hooks/agent-router.py`, `.claude/hooks/_agent_profiles.py`, `.claude/agent-profiles.json` の更新対象が明示されている
- [ ] `libs/usecase/src/git_workflow.rs`, `scripts/git_ops.py`, `scripts/test_git_ops.py` の transient path 契約更新が計画に含まれている
- [ ] `Makefile.toml`, `.takt/`, `scripts/takt_profile.py`, test 群の削除・置換順が定義されている
- [ ] `.claude/settings.json`, verify scripts, docs, selftests の更新対象が明示されている
- [ ] `cargo make ci`、PR merge、`/track:commit`、archive/registry 更新が takt 無しで閉じる DoD がある
