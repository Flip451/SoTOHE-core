# Observations — 2026-08-25-orchestrator-context-discipline

## 委譲の境界（機械検証不能）

- Codex implementer / review-fix-lead の sandbox は `.agents/` と `.codex/` 配下への書込みを拒否する（`apply_patch` も拒否）。記録上、T011 / T012 / T013 は `.agents/` 側、T014 は workflow 側、T015 は `.codex/` 側である。`.agents/` / `.codex/` 配下への review finding 修正は、pr-review SSoT の定める回復経路（委譲失敗時のみ親が直接編集）として orchestrator が適用した。回復経路の使用頻度が高かったのは ADR の Consequences が予見した「委譲経路が弱い provider」の事例であり、Reassess When の観測対象にあたる。
- review-fix-lead の書込み境界は「現 diff に含まれるファイル」に限られるため、未変更ファイルへの修正要求（例: 未着手の thin adapter の矛盾）は `blocked_cross_scope` で親に返る。次 unit の task に属する内容は最小限の矛盾解消だけを親が行い、残りを当該 task に残した（T008 で吸収）。

## 記録上の注意

- commit `1245c377` は本文が B2（T003, T005, T006, T009, T011, T013, T016）の内容だが、subject が B3 の文面になっている。guarded commit の実行中に次 batch の `tmp/track-commit/commit-message.txt` を先行して書いたため、wrapper が末尾で読んだ時点で差し替わっていた。正しい変更一覧は同 commit の git note にある。amend は禁止のため履歴は修正していない。
- T017（orchestrator の既定 reasoning effort）は着手時点で既に `medium` であり、変更なしで完了した。

## 計測の起点

- 本 track では workflow SSoT の全文読みは Step 0 の bounded exception（adr2pr の実行計画）に限定し、成果物 JSON は diff / blocker 調査時のみ開いた。
