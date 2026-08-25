<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 12, yellow: 0, red: 0 }
---

# シークレット秘匿の静的正規表現を fail-closed にする

## Goal

- [GO-01] sotp が静的な秘匿パターンを利用する際、パターン不正によって秘匿機能全体が無音で無効化され、機密値が出力へ流出する状態を許さない。 [adr: knowledge/adr/2026-08-20-1053-sensitive-redaction-fail-closed.md#D1]

## Scope

### In Scope
- [IN-01] 秘匿境界で使用する全ての静的正規表現リテラルについて、構築失敗が秘匿のスキップ又は代替なしの継続につながらず、処理を停止させること。 [adr: knowledge/adr/2026-08-20-1053-sensitive-redaction-fail-closed.md#D1] [tasks: T001]
- [IN-02] 静的秘匿パターンごとに構築を検証し、production で初めて不正パターンが検出される経路を残さないこと。 [adr: knowledge/adr/2026-08-20-1053-sensitive-redaction-fail-closed.md#D1] [tasks: T001]
- [IN-03] 秘匿・検証・権限判定のセキュリティ境界では無音の機能縮退を禁止し、構築又は初期化の失敗を停止として扱う規約を記録すること。 [adr: knowledge/adr/2026-08-20-1053-sensitive-redaction-fail-closed.md#D2] [tasks: T002]

### Out of Scope
- [OS-01] operator 設定その他の外部入力で動的に供給される秘匿パターンの設計又は失敗時の扱いは対象外とする。 [adr: knowledge/adr/2026-08-20-1053-sensitive-redaction-fail-closed.md#D1] [tasks: T001]
- [OS-02] 起動時に静的パターンを一括検証するための新しいコマンド又は実行面は追加しない。 [adr: knowledge/adr/2026-08-20-1053-sensitive-redaction-fail-closed.md#D1] [tasks: T001]

## Constraints
- [CN-01] 静的秘匿パターンの構築不能はプログラミングエラーとして fail-stop とし、警告のみの通知、無効値への縮退、又は秘匿なしでの出力継続を許さない。 [adr: knowledge/adr/2026-08-20-1053-sensitive-redaction-fail-closed.md#D1] [tasks: T001]
- [CN-02] この変更は静的な秘匿境界パターンに限定し、追加される同種の静的リテラルにも同一の fail-closed 構築保証を適用する。 [adr: knowledge/adr/2026-08-20-1053-sensitive-redaction-fail-closed.md#D1] [tasks: T001]

## Acceptance Criteria
- [ ] [AC-01] 不正な静的秘匿パターンは構築検証で検出され、秘匿を無音で無効化したまま出力処理を続行しない。 [adr: knowledge/adr/2026-08-20-1053-sensitive-redaction-fail-closed.md#D1] [tasks: T001]
- [ ] [AC-02] 有効な静的秘匿パターンを用いる出力経路では、対象となる機密値が従来どおり秘匿される。 [adr: knowledge/adr/2026-08-20-1053-sensitive-redaction-fail-closed.md#D1] [tasks: T001]
- [ ] [AC-03] security 規約は、秘匿・検証・権限判定の各セキュリティ境界で無音の機能縮退を禁止し、構築又は初期化の失敗を停止させる方針を明記する。 [adr: knowledge/adr/2026-08-20-1053-sensitive-redaction-fail-closed.md#D2] [tasks: T002]
- [ ] [AC-04] 静的秘匿パターンのためだけに、新しい起動時一括検証コマンド又は実行面を追加しない。 [adr: knowledge/adr/2026-08-20-1053-sensitive-redaction-fail-closed.md#D1] [tasks: T001]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 12  🟡 0  🔴 0

