<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 25, yellow: 0, red: 0 }
---

# verification.md を observations.md に改名 — 役割を手動観測ログに限定

## Goal

- [GO-01] verification.md から acceptance criteria 充足宣言の役割 (役割 A) を完全に廃止し、AC 充足の正を spec.json signals + review.json zero_findings + impl-plan.json task done/commit_hash の 3 機構に移譲することで、workflow-ceremony-minimization の「成果物レビューは事後方式」原則に準拠した workflow を確立する [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D1]
- [GO-02] 機械検証不能な手動観測ログ (役割 B) を新ファイル track/items/<id>/observations.md に移行し、verification.md という名前を新規 track で使わないことで、grep-based な全参照洗い出しを可能にして移行作業の機械的検証を担保する [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D2]
- [GO-03] verify-latest-track (libs/infrastructure/src/verify/latest_track.rs) から verification.md の必須化と VERIFICATION_SCAFFOLD_LINES scaffold 検出を完全に削除し、CI gate を spec.md / spec.json / plan.md のみに縮退させることで、「file 存在 = phase 状態」原則を observations.md にも適用する [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D3]
- [GO-04] observations.md の作成タイミングを implementer 裁量 + spec AC 明示の観測要求のいずれかに限定し、/track:implement / /track:full-cycle の手順記述を「上記条件に該当する場合のみ observations.md を作成/追記」に書き換えることで、ceremony 削減後の運用を workflow docs に反映する [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D4]

## Scope

### In Scope
- [IN-01] libs/infrastructure/src/verify/latest_track.rs から VERIFICATION_SCAFFOLD_LINES static set、scaffold_placeholder_lines 関数、validate_verification_file 関数を削除し、verify-latest-track の検証対象を spec.md / spec.json / plan.md のみに縮退させる [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D3] [tasks: T001]
- [IN-02] /track:implement および /track:full-cycle のコマンド記述から「無条件で verification.md を update」する手順を削除し、「D4 (a)/(b) のいずれかに該当する場合のみ observations.md を作成/追記する」手順に置き換える [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D4] [tasks: T003]
- [IN-03] /track:commit コマンド記述から verification.md への参照を削除し、observations.md の optional source として扱う記述 (D4 の条件に沿って存在する場合のみ参照) に更新する [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D4] [tasks: T004]
- [IN-04] track/workflow.md の「## verification.md」セクションを廃止または改訂し、observations.md の役割 (D2: 機械検証不能な手動観測ログ、自由形式、scaffold なし) および作成条件 (D4 (a)/(b)) を新セクションで記述する [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D2, knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D4] [tasks: T005]
- [IN-05] CLAUDE.md、START_HERE_HUMAN.md、.claude/rules/08-orchestration.md、.claude/rules/10-guardrails.md から verification.md への言及を削除または observations.md の文脈に書き換える [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D2] [tasks: T006]
- [IN-06] verify-latest-track に関連する unit / integration test を VERIFICATION_SCAFFOLD_LINES の削除に合わせて更新・削除し、縮退後の CI gate (spec.md / spec.json / plan.md のみ検証) を正とするテストを整備する [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D3] [tasks: T002]

