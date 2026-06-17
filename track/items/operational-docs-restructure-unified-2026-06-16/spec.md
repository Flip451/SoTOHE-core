<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 48, yellow: 0, red: 0 }
---

# 運用ドキュメント再編（統合版）— ルート文書一本化・track/workflow.md 分散・工学規約の conventions 移管

## Goal

- [GO-01] リポジトリの運用ドキュメント群（ルート直下の人間向け文書・track/workflow.md・.claude/rules/04/05/06）が抱える重複・陳腐化・SoT 方向逆転を解消し、README.md（人間向け入口）・knowledge/conventions/（工学規約の正本）・コマンド定義（フローの正本）に参照方向を一本化する [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D1]
- [GO-02] 自動ロード文書（CLAUDE.md・残置 .claude/rules）が再編後の最終状態（削除・移管・新設された文書）を正確に反映し、セッション文脈コストと情報誤案内を最小化する [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D10]

## Scope

### In Scope
- [IN-01] README.md を GitHub ランディングページ兼唯一の人間向け入口として増強する。SoTOHE の価値説明（SoT Chain・信号機・track モデル）と最小の使い方（前提条件、/adr:add → /track:adr2pr の正規フロー）を含め、DEVELOPER_AI_WORKFLOW.md の固有価値（前提条件・自由文依頼例）を吸収する [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D1] [tasks: T008]
- [IN-02] START_HERE_HUMAN.md と LOCAL_DEVELOPMENT.md を削除する [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D1] [tasks: T001]
- [IN-03] DEVELOPER_AI_WORKFLOW.md を削除する。phase 構成・コマンド一覧・標準フロー等の重複内容は移送せず削除する。ブランチ運用の節は新設する knowledge/conventions/branch-strategy.md に集約される [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D2] [tasks: T008]
- [IN-04] track/workflow.md を削除する。Guiding Principles（9 項目削除・2 項目追記）、重複する phase 構成・Task Workflow・Track Commands リスト・Mermaid Diagram Convention は移送せず削除する [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D3] [tasks: T007]
- [IN-05] knowledge/conventions/branch-strategy.md を新設し、track/workflow.md の Branch Strategy + ガードポリシーの固有内容を移送する [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D3] [tasks: T003]
- [IN-06] knowledge/conventions/track-lifecycle.md を新設し、track/workflow.md の plan.md と metadata.json SSoT + observations.md + registry.md 更新ルールの固有内容を移送する [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D3] [tasks: T003]
- [IN-07] knowledge/conventions/git-notes.md を新設し、track/workflow.md の Git Notes の固有内容を移送する [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D3] [tasks: T003]
- [IN-08] track/workflow.md を参照している箇所（.claude/commands/track/setup.md・.claude/rules/08/09/10・CLAUDE.md）を新設 3 convention に張り替える。Operational split 記述は conventions が「day-to-day workflow rules」を持つことを明記する形に更新する [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D3] [tasks: T007]
- [IN-09] .claude/rules/04-coding-principles.md の内容を knowledge/conventions/ 配下の正式 convention として移管し、旧ファイルを削除する（ポインタ stub も残さない）。移管先は既存 conventions 構成に合わせた分割構成とする [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D4] [tasks: T004]
- [IN-10] .claude/rules/05-testing.md の内容を knowledge/conventions/ 配下の正式 convention として移管し、旧ファイルを削除する [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D4] [tasks: T005]
- [IN-11] .claude/rules/06-security.md のコード実装パターンを既存の knowledge/conventions/security.md に統合し、旧ファイルを削除する [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D4] [tasks: T006]
- [IN-12] track/workflow.md の Guiding Principles 11 項目のうち、原則 10（自己修復優先）を .claude/rules/08-orchestration.md の「If unsure」末尾に 1 行追記し、原則 11（レビューサーフェース最小化）を .claude/rules/10-guardrails.md の「Small task commits」項目に O(N²) 根拠として 1 行追記する。残り 9 項目は他箇所で enforce 済みなので移送せず削除する [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D5] [tasks: T007]
- [IN-13] AGENTS.md を PR レビュー専用 briefing として強化する。severity policy（P0/P1 のみ報告）を維持したうえで、ローカルレビュワーが取りこぼしうる観点（ブランチ全体を通した整合性・複数コミットにまたがる変更の一貫性など PR 単位でしか見えない観点）をすべて盛り込む。conventions へのポインタ化はしない [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D6] [tasks: T002]
- [IN-14] .claude/rules/08-orchestration.md の Briefing Requirements 節（「全 capability briefing は .claude/rules/04-coding-principles.md を参照せよ」ルール）を削除する [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D7] [tasks: T004]
- [IN-15] 運用文書から削除済み機構への参照と実行すると失敗する手順案内を排除する。対象: .claude/rules/08 の scripts/check_layers.py 参照・.claude/rules/09 の python3 前提記述・.claude/rules/10 の scripts/verify_orchestra_guardrails.py 参照・移管後 security convention の EXPECTED_DENY 追加先への verifier 参照・移管後 testing convention の test-one-exec タスク名・本再編で削除する DEVELOPER_AI_WORKFLOW.md への参照・新設 3 convention に引き継ぐ dead-ref [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D8] [tasks: T005, T006, T007, T008, T009]
- [IN-16] track/workflow.md の Quality Gates チェックリストは（track/workflow.md 廃止に伴い）削除し、新たな doc として起こさない。Makefile.toml の ci-local / ci-container task dependencies が機械可読な Quality Gates の真実の源泉であることを明確にする [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D9] [tasks: T007]
- [IN-17] CLAUDE.md の参照一覧を本再編（削除・移管・新設）に合わせて更新する。DEVELOPER_AI_WORKFLOW.md / track/workflow.md 行を削除し、移管後の conventions と新設 3 convention を反映し、.claude/rules の列挙を 01/07/08/09/10 に限定する [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D10] [tasks: T004, T005, T006, T007, T008, T009]
- [IN-18] 残置する .claude/rules（01/07/08/09/10）を再編後の状態と突合し、索引・運用規則として必要十分な情報量に絞る。重複説明の削減、現行コマンドとの整合確認を含む [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D10] [tasks: T009]
- [IN-19] knowledge/conventions/README.md の convention 索引を bin/sotp conventions update-index で再生成する（新設 3 convention と移管 convention の追加を反映する） [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D3] [tasks: T003, T004, T005, T006, T010]

