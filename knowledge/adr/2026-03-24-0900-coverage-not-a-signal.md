---
adr_id: 2026-03-24-0900-coverage-not-a-signal
decisions:
  - id: 2026-03-24-0900-coverage-not-a-signal_grandfathered
    status: accepted
    grandfathered: true
---
# Coverage は信号機ではなく CI ゲートとする

## Status

Accepted

## Context

CC-SDD-01（要件-タスク双方向トレーサビリティ）の coverage メトリクスを、既存の信号機（Stage 1: spec signals, Stage 2: domain state signals）と同列の Stage 3 信号として定義するか検討した。

## Decision

coverage は信号機に組み込まず、`sotp verify spec-coverage` の CI ゲート（error）として実装する。

理由:
1. **測定軸が異なる**: 信号機は「確からしさ」の段階評価。coverage は「紐付きの有無」で二値。軸が違うものに同じ色を使うと Blue の意味が文脈依存になり認知負荷が高まる
2. **3 段階の意味がない**: source confidence は文書/議論/推測、domain state は型+遷移/型のみ/型なし、と中間状態に意味がある。coverage の Yellow に意味のある中間状態がない
3. **信号機の数**: 既に 2 種類。3 種類目を追加すると spec.md/plan.md の視覚ノイズが増える
4. **CI error で十分**: coverage 100% 必須を CI で強制するため、信号機による段階評価は不要

## Rejected Alternatives

- Stage 3 信号として独立定義: 認知負荷が高く、二値メトリクスに 3 段階は不自然
- Stage 1 に統合（task_refs なし = Yellow 降格）: 「出典の確からしさ」と「タスク紐付き」が混在し、信号の意味が不明瞭に

## Consequences

- Good: 信号機は「品質の段階評価」、CI ゲートは「構造的整合性チェック」で役割が明確に分離
- Good: 信号機が 2 種類のまま維持され認知負荷が増えない
- Bad: coverage 状況が spec.md/plan.md の信号サマリーに表示されない（CI 出力でのみ確認可能）

## Reassess When

- 信号機以外の品質メトリクスを spec.md に可視化するダッシュボード機能を検討する場合
