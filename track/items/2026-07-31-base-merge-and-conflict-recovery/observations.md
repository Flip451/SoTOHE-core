# Observations

## 2026-08-02 — types re-entry の暫定二段階帰属 workaround

types review 入口で current catalogue に対する `task-contract coverage` / `check` が、通常の
Phase 3 再生成より先に完全な下流帰属を要求する循環を回避するため、ユーザー裁定
`tmp/handoff/2026-08-02-lane-d-delta-adjudication.md` に基づく一時的な運用 workaround を適用した。

- 根本原因はレーン A の `scope-conditional-pre-review-gates`（G22）で修理中である。
- 恒久 ADR として提案された `2026-08-02-0917-provisional-repair-task-bridge.md` は棄却・削除済みであり、
  この記録を ADR または policy へ昇格させない。
- impl-planner が暫定 `todo` repair task `T010` を作成し、変更後 contract と新規 catalogue entry の
  帰属を done task `T002` から分離した。done task の status、commit hash、履歴上の意味は変更していない。
- `T010` は帰属評価のためだけの暫定 task であり、この段階では `in_progress` / `done` へ遷移させず、
  実装および source 編集を開始しない。
- 適用直後の `bin/sotp task-contract coverage` と `bin/sotp task-contract check` はともに通過した。
- types review 収束後の通常の full Phase 3 再生成で、repair task、batch、coverage、task-contract を
  一体として再検証・正規化する。
- G22 merge 後に `develop` を取り込み、gitignored な `bin/sotp` を `cargo make build-sotp` で再構築すれば、
  scope 条件付き gate によりこの workaround は不要になる。

## 2026-08-03 — guarded base-merge conflict source-repair exception

D2 の既裁定に従い、conflict hunk の選択だけを pre-gate 実行可能化のために許す source-repair
境界を適用した。対象はこの workflow/policy 文書群のみで、placeholder・意味追加は行わない。

- 対象ファイル: `.harness/workflows/track/recover.md`, `.harness/policies/sot-reentry-sequencing.md`,
  `.harness/policies/pre-track-adr-authoring.md`, `CLAUDE.md`, `.agents/skills/track-recover/SKILL.md`
- 選択した辺: 既存文書の conflict-recovery/one-writer 境界を、D2 限定の hunk 選択と上流再収束後の
  designated-writer 再実行として明文化した。
- 理由: `task-contract-check` / signal / `track-active-gate` が multi-SoT conflict の未解消入力で
  起動不能になるため。semantic authorship は designated writer に残し、レビュー・commit gate は維持する。
- 追加適用: `spec-designer` / `type-designer` / `impl-planner` / `adr-editor` / `adr-diagnoser` の
  capability contract に `conflict-preparation` mode と即時 guardian/re-entry 条件を同期した。