### Out of Scope
- [OS-01] 過去 track（done 済み）の spec.json / impl-plan.json 等に埋め込まれた旧パス（.claude/rules/04/05/06・track/workflow.md・DEVELOPER_AI_WORKFLOW.md）の修正。過去 track の artifact は歴史的記録であり、参照整合の検証は現行 track のみが対象でゲートに影響しない [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D4]
- [OS-02] AGENTS.md の規約内容を conventions に転記して正本を一本化すること。AGENTS.md と conventions の意図的な重複は許容し、規約変更時の手動同期を前提とする [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D6]
- [OS-03] writer agent 定義（.claude/agents/ 配下）の必読パス指定の更新。移管後の conventions パスへの更新は後続の保守タスクとして扱う（本再編のスコープ外） [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D4]
- [OS-04] Quality Gates チェックリストを新しい文書として起こすこと。Makefile.toml を SSoT とするため、チェックリストの doc 化は行わない [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D9]
- [OS-05] Makefile.toml の ci-local / ci-container task の category メタ整備。cargo make help の出力改善は本再編後の任意保守作業とする [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D9]
- [OS-06] Branch Strategy 可変化を扱う後続 ADR の実装。branch-strategy.md を config-driven 記述に書き直す作業は本再編の後続として別 track で扱う [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D3]

