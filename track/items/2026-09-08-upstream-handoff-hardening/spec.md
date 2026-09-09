<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 35, yellow: 0, red: 0 }
---

# 共通ハーネスの責務・復旧・検証契約を整える

## Goal

- [GO-01] SoTOHE-core の共通 workflow、provider adapter、配布案内、型 catalogue 案内、および PR review 入口が、それぞれの所有権と利用者に見える完了契約へ整合するようにする。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D1, knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D2, knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D3]
- [GO-02] SoTOHE-core が残作業の復旧、reviewer 失敗の安全な診断、複合 anchor の局所的な義務判定を、既存の保護と検証可能性を保って実行できるようにする。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D4, knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D5, knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D6]

## Scope

### In Scope
- [IN-01] 共通 workflow と Codex / Claude provider adapter の責務・呼出し・再開案内を、既存の所有権に整合させる。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D1] [tasks: T001, T002, T012]
- [IN-02] Claude 配布 allowlist と type-designer の成果物案内を、現行のコマンドおよび type catalogue 契約に整合させる。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D2] [tasks: T011, T003]
- [IN-03] 自動 PR review の共通方法論の正本化、custom focus との分離、完了結果の区別を整える。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D3] [tasks: T004, T013]
- [IN-04] open PR に結び付く track で発見された実在の残作業を正規 task として復旧し、再計画後に通常の実装・検証・review・PR 再審査へ戻す。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D4] [tasks: T005]
- [IN-05] reviewer subprocess 失敗について、provider、取得できた終了コード、失敗を区別できる安全な分類を、秘匿と有界化を保った診断として上位 CLI へ伝える。利用者中断・timeout・出力形式不正と区別し、失敗から成功 verdict を生成しない。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D5] [tasks: T006, T007]
- [IN-07] review_v2 の状態照会専用構成を、infrastructure 所有の NullReviewer を composition root が構築して ReviewCycle へ注入する局所的な adapter 修正として整える。 [adr: knowledge/adr/2026-09-09-0916-query-only-reviewer-safety.md#D1] [tasks: T006, T007]
- [IN-06] 複合 anchor の obligation 判定を対象 entry の局所責務へ較正し、合格例・不合格例と cache 同一性でその契約を検証する。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D6] [tasks: T008, T009, T010]

### Out of Scope
- [OUT-01] PreCompact の再開を新しい永続機構に置き換えること、または Claude agent 一覧を現在の provider 割当ての固定記述にすること。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D1] [tasks: T001, T002, T012]
- [OUT-02] 利用者の Claude 設定値を CI で固定すること、既存 top-level inherent implementation 形式を廃止すること、または新しい schema 形式を追加すること。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D2] [tasks: T011, T003]
- [OUT-03] ユーザー承認済み逸脱の既存承認要件を緩和すること、または共通 PR review 方法論を root の常時入力へ複製すること。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D3] [tasks: T004, T013]
- [OUT-04] merge 済み、archived、または closed PR の修正を既存 track に復旧すること、あるいは task 追加・義務 derive のたびに GitHub 照会を要求すること。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D4] [tasks: T005]
- [OUT-05] stderr 全文や任意の subprocess 出力を上位 error、debug、log に露出すること、または新しい生診断ログ参照を追加すること。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D5] [tasks: T006, T007]
- [OUT-06] 他 entry の検証済み状態を仮定・探索する新たな横断検査、既存の矛盾・すり替え・中心部未検証の検出を撤去すること、または有限の較正例を未知の複合 anchor を完全に判定する証明として扱うこと。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D6] [tasks: T008, T009, T010]

## Constraints
- [CN-01] 運用文書は provider の現在の割当数や特定の担当者数を固定せず、実際の割当ては既存 profile から判断できるようにする。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D1] [tasks: T001, T012]
- [CN-02] 既存 schema が支える top-level inherent implementation 形式は維持し、新たな schema 形式を追加しない。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D2] [tasks: T003]
- [CN-03] PR review の共通方法論を root の常時入力へ本文として追加しない。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D3] [tasks: T004]
- [CN-04] Done / Archived の凍結を撤去せず、完了履歴をダミー task または無意味な状態往復で変更しない。通常の PR 作成前の開発手順に PR 復旧条件を課さない。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D4] [tasks: T005]
- [CN-05] 任意の subprocess 出力をそのまま error、debug、または log へ保存・追加せず、新たな診断ログ保存機構も導入しない。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D5] [tasks: T006, T007]
- [CN-06] モデル名変更だけによる再判定、force、cache 手編集・削除、無意味な hash 変更、有効な参照の削除、責務越境した実装または不適切な waiver を判定回避策にしない。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D6] [tasks: T008, T009, T010]

