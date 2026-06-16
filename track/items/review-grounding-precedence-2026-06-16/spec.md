<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 27, yellow: 0, red: 0 }
---

# ADR decision 根拠信号機: review 優先化 + grounding 値オブジェクト検証

## Goal

- [GO-01] ADR decision 根拠信号機の優先規則を「review grounding が一件でもあれば 🟡」に修正し、user_decision_ref と review_finding_ref の両方を持つ decision が誤って 🔵 に評価される現行バグを解消する。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D1]
- [GO-02] grounding 参照文字列（user_decision_ref / review_finding_ref）をドメイン値オブジェクト（newtype）で保護し、空文字列・空白のみの placeholder が Some(_) を満たして信号を誤誘発するのを型レベルで排除する。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D2]

## Scope

### In Scope
- [IN-01] classify_grounds 関数の判定順反転: review_finding_ref が user_decision_ref より優先される（両方あれば 🟡、user のみなら 🔵、いずれもなければ 🔴、grandfathered はスキップ）。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D1] [tasks: T003]
- [IN-02] grounds.rs の doc コメントにある「user 優先」記述を新しい優先規則（review 優先）に更新する。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D1] [tasks: T003]
- [IN-03] テスト test_evaluate_adr_decision_user_ref_takes_priority_over_review_ref の期待値を反転し、両方の ref を持つ decision が 🟡 を返すことを検証するテストに更新する。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D1] [tasks: T003]
- [IN-04] grounding 参照用ドメイン値オブジェクト（newtype）を domain 層に新設する。try_new で空文字列・空白のみを Err（ValidationError）として拒否し、正常値は Ok で返す。既存の InformalGroundSummary::try_new / AdrAnchor::try_new と同じ「空・空白 reject」パターンに揃える。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D2] [tasks: T001]
- [IN-05] AdrDecisionCommon の user_decision_ref / review_finding_ref フィールドを Option<String> から Option<grounding 参照 newtype> に変更する。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D2] [tasks: T002]
- [IN-06] infrastructure の DTO→domain 変換（decision_dto_to_entry / AdrDecisionCommon 構築）で値オブジェクトの構築エラーを AdrFrontMatterCodecError::InvalidDecisionField として伝播させる。空・未記入 placeholder が slip-through するのを防ぐ fail-closed 設計（CN-01）。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D2] [tasks: T004]
- [IN-07] knowledge/conventions/adr.md の YAML front-matter grounds 表の review_finding_ref 行を「review あり → 🟡（user_decision_ref の有無を問わず）」に更新する（D1 の Consequences に明記された convention 更新義務）。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D1] [tasks: T005]
- [IN-08] 旧 ADR 2026-04-27-1234-adr-decision-traceability-lifecycle.md の D1 front-matter を status: superseded に遷移させ、superseded_by: 2026-06-16-0042-adr-signal-review-grounding-precedence.md#D1 を付帯する。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D3] [tasks: T006]

### Out of Scope
- [OS-01] D4 の対象である 6 件の既存 both-ref decision（2026-05-29-1118: D1/D2/D3/D4、2026-05-18-1223: D3、2026-05-26-1813: D6）に対する per-decision ユーザー確認・grounding 付与は、このトラックの code/artifact 変更に含めない。D1 の優先規則反転により当該 decision は信号上 🔵→🟡 に変わるが、🟡 は CI を block しないため実装上の問題にはならない。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D4]
- [OS-02] grounding 参照 newtype を user / review で共有する単一型にするか、UserDecisionRef / ReviewFindingRef の 2 型に分けるかの比較検討は Phase 2 type-designer で解決済みであり、本 spec では追加の分割型設計タスクを扱わない。Phase 2 の型カタログでは、共通の非空・非空白バリデーション規則に基づき単一の DecisionGroundRef を採用する。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D2]

## Constraints
- [CN-01] 空文字列・空白のみの grounding 参照は None への silent normalization を行わず、fail-closed の構築時エラー（Err）として報告しなければならない。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D2] [tasks: T001, T004]
- [CN-02] 信号機の 3 値モデル（🔵🟡🔴）は不変であり、新たな色・バリアントを追加しない。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D1] [tasks: T003]
- [CN-03] grandfathered: true の decision は引き続き評価スキップ（最上位スキップ）とし、D1 の優先規則反転後もこの挙動を変更しない。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D1] [tasks: T003]
- [CN-04] grounding 参照 newtype は domain 層に配置し、DTO との変換（YAML→domain）は infrastructure 層で行う。domain 型に YAML/serde の知識を持ち込まない（ヘキサゴナルアーキテクチャ原則）。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D2] [tasks: T001, T002, T004]
- [CN-05] 旧 ADR 2026-04-27-1234 の D2 / D3 / D4 は本トラックの supersede 対象外であり、これらの decision は有効のまま変更しない。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D3] [tasks: T006]

## Acceptance Criteria
- [ ] [AC-01] user_decision_ref と review_finding_ref の両方を持つ decision が classify_grounds により 🟡 を返す（旧挙動は 🔵、D1 反転の核心）。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D1] [tasks: T003]
- [ ] [AC-02] user_decision_ref のみを持ち review_finding_ref を持たない decision が classify_grounds により 🔵 を返す。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D1] [tasks: T003]
- [ ] [AC-03] いずれの grounding ref も持たない decision が classify_grounds により 🔴 を返す。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D1] [tasks: T003]
- [ ] [AC-04] grandfathered: true の decision が classify_grounds の評価をスキップされる（信号色を返さず除外される）。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D1] [tasks: T003]
- [ ] [AC-05] 空文字列（""）を渡して grounding 参照 newtype を構築しようとすると Err（ValidationError）が返る。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D2] [tasks: T001]
- [ ] [AC-06] 空白のみの文字列（"  " 等）を渡して grounding 参照 newtype を構築しようとすると Err（ValidationError）が返る。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D2] [tasks: T001]
- [ ] [AC-07] 正常な非空・非空白の grounding 参照文字列（例: "chat_segment:2026-06-16"）を渡して grounding 参照 newtype を構築すると Ok が返る。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D2] [tasks: T001]
- [ ] [AC-08] bin/sotp verify adr-signals が空 placeholder（Some("") 相当）を user_decision_ref / review_finding_ref に持つ ADR ファイルを InvalidDecisionField エラーとして fail する（fail-closed, CN-01）。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D2] [tasks: T004]
- [ ] [AC-09] 旧 ADR 2026-04-27-1234-adr-decision-traceability-lifecycle.md の D1 front-matter の status フィールドが superseded であり、superseded_by が 2026-06-16-0042-adr-signal-review-grounding-precedence.md#D1 を指している。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D3] [tasks: T006]
- [ ] [AC-10] knowledge/conventions/adr.md の grounds 表の review_finding_ref 行が「review あり → 🟡（user_decision_ref の有無を問わず）」を記述しており、旧「user_decision_ref 未設定なら 🟡」という文言が残っていない。 [adr: knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md#D1] [tasks: T005]

## Related Conventions (Required Reading)
- knowledge/conventions/adr.md#YAML front-matter
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/source-attribution.md#Source Tag Types
- knowledge/conventions/pre-track-adr-authoring.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 27  🟡 0  🔴 0

