# Stage 2 信号機にコンパイル通過を条件に入れない

## Status

Accepted

## Context

Stage 2（domain state signals）の Blue 基準は「syn AST スキャンで型名+遷移関数名が存在すること」。コンパイルが通るかは検証していない。`todo!()` や壊れたコードでも名前が AST にあれば Blue になる。

コンパイル通過を Blue の条件に追加すべきか検討した。

## Decision

Phase 3 の SPEC-03（CI 証拠で昇格）が入るまでは、コンパイル通過を Stage 2 Blue の条件に入れない。

理由:
1. **計画段階との整合**: vision v3 の Phase B（型スケルトン生成 → cargo check）は計画時の一時検証。信号機の常時評価基準に cargo check を入れると、スケルトン段階で Blue にならない
2. **CI コスト**: Stage 2 評価のたびに `cargo check` を実行する必要が生じる。syn AST スキャンは ms 単位だが cargo check は秒〜分単位
3. **SPEC-03 との責務分離**: 「テスト/コンパイル通過 = Yellow → Blue 昇格」は SPEC-03 が定義する場所。Stage 2 に先取りで入れると SPEC-03 導入時に基準が衝突する

## Rejected Alternatives

- Blue 条件に cargo check 通過を追加: 計画段階で Blue になれない。CI コスト増。SPEC-03 と責務が重複

## Consequences

- Good: 計画段階の型スケルトンでも Blue にでき、Phase B ワークフローと整合
- Good: Stage 2 評価が高速（syn AST のみ）
- Bad: `todo!()` やコンパイルエラーを含むコードでも Blue になりうる（信号の信頼度の限界）

## Reassess When

- Phase 3 SPEC-03（CI 証拠で昇格）導入時に、コンパイル/テスト通過を含めた統合基準を再定義