## Acceptance Criteria
- [ ] [AC-01] 共通 workflow は手順、状態遷移、入力取得、再開条件の正本として機能し、provider adapter は provider 固有の呼出し面・配線・制約・報告だけを担う。既存の入力確認と裁定境界は維持される。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D1] [tasks: T001, T012]
- [ ] [AC-02] Codex の汎用 capability 呼出し、Claude の dispatcher 条件付き委譲、phase writer、および rollback の実装経路が、それぞれ定められた provider 選択と担当境界に従って動作する。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D1] [tasks: T001, T002]
- [ ] [AC-03] Claude 配布の allowlist は既存の phase、test-obligation、catalog、ref-verify コマンド系統を利用可能にしつつ、利用者の設定値を CI が固定する契約を追加しない。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D2] [tasks: T011]
- [ ] [AC-04] type-designer の案内は成果物を所有する workflow の終端へ到達し、通常の inherent method と既存の top-level inherent implementation 形式を混同せず扱える。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D2] [tasks: T003]
- [ ] [AC-05] すべての自動 PR reviewer に共通する確認方法と finding 報告境界は framework 所有の正本から適用され、custom prompt は個別 focus と severity を保持したままその正本を参照する。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D3] [tasks: T004]
- [ ] [AC-06] PR review の完了表示は明示的な finding なしとユーザー承認済みの逸脱を区別し、後者を zero findings と表示しない。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D3] [tasks: T013]
- [ ] [AC-07] 同一 track の open PR で実在する残作業が見つかった場合、復旧 workflow は PR と現在 branch の対応を確認して残作業を正規 task として登録し、task 群から状態を再導出して通常の再計画・義務再導出へ進める。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D4] [tasks: T005]
- [ ] [AC-08] PR writer の修正後は下流成果物を依存順に再収束し、未完了 task があれば通常の full-cycle を完了してから PR を再審査する。対応する PR 状態を確認できない場合は復旧を仮定せず原因を報告する。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D4] [tasks: T005]
- [ ] [AC-09] reviewer subprocess の失敗は provider、取得できた終了コード、および安全な失敗分類を保ったまま上位 CLI へ伝わり、利用者中断、timeout、出力形式不正と区別される。失敗から成功 verdict は生成されない。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D5] [tasks: T006, T007]
- [ ] [AC-10] 自由文の subprocess 診断を表示する場合は既存の秘匿境界を通し、秘匿後の UTF-8 byte 列を 4 KiB（4096 bytes）以下に制限する。上限を超えた、UTF-8 として扱えない、または秘匿できない内容は自由文を表示せず、固定分類 `diagnostic_unavailable` と取得済みの失敗終了コードへ縮退する。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D5] [tasks: T006, T007]
- [ ] [AC-11] 複合 anchor の判定は対象 obligation item と entry declaration が所有する振る舞いを局所的に評価し、同じ anchor 内で別 entry が所有する部分を Fail の根拠にしない。一方で対象の中心的振る舞いが未検証なら既存の Fail 類型を適用する。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D6] [tasks: T008, T009, T010]
- [ ] [AC-12] 較正は局所責務を満たす合格例と中心的振る舞いが未検証の不合格例の双方を用い、構造的回帰試験と実際の設定 provider による結果を別々に報告する。prompt または判定入力の実質的変更は cache 同一性に反映される。 [adr: knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md#D6] [tasks: T008, T009, T010]
- [ ] [AC-14] review_v2 の状態照会専用構成では、NullReviewer を infrastructure の secondary adapter とし、composition root はそれを構築して ReviewCycle へ注入するだけにする。既存の Reviewer port の method 契約は変更しない。 [adr: knowledge/adr/2026-09-09-0916-query-only-reviewer-safety.md#D1] [tasks: T006, T007]
- [ ] [AC-15] review results の state-summary は get_review_states で状態を読み、review check-approved は evaluate_approval を通じて同じ get_review_states を用いる。どちらも Reviewer を実行しない。NullReviewer の review または fast_review が誤って呼ばれた場合は、成功 verdict または provider 実行を生成せず、型付き PreSpawn / Unavailable 診断で fail-closed に失敗する。 [adr: knowledge/adr/2026-09-09-0916-query-only-reviewer-safety.md#D1] [tasks: T006, T007]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 35  🟡 0  🔴 0

