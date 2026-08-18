---
adr_id: "2026-07-29-0839-catalogue-generic-type-alias"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:sotohe-issues-discussion:2026-07-29"
    status: proposed
---
# TDDD catalogue でジェネリクスパラメータ付き型エイリアスを宣言可能にする

## Context

TDDD の `<layer>-types.json` は alias kind の型契約を宣言できるが、ジェネリクスパラメータ付きエイリアス（`type Foo<T> = ...`）を表現できない。Rust 側では generic type alias は安定機能であり、実装で自然に使いたい形が catalogue で宣言できないため、設計表現が非ジェネリクスな別形へ歪む。

## Decision

### D1: alias kind でジェネリクスパラメータ宣言を可能にする

catalogue schema・catalogue-lint・chain ③（catalogue ↔ implementation 照合）をジェネリクスパラメータ付き alias に対応させる。chain ③ は字句照合ベースであり、型パラメータの表記揺れが従来の `Self` vs 具体型名と同型の mismatch を生みうるため、パラメータ表記の正規形（catalogue 側の宣言表記に実装を揃える）を対応の一部として定義する。

## Consequences

- 良: 型設計の表現力が実装言語と揃い、catalogue 起因の設計歪みが減る。
- 中立: 後方互換性の広い確認は不要 — chain ③ はアクティブ track に対してのみ評価され、完了 track の catalogue が再評価されることはない。拡張は追加的（optional フィールド）であり、ジェネリクス未宣言の既存 entry の評価は不変。
- 負: 検証ポイントは一点のみ — 実装側に既存する generic type alias を chain ③ が現在どう扱っているかを確認し、正規形ルールの導入がその挙動を変えない（拡張の追加性が成立する）ことを保証する。移行時点で進行中の track は sotp 再ビルド後に新ロジックを踏むが、追加性が成立していれば評価は変わらない。

## Reassess When

- 照合の正規形ルールが複雑化し、字句照合の限界（構文解釈が必要）に達したとき。
