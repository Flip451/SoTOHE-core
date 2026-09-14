<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 16, yellow: 0, red: 0 }
---

# テスト義務検証の仕様区分・対象責務・鮮度の整合性修復

## Goal

- [GO-01] 履行検証器と免除検証器が、仕様要素の識別子・所属区分・本文、対象宣言が所有する責務、および完全な正規化済み判定要求を一貫して用い、対象外の意味反転、責務越境、意味変更後の旧判定再利用を防ぐ。 [adr: knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D1, knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D3, knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D4]

## Scope

### In Scope
- [IN-01] 両検証器へ渡す各仕様要素の意味入力で、既存の構造化仕様モデルを正として識別子・所属区分・本文を保持する。所属区分を識別子接頭辞や本文から推測せず、対象外の内容を肯定的な実装要求として表示しない。 [adr: knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D1, knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D2] [tasks: T001, T002, T003, T004, T005, T006, T007]
- [IN-02] 対象外を、実装義務を生まない範囲境界、非保証、対象責務に属する明示禁止として区別して解釈する。対象外の参照関係を一括除外・自動免除せず、材料不足は既存の保留経路で扱う。 [adr: knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D2] [tasks: T005, T006, T007]
- [IN-03] 履行・免除の判定を、義務の対象である宣言項目が所有する参照先仕様の振る舞いに限定する。責務越境の報告は、実際の判定入力に届く宣言・義務の同一性と説明・テスト本文または免除理由を照合して診断し、確認された入力欠落だけを補う。 [adr: knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D3] [tasks: T001, T002, T003, T004, T005, T006, T007]
- [IN-04] 評価、確認、結果表示が同じ正規化済み判定要求全体から意味入力の鮮度を決めるようにする。仕様区分・対象責務を含む可変の意味入力、要求構造、または判定意味論に関わる入力形式の改訂は、旧合否を再利用不能にする。 [adr: knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D4] [tasks: T001, T002, T003, T004, T005, T006, T008]
- [IN-05] 入力構築・検証器指示・鮮度判定について決定的な回帰検証を追加し、通常のビルド・更新経路で、既存キャッシュを残したまま修正後の評価と確認を行う。 [adr: knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D5] [tasks: T002, T003, T004, T005, T006, T008, T009]

### Out of Scope
- [OUT-01] 参照関係を評価対象から除外・自動免除して対象外の問題を回避すること、または対象の責務を増やして既存の判定に合わせること。 [adr: knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D2, knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D3] [tasks: T001, T002, T003, T005, T006, T007]
- [OUT-02] 入力不足の確認前に外部文脈取得を追加すること、またはリポジトリ・上流成果物を都度探索して責務や鮮度の意味入力を推定すること。 [adr: knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D3, knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D4] [tasks: T001, T002, T003, T004, T008]
- [OUT-03] キャッシュの削除・手編集、ゲート緩和、または同一条件で合格するまでの反復を、判定更新または修正確認の経路にすること。 [adr: knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D4, knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D5] [tasks: T004, T008, T009]
- [OUT-04] モデル、提供者、実行段階を検証器指紋の構成要素へ追加すること。 [adr: knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D4] [tasks: T001, T002, T005, T006, T008]

## Constraints
- [CN-01] 旧形式、指紋欠如、または指紋不一致の判定は、合否にかかわらず再利用せず、既存のタスク状態に応じた未確定判定へ戻す。鮮度の回復は新しい入力での再評価だけで行う。 [adr: knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D4] [tasks: T001, T002, T003, T004, T005, T006, T008, T009]
- [CN-02] 正規化済み判定要求に含まれる対象宣言・義務・参照先仕様・テスト本文または免除理由を照合し、対象宣言の振る舞いが対象責務の約束と矛盾する場合、テスト本文または免除理由が対象責務とは別の振る舞いだけを扱う場合、または対象義務の説明が直接要求する振る舞いに対応する証拠がない場合は不合格とする。対象宣言と義務の同一性、参照先仕様の所属・本文、またはテスト本文・免除理由が判定要求に不足して照合できない場合は保留として扱う。 [adr: knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D2, knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D3, knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D5] [tasks: T001, T002, T003, T005, T006, T007]

## Acceptance Criteria
- [ ] [AC-01] 決定的な検証で、goal・constraints・acceptance_criteria・in_scope・out_of_scope の各仕様要素について、識別子・所属区分・本文が両検証器の実入力に保持され、同一本文が対象範囲と対象外のどちらに属するかを区別できることを確認できる。さらに、対象外の名詞句を肯定的な実装要求へ反転させず、単なる非保証から否定要件を作らない一方、対象責務に属する明示的な禁止は検証対象として扱うことを確認できる。 [adr: knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D1, knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D2, knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D5] [tasks: T001, T002, T003, T004, T005, T006, T007]
- [ ] [AC-02] 決定的な検証で、単一入力を扱う対象宣言と入力選択を所有する宣言を区別し、前者に呼び出し元の選択・設定読込責務を要求せず、後者の選択欠陥は検出対象に保つことを確認できる。 [adr: knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D3, knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D5] [tasks: T003, T005, T006, T007]
- [ ] [AC-03] 仕様区分の移動、対象責務情報の変更、判定要求の構造変更、または検証器入力形式・意味論の改訂について、評価・確認・結果表示が旧合格・旧不合格を同じように失効させ、再評価後の状態で一致することを確認できる。 [adr: knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D4, knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D5] [tasks: T001, T002, T003, T004, T005, T006, T008]
- [ ] [AC-04] 通常のビルド・更新経路で修正版を評価・確認するとき、既存キャッシュを削除または手編集せずに、新しい判定入力に対する判定で鮮度を回復できる。 [adr: knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D4, knowledge/adr/2026-09-11-1320-test-obligation-verifier-context-integrity.md#D5] [tasks: T003, T004, T005, T006, T008, T009]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 16  🟡 0  🔴 0

