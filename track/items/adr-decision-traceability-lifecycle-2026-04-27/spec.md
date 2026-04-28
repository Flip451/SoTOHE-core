<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 30, yellow: 0, red: 0 }
---

# ADR decision の根拠 trace 信号機評価 + 個別 lifecycle 管理

## Goal

- [GO-01] ADR ファイルに YAML front-matter ブロック (adr_id + decisions[] 配列) を追加し、各 decision に user_decision_ref / review_finding_ref / status / grandfathered 等のフィールドを machine-parseable な形式で encode することで、orchestrator 独断 decision を機械的に検出できる仕組みの基盤を確立する [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D3, knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D1]
- [GO-02] bin/sotp verify adr-signals サブコマンドを実装し、ADR front-matter の decisions[] を解析して各 decision の根拠 trace フィールド (user_decision_ref / review_finding_ref) の有無から 🔵🟡🔴 信号を評価し、🔴 (根拠なし) かつ grandfathered: true でない decision を CI block 対象とすることで、briefing 経由の未承認 decision 混入を構造的に検出する [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D1]
- [GO-03] ADR 各 decision に proposed / accepted / implemented / superseded / deprecated の個別 status を front-matter で管理し、partial supersession・implementation tracking・deprecation tracking を 1 ファイル内で表現できる lifecycle 管理能力を提供する [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D2]
- [GO-04] 既存 ADR への一括強制 CI block を避けるため、既存 ADR の decision に grandfathered: true を付けて verify をスキップする段階的 back-fill 経路を確立し、新規 ADR はすべて front-matter 必須とする 2-speed 移行を可能にする [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D4]

## Scope

