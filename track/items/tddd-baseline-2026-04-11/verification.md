# Verification: TDDD-02 Baseline reverse signal

## Scope Verified

- [ ] ADR 2026-04-11-0001 の全 Decision セクションがタスクに対応している
- [ ] 4 グループ評価 (A\B, A∩B, B\A, ∁(A∪B)∩C) が正しく実装されている
- [ ] baseline の除外フィールド (outgoing, module_path) が含まれていない

## Manual Verification Steps

- [ ] `baseline-capture` 実行後に `domain-types-baseline.json` が生成される
- [ ] `baseline-capture` が既存 baseline がある場合にスキップする (冪等)
- [ ] `domain-type-signals` が baseline 不在時にエラーを返す
- [ ] 既存型 (100+) が reverse check で Red にならずスキップされる
- [ ] 未宣言の新規型追加時に Red が出る
- [ ] 未宣言の構造変更時に Red が出る
- [ ] 未宣言の型削除時に Red が出る
- [ ] 宣言済み型は forward check のみで評価される
- [ ] `/track:design` の Step 4 が `baseline-capture` を呼び出す記述になっている
- [ ] E2E: `/track:design` → `baseline-capture` → `domain-type-signals` の統合フローが正常に動作する
- [ ] `cargo make ci` が通る

## Result / Open Issues

(実装後に記入)

## Verified At

(検証完了後に記入)
