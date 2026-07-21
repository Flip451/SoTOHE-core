<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 28, yellow: 0, red: 0 }
---

# 同梱運用ドキュメントのアーキテクチャ記述 SSoT 再編

## Goal

- [GO-01] 同梱運用ドキュメントのアーキテクチャ記述を、機械可読な層依存ルールと role × layer 配置規則を正本とする参照構造へ再編し、散文による重複記述起因のドリフトを除去する [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D1]
- [GO-02] 監査で確認された High / Medium の運用ドキュメント不整合を解消し、文書・テンプレート・CI が現行の規約と fail-closed 方針に整合する状態にする [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D1, knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D2, knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D3, knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D4, knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D5, knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D6, knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D7, knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D8, knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D9]

## Scope

### In Scope
- [IN-01] hexagonal-architecture.md を廃止し、crate 間依存の正本を architecture-rules.json と deny.toml、role × layer 配置（ポート配置を含む）の正本を type-designer-kind-selection.md の R1 マトリクスへ一本化する [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D1] [tasks: T001]
- [IN-02] 廃止文書の残余内容のうち usecase purity 規則だけを coding-principles.md へ移設し、trait-based abstraction の例と async 採用 Note は廃棄する [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D2] [tasks: T001]
- [IN-03] 廃止する architecture convention への現行参照を新しい SSoT または purity 規則の移設先へ付け替え、アーキテクチャ変更時の文書更新チェックリストを適用後の文書集合と同期させる [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D3] [tasks: T003]
- [IN-04] ポート配置に言及する文書と capability architecture guard を R1 の domain / usecase の二層配置へ統一し、R1 に配置判別の tie-break 基準と境界例の分類を加える [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D4] [tasks: T002]
- [IN-05] ValueObject 規則を、値自身から新しい値または述語を導出する side-effect-free なメソッドを許容し、依存または外部リソースを扱う service 的な振る舞いを禁止対象とする内容へ是正する [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D5] [tasks: T002]
- [IN-06] DRY 修正の規範を、意図的な core 型と adapter ミラー DTO / enum の構造類似を統合対象外とし、正当な cross-layer 共通化は依存可能な内側の層へ抽出する内容へ是正する [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D6] [tasks: T002]
- [IN-07] 入口ドキュメントの層表記を依存方向が一意に読める形へ統一し、delivery crate の依存関係を明示する [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D7] [tasks: T003]
- [IN-08] overlay と maintainer 側の ADR テンプレートを、adr_id と decision 単位の根拠・status を持つ現行 front-matter 形式へ整合させる [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D8] [tasks: T004]
- [IN-09] track-aware CI gate を develop から main への release PR だけ明示的に免除し、それ以外の PR では track 文脈を fail-closed で要求し続ける [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D9] [tasks: T005]

### Out of Scope
- [OS-01] 監査で列挙された Low findings 5 件および改善提案 44 件を実装すること [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D3]
- [OS-02] Rust の実行時挙動・実装、または出荷 catalogue lint 設定を変更すること。Rust ファイルの変更は D3 の参照付け替えを行う doc comment に限る [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D3]
- [OS-03] sotp verify usecase-purity の機械強制の実装または強度を変更すること [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D2]

## Constraints
- [CN-01] アーキテクチャの依存・配置規則の判別基準・許可関係を人間可読文書で再記述せず、それぞれの権威ある SSoT への参照で表現する。D4 / D7 が要求する入口・guard の短い案内は適用層または依存先を示して当該 SSoT を参照できるが、判別基準・許可関係を追加または置換してはならない [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D1] [tasks: T001, T003]
- [CN-02] usecase purity 規則の移設は、現状 CI blocking であり違反を error finding（exit 1）として cargo make ci の verify-usecase-purity-local gate を失敗させる強制強度を consumer 向けに明記する。この track では機械強制の実装または強度を変更せず、緩和は採用者が ADR で判断する [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D2] [tasks: T001]
- [CN-03] release PR 以外の非-track PR に track-aware gate を迂回する経路を設けず、免除を develop → main の一形状に限定する [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D9] [tasks: T005]
- [CN-04] 実装フットプリントは Markdown 文書と CI 設定を中心とし、Rust への変更は doc comment の引用先付け替えに限定する [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D3] [tasks: T003]

## Acceptance Criteria
- [ ] [AC-01] hexagonal-architecture.md が存在せず、crate 間依存は architecture-rules.json と deny.toml、role × layer 配置は type-designer-kind-selection.md の R1 を正本として参照される [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D1] [tasks: T001]
- [ ] [AC-02] coding-principles.md に usecase purity 規則、Good / Bad 例、CI blocking の強制強度（error finding、exit 1、cargo make ci の verify-usecase-purity-local gate）があり、廃止文書由来の trait-based abstraction 例は存在しない。async runtime の採用は ADR の決定事項として一文で示される [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D2] [tasks: T001]
- [ ] [AC-03] 現行のレビュープロンプト、入口文書、rules、skills、Rust doc comment に廃止した architecture convention への引用が残らず、引用は該当する新 SSoT または purity 規則へ向く [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D3] [tasks: T003]
- [ ] [AC-04] architecture-customizer の更新対象と maintainer checklist が、適用後にアーキテクチャ記述を持つ文書の実集合を網羅する [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D3] [tasks: T003]
- [ ] [AC-05] ポート配置に言及する調査プロンプトと capability architecture guard が domain / usecase の二層配置を示し、R1 に domain と usecase を選ぶ tie-break 基準および境界例がある [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D4] [tasks: T002]
- [ ] [AC-06] R3 と R6 が、値等価で識別され side-effect-free な導出メソッドのみを持つ型を ValueObject として扱い、依存または外部リソースを扱う service 的 struct を ValueObject として扱わない規則を示す [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D5] [tasks: T002]
- [ ] [AC-07] dry-fix-lead と dry-check-workflow が、意図的ミラー型を DRY 違反から除外し、正当な cross-layer 共通化を依存可能な内側の層へ抽出する規範を示す [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D6] [tasks: T002]
- [ ] [AC-08] CLAUDE.md の層表記は依存方向を一意に示し、cli-driver は usecase のみ、cli-composition は全層を配線し、cli は bin であることを示す [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D7] [tasks: T003]
- [ ] [AC-09] overlay/knowledge/adr/README.md と knowledge/adr/README.md のテンプレートが file-level Status を含まず、adr_id と decision ごとの status および根拠 ref を持つ front-matter を例示する [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D8] [tasks: T004]
- [ ] [AC-10] CI の track-aware gate は develop を head、main を base とする release PR では実行されず、それ以外の PR では実行される。関連する branch 再作成 step と gate step の条件関係がコメントで明記される [adr: knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md#D9] [tasks: T005]

## Related Conventions (Required Reading)
- knowledge/conventions/no-upstream-restatement.md#Rules
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rules
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/type-designer-kind-selection.md#R1
- knowledge/conventions/dry-check-workflow.md#Rules
- knowledge/conventions/adr.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 28  🟡 0  🔴 0

