<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 19, yellow: 0, red: 0 }
---

# レビュー指示書のカテゴリ閉列挙を半開形式へ改める

## Goal

- [GO-01] コード層 reviewer briefing を半開形式へ移行し、優先カテゴリに列挙されていない場合でも役割文への違反を報告可能にする。一方で、文書検査系の閉列挙とノイズ抑制の下限は維持する。 [adr: knowledge/adr/2026-07-23-0109-review-briefing-open-category-format.md#D1, knowledge/adr/2026-07-23-0109-review-briefing-open-category-format.md#D2, knowledge/adr/2026-07-23-0109-review-briefing-open-category-format.md#D3]
- [GO-02] briefing 改訂時に役割文で禁じた事象と報告可能なカテゴリの不整合を発見できる maintainer guidance を整備する。 [adr: knowledge/adr/2026-07-23-0109-review-briefing-open-category-format.md#D4]

## Scope

### In Scope
- [IN-01] `.harness/custom/review-prompts/` の `domain.md`、`usecase.md`、`infrastructure.md`、`cli.md`、`cli_composition.md`、`cli_driver.md`、`harness-policy.md` を、閉列挙ではないコード層 reviewer briefing として改訂する。 [adr: knowledge/adr/2026-07-23-0109-review-briefing-open-category-format.md#D1] [tasks: T001]
- [IN-02] 対象コード層 briefing から「Report findings ONLY for the following categories」という閉列挙の指示を除き、役割文への違反は常に報告対象であることを明記する。既存カテゴリ列挙は、網羅を主張しない優先カテゴリとして扱う。 [adr: knowledge/adr/2026-07-23-0109-review-briefing-open-category-format.md#D1] [tasks: T001, T002]
- [IN-03] 全 reviewer briefing の What NOT to report を維持し、文体・命名・体裁などのノイズや、閉じた gate 後の代替案提案を報告対象へ戻さない。 [adr: knowledge/adr/2026-07-23-0109-review-briefing-open-category-format.md#D2] [tasks: T001, T002]
- [IN-04] `plan-artifacts.md` を代表とする事実検査系 briefing は、欠陥クラスが列挙で閉じるものとして閉列挙のまま維持する。 [adr: knowledge/adr/2026-07-23-0109-review-briefing-open-category-format.md#D3] [tasks: T002]
- [IN-05] ADR、spec、types、impl-plan の SoT 別 briefing は、各 briefing が ADR / 行動契約 / 型カタログ / 実行可能計画の事実検査を対象とし、欠陥クラスを既存の列挙で閉じられるため、個別判定の結果をすべて閉列挙として維持する。 [adr: knowledge/adr/2026-07-23-0109-review-briefing-open-category-format.md#D3] [tasks: T002]
- [IN-06] `.claude/rules/09-maintainer-checklist.md` に、briefing の役割文で禁じた事象が報告可能なカテゴリを持たないまま残っていないことを確認する項目を追加する。 [adr: knowledge/adr/2026-07-23-0109-review-briefing-open-category-format.md#D4] [tasks: T003]

### Out of Scope
- [OUT-01] 文書検査系 briefing の閉列挙を一律に廃止したり、事実検査として閉じている欠陥クラスを自由記述レビューへ変更したりしない。 [adr: knowledge/adr/2026-07-23-0109-review-briefing-open-category-format.md#D3] [tasks: T002]
- [OUT-02] ノイズ抑制のための What NOT to report を削除または弱めず、役割文違反を理由に文体・命名・体裁の指摘を報告可能にしない。 [adr: knowledge/adr/2026-07-23-0109-review-briefing-open-category-format.md#D2] [tasks: T001, T002]
- [OUT-03] 未列挙の役割文違反ごとに新しい閉列挙カテゴリを追加して追随する方式にはしない。 [adr: knowledge/adr/2026-07-23-0109-review-briefing-open-category-format.md#D1] [tasks: T001]

## Constraints
- [CN-01] 半開化した briefing は、優先カテゴリを探索の焦点および severity 判定の基準として維持しつつ、それらが報告可能な設計逸脱の全件列挙であると示してはならない。 [adr: knowledge/adr/2026-07-23-0109-review-briefing-open-category-format.md#D1] [tasks: T001]
- [CN-02] 半開・閉の選択は、review 対象の欠陥クラスを既存カテゴリの列挙だけで完全に判定できるかで決める。7 本のコード層 briefing は設計逸脱が開集合のため半開にし、ADR、spec、types、impl-plan および plan-artifacts 型の事実検査 briefing は欠陥クラスが列挙で閉じるため閉列挙にする。 [adr: knowledge/adr/2026-07-23-0109-review-briefing-open-category-format.md#D3] [tasks: T002]
- [CN-03] briefing の role statement は、優先カテゴリの判定文脈を与える役割を維持し、カテゴリ列挙との整合を取るために削除しない。 [adr: knowledge/adr/2026-07-23-0109-review-briefing-open-category-format.md#D1] [tasks: T001]

## Acceptance Criteria
- [ ] [AC-01] 7 本の対象コード層 briefing のそれぞれについて、閉列挙の ONLY 指示がなく、role statement 違反を常に報告対象とする明文があり、既存カテゴリが優先カテゴリとして示されることを文書内容で確認できる。 [adr: knowledge/adr/2026-07-23-0109-review-briefing-open-category-format.md#D1] [tasks: T001]
- [ ] [AC-02] 全 reviewer briefing に What NOT to report が残り、文体・命名・体裁の指摘と閉じた gate 後の代替案提案が引き続き非報告対象であることを確認できる。 [adr: knowledge/adr/2026-07-23-0109-review-briefing-open-category-format.md#D2] [tasks: T001, T002]
- [ ] [AC-03] `impl-plan.md` に `Report findings ONLY for the following categories` という閉列挙の明文と列挙された欠陥カテゴリが残り、同 briefing が列挙外の報告を許可しないことを文書内容で確認できる。 [adr: knowledge/adr/2026-07-23-0109-review-briefing-open-category-format.md#D3] [tasks: T002]
- [ ] [AC-04] ADR、spec、types、impl-plan の各 SoT briefing に `Report findings ONLY for the following categories` の閉列挙が残り、各 briefing の既存カテゴリだけで ADR / 行動契約 / 型カタログ / 実行可能計画の事実検査を判定することを文書内容で確認できる。 [adr: knowledge/adr/2026-07-23-0109-review-briefing-open-category-format.md#D3] [tasks: T002]
- [ ] [AC-05] maintainer checklist に、briefing 改訂時に role statement で禁じた事象が報告可能なカテゴリを持たないまま残っていないかを確認する項目が存在することを確認できる。 [adr: knowledge/adr/2026-07-23-0109-review-briefing-open-category-format.md#D4] [tasks: T003]

## Related Conventions (Required Reading)
- knowledge/conventions/review-protocol.md#zero_findings 完了条件
- knowledge/conventions/no-upstream-restatement.md#Rules
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/responsibility-boundary.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 19  🟡 0  🔴 0