### In Scope
- [IN-01] ADR YAML front-matter スキーマの定義: adr_id / decisions[] 配列 (id / user_decision_ref / review_finding_ref / candidate_selection / status / superseded_by / implemented_in / grandfathered の各フィールド) を ADR フォーマット標準として確立し、knowledge/conventions/adr.md を更新する [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D3] [tasks: T001, T002, T011]
- [IN-02] bin/sotp verify adr-signals サブコマンドの実装: knowledge/adr/*.md を走査し、front-matter を YAML パースして decisions[] の根拠フィールドから 🔵🟡🔴 を評価する。grandfathered: true の decision はスキップ。🔴 が 1 件以上ある場合は non-zero exit で CI block する [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D1] [tasks: T003, T004, T005, T006]
- [IN-03] cargo make ci への verify-adr-signals task の追加: 既存 verify-* タスク群 (verify-plan-artifact-refs 等) と並列実行可能な形で組み込み、新規 ADR が front-matter 必須を満たしているかを CI 経路で常時チェックする [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D1] [tasks: T007]
- [IN-04] 本トラック自身の ADR (2026-04-27-1234-adr-decision-traceability-lifecycle.md) への YAML front-matter 追加: D1-D4 のすべての decision を front-matter で encode し、本 ADR が新フォーマットに準拠した最初の例として機能させる [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D3, knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D4] [tasks: T008]
- [IN-05] adr-editor への front-matter 記述義務付け: adr-editor agent が ADR を新規作成・更新する際に front-matter を付与するよう .claude/agents/adr-editor.md の rules を更新する [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D3, knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D1] [tasks: T010]
- [IN-06] 既存 ADR への grandfathered: true 付与: 本 ADR 採択前に作成された既存 ADR に一括で grandfathered: true を含む minimal front-matter を追加し、verify-adr-signals の CI block を回避しながら段階的な back-fill を可能にする [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D4] [tasks: T009]
- [IN-07] domain 層への AdrSignalEvaluator 純粋関数の実装: decision の user_decision_ref / review_finding_ref / grandfathered フィールドから 🔵🟡🔴 を評価する関数を domain 層に配置し、usecase 層の interactor から呼び出せる形にする [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D1] [tasks: T001, T002]

### Out of Scope
- [OS-01] 既存 ADR の back-fill 完了 (grandfathered: true の解除と根拠 trace フィールドの記入): back-fill 作業は各 ADR の decision 根拠をさかのぼる調査が必要であり、本 track では grandfathered: true の一括付与のみを行い、back-fill 自体は別 track / 別作業として段階的に実施する [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D4]
- [OS-02] front-matter の decision status を利用した高度な lifecycle クエリ CLI (例: implemented 状態の decision 一覧表示、superseded チェーンの追跡): verify-adr-signals の基本信号評価に集中し、status 活用の詳細クエリ機能は将来の track で追加する [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D2]
- [OS-03] ADR front-matter の GitHub Rendered View 改善 (front-matter を人間が読みやすい形で GitHub 上に表示する仕組み): GitHub が YAML front-matter をレンダリング時に隠す既知の制限への対応は別途検討する [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D3]
- [OS-04] briefing 機械生成 (WF-67 Phase B): orchestrator の briefing を machine-generated / static template 化する WF-67 側の対策は別トラックで扱う。本 track は ADR 側の safety net (verify-adr-signals) のみを担当する [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D1]
- [OS-05] pre-track-adr-authoring.md の rules セクション更新以外の既存 convention / command ファイルの大規模改訂: 本 track は adr-editor.md および adr.md のみを更新対象とし、他の convention / command ファイルへの波及的修正は行わない [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D3]

## Constraints
- [CN-01] ADR MD body (## Context / ## Decision / ## Rejected Alternatives / ## Consequences / ## Reassess When / ## Related セクション) は一切変更しない。YAML front-matter の追加のみが許容される変更であり、narrative の書き換えや削除は禁止する [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D3] [tasks: T008, T009]
- [CN-02] knowledge/conventions/pre-track-adr-authoring.md の「ADR に Status 見出しや approved のような状態フィールドは作らない」ルールとの整合を保つ。YAML front-matter の decisions[].status は decision 個別の lifecycle フィールドであり、ADR ファイル全体の承認状態フィールド (## Status セクション等) とは別 axis であることを convention 更新の文書で明示する [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D2] [tasks: T011]
- [CN-03] grandfathered: true を付けた decision は verify-adr-signals の CI block 対象から除外する (exit 0 でスキップ)。grandfathered 解除は強制しないが、front-matter 追加後に back-fill を行う際の経路として明記する [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D4] [tasks: T003, T005, T009]
- [CN-04] verify-adr-signals は fail-closed で動作する: front-matter が存在しない ADR ファイルが knowledge/adr/ に存在する場合、grandfathered: true 相当の扱いをするか non-zero exit にするかの振る舞いを ADR の D4 趣旨に沿って明確化し、新規 ADR が front-matter 未記述のまま通過しない設計とする [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D1, knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D4] [tasks: T003, T011]
- [CN-05] AdrSignalEvaluator 純粋関数を domain 層に配置し、usecase 層が I/O 責務 (ADR ファイル読み込み) を担い、infrastructure 層がファイルシステムアクセスを実装する。hexagonal architecture の layer dependency 規則に準拠する [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D1] [tasks: T001, T002, T003, T004, T005]
- [CN-06] workflow-ceremony-minimization.md の「人工的な状態フィールドを作らない」原則と本 track の decision 個別 status が衝突しないことを、front-matter 更新の文書で説明する。decision 個別 status は機械検証可能 (verify-adr-signals が status を評価できる) であり、形骸化する file-level summary とは異なる [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D2] [tasks: T011]

## Acceptance Criteria
- [ ] [AC-01] bin/sotp verify adr-signals コマンドが実装されており、knowledge/adr/ 配下の全 ADR ファイルを走査して decisions[] の根拠フィールドを評価できる。🔴 (user_decision_ref も review_finding_ref もなく grandfathered: true でもない) decision が 1 件以上ある場合は non-zero exit で stderr にエラーを出力する。🔴 ゼロの場合は exit 0 [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D1] [tasks: T006]
- [ ] [AC-02] cargo make ci に verify-adr-signals task が追加されており、CI 実行時に verify-adr-signals が pass する。本 track の ADR (2026-04-27-1234) は front-matter 付きで 🔵 または 🟡 となり block されない [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D1, knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D4] [tasks: T007, T008]
- [ ] [AC-03] knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md に YAML front-matter が追加されており、D1-D4 の全 decision が decisions[] 配列に encode されている。各 decision は user_decision_ref または review_finding_ref を持ち 🔵 または 🟡 に評価される [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D3] [tasks: T008]
- [ ] [AC-04] 既存の ADR (本 ADR 採択前に作成された knowledge/adr/*.md) に grandfathered: true を含む minimal front-matter が付与されており、verify-adr-signals を実行しても既存 ADR 由来の CI block が発生しない [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D4] [tasks: T009]
- [ ] [AC-05] .claude/agents/adr-editor.md の rules に front-matter 記述義務が追加されており、adr-editor が新規 ADR 作成時に YAML front-matter (adr_id + decisions[] の各フィールド) を含むファイルを生成するよう指示されている [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D3, knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D1] [tasks: T010]
- [ ] [AC-06] knowledge/conventions/adr.md に YAML front-matter フォーマット (adr_id / decisions[] の全フィールド) が記載されており、決定ステータスの軸 (ファイル全体 vs decision 個別) の違いが説明されている。pre-track-adr-authoring.md との整合が明示的に記述されている [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D3, knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D2] [tasks: T011]
- [ ] [AC-07] verify-adr-signals が front-matter のない ADR を新規 ADR と見なして fail-closed で処理する振る舞い (または front-matter なしでも grandfathered 扱いにする振る舞い) が実装されており、その振る舞いが adr.md convention に記述されている [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D1, knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D4] [tasks: T003, T011]
- [ ] [AC-08] cargo make ci の全項目 (fmt-check + clippy + nextest + test-doc + deny + check-layers + verify-*) が pass する。verify-adr-signals を含む新規 task が CI gate に組み込まれており、既存 ADR の grandfathered 付与後に CI が通過する [adr: knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D1, knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md#D4] [tasks: T012]

## Related Conventions (Required Reading)
- knowledge/conventions/adr.md#Format
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/05-testing.md#Test Structure

## Signal Summary

### Stage 1: Spec Signals
🔵 30  🟡 0  🔴 0

