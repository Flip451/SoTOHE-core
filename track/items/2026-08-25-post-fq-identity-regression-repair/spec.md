<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 14, yellow: 0, red: 0 }
---

# 完全修飾識別の add 型・bin root 別名リグレッションを修復する

## Goal

- [GO-01] 完全修飾 identity の解決を一貫した fail-closed な解決集合と正準化に基づかせ、実装前 add 型、bin root 別名、module_path 省略による回帰を解消する。 [adr: knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D1, knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D2, knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D3]

## Scope

### In Scope
- [IN-01] rustdoc paths と catalogue の add 宣言を統合した解決集合を用い、catalogue 由来の合成 summary を含む型 identity の解決を行うこと。 [adr: knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D1] [tasks: T001, T002, T003, T006, T007]
- [IN-02] 解決集合を読む既存の型解決経路、function identity、catalog import を、共通の解決結果と crate root 正準化に通すこと。 [adr: knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D1, knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D2] [tasks: T002, T003, T004, T006, T007, T008]
- [IN-03] module_path 省略時の add、modify、delete、reference の identity 解決規則、および namespace-aware な同名比較を実現すること。 [adr: knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D3] [tasks: T001, T002, T003, T007]
- [IN-04] test-obligation derive における trait_impls[].for_type の trait-impl carrier 解決を、catalogue の完全修飾 entry key に従わせること。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1, knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D1] [tasks: T005]

### Out of Scope
- [OS-01] 一つの crate に複数の bin target があり root 別名が一対一でなくなる構成への対応は本 track の対象外とする。 [adr: knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D2] [tasks: T002, T004, T008]

## Constraints
- [CO-01] baseline と current の rustdoc paths および catalogue 宣言を合わせた解決集合は一箇所で構築し、codec、type-signal identity index、deletion 処理、Phase 1 definition-path authority の各解決経路は同じ結果を使用する。経路別の add 型の黙認、生名 fallback、又は個別 fallback を残さない。 [adr: knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D1] [tasks: T002, T003, T006, T007]
- [CO-02] crate root 別名は解決集合を読む正準化で一度だけ適用し、D1 の四経路、function identity、catalog import はその正準化を通過する。経路ごとの別名適用は残さない。 [adr: knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D2] [tasks: T002, T003, T004, T006, T007, T008]
- [CO-03] module_path の省略は crate root ではなく配置未指定を表す。明示 module_path 又は修飾 key は完全一致で照合し、catalogue crate 内では type と trait の namespace を分けて同名を比較する。曖昧又は根拠不足の解決は fail-closed とする。 [adr: knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D3] [tasks: T001, T002, T003, T007]
- [CO-04] test-obligation derive の trait-impl carrier 解決は catalogue の完全修飾 identity を唯一の entry key とし、短名への raw fallback 又は経路固有の fallback を残さない。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1, knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D1] [tasks: T005]

## Acceptance Criteria
- [ ] [AC-01] catalogue が宣言する未実装 add 型を含め、add 型同士の相互参照、modify 型から add 型への参照、および宣言順序に依存しない参照解決が成功する。rustdoc と catalogue のいずれにも存在しない参照は fail-closed で拒否される。 [adr: knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D1] [tasks: T002, T003, T006, T007]
- [ ] [AC-02] bin target crate で catalogue の crate 名と rustdoc root 名が異なる場合も、型 entry の add、modify、reference と function path の双方が正しく解決される。 [adr: knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D2] [tasks: T002, T003, T004, T006, T007, T008]
- [ ] [AC-03] module_path を省略した add は、baseline に同名があれば fail-closed とし、baseline に同名がなく current に同名が一つだけあるときその実装済み identity に解決し、current に複数候補があるときは明示指定を求めて fail-closed とし、候補がないときは module 未確定の未実装 identity として扱う。modify、delete、reference は baseline に同名が一つだけある場合にのみ解決する。 [adr: knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D3] [tasks: T001, T002, T003, T007]
- [ ] [AC-04] test-obligation derive は trait_impls[].for_type を catalogue の完全修飾 entry key に解決し、解決不能又は曖昧な場合は fail-closed とする。短名を raw entry key として用いる fallback は行わない。 [adr: knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1, knowledge/adr/2026-08-25-0804-post-fq-identity-regression-repair.md#D1] [tasks: T005]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 14  🟡 0  🔴 0