### Out of Scope
- [OS-01] 過去 track の track/items/*/verification.md および track/archive/*/verification.md の batch rename または内容刈り込み。これらは歴史資料として原型保存する (D5) [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D5]
- [OS-02] observations.md の scaffold 自動生成機能の追加。CI gate への必須化や scaffold check の新設も行わない (D3 の「将来 observations.md に対しても scaffold check / 必須化を新設しない」方針) [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D3]
- [OS-03] git history / 過去 ADR / 過去 commit message / 過去 review.json などからの verification.md 文字列参照の修正。これらは歴史資料として保存する (D5) [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D5]
- [OS-04] 歴史資料にある verification.md 参照の書き換えは行わない: knowledge/adr/ 内の過去 ADR 本文、過去 commit message、過去 review.json などは原型保存する (D5)。IN-02 / IN-03 / IN-04 / IN-05 の対象 (active workflow docs: .claude/commands/track/*.md、track/workflow.md、CLAUDE.md、START_HERE_HUMAN.md、.claude/rules/) は書き換え対象であり本 OS-04 の対象外である [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D5]

## Constraints
- [CN-01] verify-latest-track の縮退後も、最新 track 選択のための metadata.json / impl-plan.json の読み込みは維持する。検証対象ファイルを spec.md / spec.json / plan.md に縮退させることは、track 選択ロジックの変更ではない [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D3] [tasks: T001, T002]
- [CN-02] observations.md は「ファイルが存在しない = 観測なし」として扱い、CI gate / verify-latest-track は observations.md の不在を error にしない。file 存在ベースの単純分岐 (存在すれば参照、なければ skip) に限定する [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D3] [tasks: T001, T002]
- [CN-03] observations.md の内容フォーマットは自由とし、scaffold / required field / required section などの構造制約を設けない。作成者の裁量で観測対象 / 手順 / 値 / 日時を含む [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D2] [tasks: T001, T005]
- [CN-04] 新規 track で verification.md を作成しない。track/items/<id>/ 配下への verification.md 新規作成は本 ADR 移行後は禁止とし、手動観測ログが必要な場合は observations.md に記録する [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D2] [tasks: T005, T006]
- [CN-05] observations.md の作成は D4 (a) 実装中に implementer が機械検証不能な観測値があると判断した場合、または D4 (b) spec.json の acceptance_criteria に「observations.md に記録する」と明示された場合に限定する。両条件は排他ではない [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D4] [tasks: T003, T005]

## Acceptance Criteria
- [ ] [AC-01] libs/infrastructure/src/verify/latest_track.rs に VERIFICATION_SCAFFOLD_LINES、scaffold_placeholder_lines、validate_verification_file が存在しない。cargo make ci (verify-latest-track を含む全 CI task) が pass する [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D3] [tasks: T001, T002]
- [ ] [AC-02] /track:implement および /track:full-cycle のコマンドドキュメントに「無条件で verification.md を update する」指示が存在しない。代わりに D4 (a)/(b) の条件付きで observations.md を作成/追記する手順が記述されている [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D4] [tasks: T003]
- [ ] [AC-03] /track:commit のコマンドドキュメントが verification.md を必須 source として参照しておらず、observations.md を optional source として扱う記述になっている (存在する場合のみ参照) [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D4] [tasks: T004]
- [ ] [AC-04] track/workflow.md に旧 verification.md セクションが存在しないか、存在する場合は observations.md の役割 (機械検証不能な手動観測ログ、自由形式、scaffold なし) と作成条件 (D4 (a)/(b)) を正確に記述したセクションに置き換えられている [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D2, knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D4] [tasks: T005]
- [ ] [AC-05] CLAUDE.md、START_HERE_HUMAN.md、.claude/rules/08-orchestration.md、.claude/rules/10-guardrails.md に verification.md への言及が残っていない (または observations.md の文脈に書き換えられている) [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D2] [tasks: T006]
- [ ] [AC-06] cargo make ci の全 task (fmt-check + clippy + nextest + test-doc + deny + check-layers + verify-*) が pass する。verify-latest-track に関連するテストが VERIFICATION_SCAFFOLD_LINES 削除後の縮退仕様を正として pass する [adr: knowledge/adr/2026-04-24-2356-verification-md-rename-observations-md.md#D3] [tasks: T001, T002]

## Related Conventions (Required Reading)
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/source-attribution.md#Rules
- knowledge/conventions/adr.md#Lifecycle: pre-merge draft vs post-merge record
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/05-testing.md#Test Structure

## Signal Summary

### Stage 1: Spec Signals
🔵 25  🟡 0  🔴 0

