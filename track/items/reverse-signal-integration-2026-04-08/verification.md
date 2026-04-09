# Verification — reverse-signal-integration-2026-04-08

## Scope Verified

- [ ] All in-scope items have corresponding tasks
- [ ] Out-of-scope items are not addressed by any task

## Manual Verification Steps

- [ ] T01: undeclared types/traits が kind_tag: undeclared_type / undeclared_trait の Red DomainTypeSignal として返される
- [ ] T02: 定義済みだが TypeGraph に見つからない型が Yellow シグナルを返す
- [ ] T02: 定義+実装+構造一致の型が従来通り Blue を返す (回帰なし)
- [ ] T03: domain-type-signals が逆方向チェックを実行し undeclared Red + 未実装 Yellow を domain-types.json に保存。サマリ出力 blue=N yellow=M red=K (undeclared=U)。domain-types.md レンダリング
- [ ] T03: domain-types.json 不在時にエラー終了し /track:design を促す (ファイル作成しない)
- [ ] T04: CI が domain-type-signals → verify spec-states の順で実行されること (undeclared Red が gate に届く前提)
- [ ] T04: verify spec-states が Red で fail + /track:design 案内
- [ ] T04: verify spec-states が Yellow のみで pass (途中コミット許容)
- [ ] T04: merge 時に Yellow が残っている場合はブロック (全 Blue 必須)
- [ ] T05: agent-profiles.json の全 profile (default / claude-heavy / codex-heavy) に designer が追加
- [ ] T06: /track:design が対象トラックの plan.md を入力に designer capability を呼び出し domain-types.json を生成
- [ ] T06: /track:design が既存 domain-types.json を増分更新
- [ ] T07: /track:plan 完了時に /track:design が次ステップとして案内
- [ ] T07: DEVELOPER_AI_WORKFLOW.md と knowledge/WORKFLOW.md に TDDD フロー追記
- [ ] T07: registry.md (cargo make track-sync-views で生成) の Next 列に /track:design が表示されること
- [ ] T08: ADR 2026-04-08-1800 が Accepted で最終化。ADR README 索引タイトル一致
- [ ] cargo make ci が通る

## Result

- pending

## Open Issues

- なし

## Verified At

- (未検証)