## Constraints
- [CN-01] .claude/rules/ には以後 Claude Code 固有の運用規則（orchestration・permission/hook ガードレール・開発環境コマンド・言語運用）だけを置く。provider に依存しない工学規約（coding-principles・testing・security）は knowledge/conventions/ が正本であり、.claude/rules/ に重複コピーを置かない [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D4] [tasks: T004, T005, T006]
- [CN-02] 旧 .claude/rules/04/05/06 への後方互換パス（ポインタ stub・リダイレクト）は設けない。過去 track の参照はリンク切れとして歴史的記録に留める。参照整合の検証（sotp verify plan-artifact-refs）は現行 track のみが対象であり、過去 track のリンク切れはゲートを block しない [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D4] [conv: knowledge/conventions/no-backward-compat.md#Rules] [tasks: T004, T005, T006]
- [CN-03] verify-orchestra（利用者の .claude/settings.json 等の設定値を CI で強制していた機構）は 2026-06-13 に全廃済みである。D8 の対応は verifier への張替えではなく、危険な permission 設定例の docs 警告への縮約または削除とする [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D8] [conv: knowledge/conventions/responsibility-boundary.md#Rules] [tasks: T009]
- [CN-04] D7（Briefing Requirements 廃止）の削除は .claude/rules/08-orchestration.md のみに適用する。writer agent 定義の必読パス指定は別途保守するが、本再編でそれを同期させる義務はない [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D7] [tasks: T004]
- [CN-05] 本再編で新設する 3 convention（branch-strategy.md / track-lifecycle.md / git-notes.md）は、track/workflow.md から引き継いだ dead-ref（廃止された cargo make タスク名・削除済み doc パス等）がないかスキャンし、あれば D8 の一環として修正する [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D8] [tasks: T003]

## Acceptance Criteria
- [ ] [AC-01] README.md が存在し、SoTOHE の価値説明（SoT Chain・信号機・track モデル）と正規フロー（/adr:add → /track:adr2pr の手順）を含む。START_HERE_HUMAN.md と LOCAL_DEVELOPMENT.md が存在しない [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D1] [tasks: T001, T008]
- [ ] [AC-02] DEVELOPER_AI_WORKFLOW.md が存在しない。README.md が前提条件・自由文依頼例の内容（DEVELOPER_AI_WORKFLOW.md の固有価値）を含む節を持つ [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D2] [tasks: T008]
- [ ] [AC-03] track/workflow.md が存在しない [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D3] [tasks: T007]
- [ ] [AC-04] knowledge/conventions/branch-strategy.md、knowledge/conventions/track-lifecycle.md、knowledge/conventions/git-notes.md が存在し、それぞれ Branch Strategy + ガードポリシー、SSoT lifecycle + observations.md + registry.md 更新ルール、Git Notes の内容を持つ [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D3] [tasks: T003]
- [ ] [AC-05] track/workflow.md への参照を持っていた .claude/commands/track/setup.md・.claude/rules/08/09/10・CLAUDE.md が、新設 3 convention を参照するように更新されている。.claude/rules/08-orchestration.md の Operational split 記述が conventions を day-to-day workflow rules の場所として明記している [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D3] [tasks: T007]
- [ ] [AC-06] .claude/rules/04-coding-principles.md・.claude/rules/05-testing.md・.claude/rules/06-security.md が存在しない。各内容は knowledge/conventions/ 配下の convention として存在する。knowledge/conventions/security.md が 06-security.md のコード実装パターンを統合した内容を持つ [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D4] [tasks: T004, T005, T006]
- [ ] [AC-07] .claude/rules/08-orchestration.md の If unsure セクション末尾に「3 回詰まったら researcher で原因切り分け（自己修復優先）」の 1 行が追記されている。.claude/rules/10-guardrails.md の Small task commits 項目に O(N²) コスト根拠の 1 行が追記されている。これら以外の Guiding Principles 9 項目は移送先に存在しない（削除のみ） [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D5] [tasks: T007]
- [ ] [AC-08] AGENTS.md が PR レビュー専用 briefing として強化されており、severity policy と PR 単位でのみ見える観点（ブランチ全体を通した整合性・複数コミットにまたがる変更の一貫性等）を含む。AGENTS.md は conventions へのポインタ化されていない（自己完結 briefing として機能する） [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D6] [tasks: T002]
- [ ] [AC-09] .claude/rules/08-orchestration.md に Briefing Requirements 節が存在しない [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D7] [tasks: T004]
- [ ] [AC-10] .claude/rules/08-orchestration.md に scripts/check_layers.py への参照が存在しない。.claude/rules/09-maintainer-checklist.md に python3 を Docker 内部に必要とする前提記述が存在しない。.claude/rules/10-guardrails.md に scripts/verify_orchestra_guardrails.py / verify-orchestra への参照が存在しない [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D8] [tasks: T007, T009]
- [ ] [AC-11] 移管後の security convention に EXPECTED_DENY 追加先・回帰テスト追加先として削除済みスクリプトや verify-orchestra を指す保守手順が存在しない。移管後の testing convention に cargo make test-one-exec（現行 Makefile.toml に存在しないタスク名）への案内が存在しない [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D8] [tasks: T005, T006]
- [ ] [AC-12] 残置する .claude/rules（01/07/08/09/10）および CLAUDE.md に DEVELOPER_AI_WORKFLOW.md への参照が存在しない [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D8] [tasks: T008]
- [ ] [AC-13] 新設 3 convention（branch-strategy.md / track-lifecycle.md / git-notes.md）に track/workflow.md から引き継いだ dead-ref が存在しない [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D8] [tasks: T003]
- [ ] [AC-14] track/workflow.md の Quality Gates チェックリストが存在しない（track/workflow.md 廃止により消える）。Quality Gates の内容を新たな doc として起こした文書が存在しない。残置する開発環境ガイダンスが Makefile.toml の ci-local / ci-container task dependencies を機械可読な Quality Gates の真実の源泉として示している [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D9] [tasks: T007]
- [ ] [AC-15] CLAUDE.md の priority references から DEVELOPER_AI_WORKFLOW.md 行と track/workflow.md 行が削除されており、新設 3 convention（branch-strategy.md / track-lifecycle.md / git-notes.md）と移管後の conventions が反映されている。.claude/rules の参照が 01/07/08/09/10 に限定されている [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D10] [tasks: T004, T005, T006, T007, T008, T009]
- [ ] [AC-16] knowledge/conventions/README.md の convention 索引が bin/sotp conventions update-index で再生成されており、新設 3 convention と移管後の convention が一覧に含まれている [adr: knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md#D3] [tasks: T003, T004, T005, T006, T010]

## Related Conventions (Required Reading)
- knowledge/conventions/no-backward-compat.md#Rules
- knowledge/conventions/responsibility-boundary.md#Rules
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/source-attribution.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 48  🟡 0  🔴 0

