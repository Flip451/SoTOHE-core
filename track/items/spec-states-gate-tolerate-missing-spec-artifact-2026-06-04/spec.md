<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 22, yellow: 0, red: 0 }
---

# spec-states commit ゲートを spec 成果物未生成の段階でも通す

## Goal

- [GO-01] `verify spec-states`（spec パス引数なし、ブランチからトラックを自動解決する経路）が、spec 成果物（spec.json / spec.md のいずれも）が存在しない Phase 0 の段階では評価を skip（no-op + success、SKIP 表示）し、非ゼロ終了しないようにする。これにより commit ゲート（`cargo make ci` 経由の `verify-spec-states-current`）が Phase 0 でも通り、「init 直後に review → commit で ADR を初回 commit する」標準フローが機能する [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1]

## Scope

### In Scope
- [IN-01] トラック解決経路（spec パス未指定で、ブランチから対象トラックを解決する経路）において、spec.json / spec.md のいずれも存在しない場合に skip（no-op + success）を返す挙動の実装を対象とする [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [IN-02] spec 成果物が存在するフェーズ（Phase 1 以降）では、従来どおりシグナルを評価する。🔴 はゲートを block し、CI 中間モードでは 🟡 は warning、merge ゲートの strict モードでは 🟡 も block するという既存の使い分けを不変に保つ [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [IN-03] skip 判定の精度担保を対象とする。skip は「spec.json / spec.md のいずれも存在しない」ことを実ファイル存在で厳密に判定した場合にのみ発動し、成果物がある状態での fail-open を作らない [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [IN-04] skip 時のユーザー向け出力（SKIP 表示）を対象とする。skip した旨が観測できる出力を標準出力または標準エラーに表示し、silent no-op にしない [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [IN-05] 新しい skip 分岐のテストカバレッジを対象とする。spec 成果物が存在しない場合に skip を返すことを確認するテストを追加する [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]

### Out of Scope
- [OS-01] 明示的 spec パス指定経路（`verify spec-states <path>`）の挙動変更は対象外とする。この経路は従来どおり当該ファイルを検証し、ファイルが存在しなければエラーとする [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1]
- [OS-02] シグナル評価セマンティクス（🔵🟡🔴 の評価ルール）の変更は対象外とする。spec 成果物が存在するフェーズでは評価ロジックをそのまま維持する [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1]
- [OS-03] 他の verify ゲート（`verify-plan-artifact-refs`、`verify-catalogue-spec-refs`、`verify-latest-track`、`check-catalogue-spec-signals`）の挙動変更は対象外とする [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1]
- [OS-04] commit ゲートから `verify-spec-states-current` を丸ごと外す案は対象外とする。ゲート自体を bypass すると spec 成果物が揃った後のフェーズでもシグナル評価が走らず fail-open になる。ADR Rejected Alternative A として記録されている [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1]
- [OS-05] Phase 0 専用の別 commit 経路の新設は対象外とする。兄弟チェックと同じ「入力不在なら skip」という一様なルールで足りるため、フェーズ別の経路を増やすのは不要な複雑化。ADR Rejected Alternative B として記録されている [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1]
- [OS-06] トラック自動解決経路の廃止（spec パスを常に必須引数にする案）は対象外とする。他の兄弟チェックはトラック自動解決 + 入力不在 skip で揃っており、spec-states だけ設計を変えると一貫性が崩れる。ADR Rejected Alternative C として記録されている [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1]
- [OS-07] skip 判定の具体的な実装位置（`build_spec_path_from_track_id` 関数の変更箇所、`dispatch_spec_states_with_resolver` のテスト対象など）の特定は対象外とする。これらは Phase 2 / 3 の関心事であり、本 spec は振る舞い契約のみを記述する [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1]

## Constraints
- [CN-01] skip は「spec.json / spec.md のいずれも実在しない」ことを厳密に判定した場合にのみ発動する。どちらか一方でも存在すれば通常評価を行い、fail-open を作らない [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [CN-02] skip 挙動は、同じ commit ゲートで既に欠損入力を skip している兄弟チェック（`verify-latest-track`、`verify-plan-artifact-refs`、`verify-catalogue-spec-refs`、`check-catalogue-spec-signals`）の寛容さに揃える。呼び出し経路（gate 経由か直接か）によって挙動が変わらない [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [CN-03] spec 成果物が存在するフェーズ（Phase 1 以降）では、シグナル評価の厳格性を従来どおり維持する。🔴 は引き続きゲートを block し、CI 中間モードと strict モードの使い分けも不変 [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [CN-04] `cargo make ci`（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する状態を維持する [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]

## Acceptance Criteria
- [ ] [AC-01] `verify spec-states`（spec パス引数なし）が、spec.json / spec.md のいずれも存在しない状態（Phase 0）でゼロ終了し、SKIP を示す出力が観測できる [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [ ] [AC-02] `verify spec-states`（spec パス引数なし）が、spec.json / spec.md が存在する状態（Phase 1 以降）では従来どおりシグナルを評価し、🔴 の場合に非ゼロ終了する（skip による fail-open が発生していない） [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [ ] [AC-03] `verify spec-states <path>`（明示的 spec パス指定経路）の挙動が変わらない。spec パスが存在しない場合にエラーとなる従来動作を維持する [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [ ] [AC-04] spec 成果物が存在しない場合に skip を返すことを確認するテストが追加されており、`cargo make test` でパスする [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [ ] [AC-05] `cargo make ci` が pass する（fmt-check + clippy + nextest + deny + check-layers + verify-* の全ステップ） [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]

## Related Conventions (Required Reading)
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- .claude/rules/04-coding-principles.md#No Panics in Library Code

## Signal Summary

### Stage 1: Spec Signals
🔵 22  🟡 0  🔴 0

