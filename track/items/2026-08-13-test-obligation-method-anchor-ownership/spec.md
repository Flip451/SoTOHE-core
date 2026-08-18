<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.7"
signals: { blue: 34, yellow: 0, red: 0 }
---

# trait_method obligations own method-scoped anchors

## Goal

- [GO-01] trait_method ごとの test-obligation が自 method の仕様 anchor だけを所有・検証できるようにし、複数 method の ApplicationService trait を method-relevant な fulfillment で test-obligation gate に通せるようにする。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1, knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D2]
- [GO-02] D1 の method-level spec_refs をコマンドが指名する catalogue で宣言・構造検証できるよう、対象となる MethodDeclaration の宣言を、宣言された entry に適用される catalogue lint 規則に適合する状態にする。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1, knowledge/adr/2026-07-04-0525-catalogue-v2-entry-lint-conformance.md#D1, knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3]

## Scope

### In Scope
- [IN-01] trait method ごとの spec anchor 担当を catalogue で宣言できるようにし、trait_method 義務が entry-level の全 anchor を一律に継承せず、自 method の spec_refs だけを所有するようにする。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1] [tasks: T1, T2, T3, T4]
- [IN-02] method の spec_refs は entry-level spec_refs からの割り当て部分集合ではなく、method 義務が所有する独立集合である。 [adr: knowledge/adr/2026-08-18-0055-entry-spec-refs-not-inventory.md#D1] [tasks: T4, T15, T16]
- [IN-03] fulfillment 検証と verifier instruction を、各 trait_method 義務が所有する anchor に限定する。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D2] [tasks: T5]
- [IN-05] D1 のためにコマンドが指名する catalogue へ宣言する MethodDeclaration を、宣言された entry に適用される catalogue lint 規則へ適合させる。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1, knowledge/adr/2026-07-04-0525-catalogue-v2-entry-lint-conformance.md#D1, knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3] [tasks: T1]
- [IN-09] PhaseCommandService の validate を、validate 自身が担当する spec anchor へ method-scoped に再 grounding し、validate の fulfillment を validate 自身の test で実証できる binding に戻す。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D3] [tasks: T6]
- [IN-10] PhaseCommandService の explain を、explain 自身が担当する spec anchor へ method-scoped に再 grounding し、explain の fulfillment を explain 自身の test で実証できる binding に戻す。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D3] [tasks: T7]
- [IN-11] PhaseCommandService の enter を、enter 自身が担当する spec anchor へ method-scoped に再 grounding し、enter の fulfillment を enter 自身の test で実証できる binding に戻す。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D3] [tasks: T8]
- [IN-12] `.harness/custom/review-prompts/harness-policy.md` から、track `scope-conditional-pre-review-gates-2026-07-31` に限定された PhaseCommandService の cross-populated fulfillment を受容する conditional Known Accepted Deviations 条項だけを撤去し、他の deviation 記述や他 track の記録は変更しない。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D3] [tasks: T9]
- [IN-13] TraitEntry.methods、TypeEntry.methods、inherent_impls[].methods の MethodDeclaration に独立した action を導入し、action 省略を add として扱い、親 entry action を継承しない。 [adr: knowledge/adr/2026-08-17-0340-method-declaration-action.md#D1] [tasks: T10]
- [IN-14] コマンドが指名する catalogue で、add または modify の method に非空の spec_refs を要求し、兄弟 method による代替を認めない。 [adr: knowledge/adr/2026-08-17-0340-method-declaration-action.md#D2, knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3] [tasks: T12, T11]
- [IN-15] コマンドが指名する catalogue で、親が reference または delete のとき子 MethodDeclaration の spec_refs は空でなければならない。非空なら構造検証で拒否する。親は TraitEntry.action、TypeEntry.action、または inherent_impl が所属する同一指名 catalogue 内 TypeEntry.action であり、type_name が未解決なら fail-closed とする。 [adr: knowledge/adr/2026-08-18-0040-parent-forbids-method-spec-refs.md#D1, knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3] [tasks: T14]

### Out of Scope
- [OS-01] コマンドが指名しない既存 catalogue 全体を新 schema へ遡及的に書き換えたり不適合として失敗させたりすることは本 track の対象外とする。 [adr: knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3] [tasks: T4, T11]
- [OS-02] cross-populated fulfillment を entry 単位の正規形として恒久化すること、または merged track にある歴史的な cross-populated binding record を書き換えることは本 track の対象外とする。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1] [tasks: T4, T5]
- [OS-03] trait_method 義務を廃止して型単位の粗い義務へ置き換えることは本 track の対象外とする。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1] [tasks: T4]
- [OS-04] entry-level spec_refs が作る check totality の cite edge を捨てること、または method 義務へ割り当てないことを理由に entry 参照の可視性を消すことは本 track の対象外とする。 [adr: knowledge/adr/2026-08-18-0055-entry-spec-refs-not-inventory.md#D1] [tasks: T5, T17]
- [OS-05] MethodDeclaration 以外の legacy catalogue entry を一律に remediation すること、または宣言済み entry への lint 適用を免除する一般方針を導入することは本 track の対象外とする。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1] [tasks: T1, T3]

## Constraints
- [CN-01] entry-level spec_refs は entry 自身の仕様への grounding であり、全 anchor の完全な目録ではない。method の spec_refs は独立した所有集合であり、entry-level spec_refs の部分集合であることや、その和で entry-level spec_refs を覆うことは要求しない。 [adr: knowledge/adr/2026-08-18-0055-entry-spec-refs-not-inventory.md#D1] [tasks: T4, T12, T15, T16]
- [CN-02] method action と Add/Modify の非空 spec_refs を含む新 schema は、コマンドが path で指名する catalogue にだけ適用する。コマンドが指名しない catalogue は書き換えず、不適合として失敗させない。 [adr: knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3] [tasks: T4, T11]
- [CN-03] fulfillment の method-scoped 検証は、各 trait_method 義務が所有する anchor だけを対象とする。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D2] [tasks: T5, T17]
- [CN-05] MethodDeclaration の lint-compliance remediation は、D1 の method-level spec_refs をコマンドが指名する catalogue で宣言・評価できる状態にするため、既存宣言の name、receiver、params、returns、async/default-implementation status、generics、where predicates、docs という観測可能な method contract information を保持したまま、宣言された entry に適用される catalogue lint 規則へ適合させる。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1, knowledge/adr/2026-07-04-0525-catalogue-v2-entry-lint-conformance.md#D1, knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3] [tasks: T1, T2]

## Acceptance Criteria
- [ ] [AC-01] コマンドが指名する catalogue の複数 method trait は、entry-level の全 spec anchor を各 method の担当分として宣言でき、trait_method 義務が各 method 自身の担当 anchor だけを所有する。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1, knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3] [tasks: T1, T2, T3, T4]
- [ ] [AC-02] コマンドが指名する catalogue では、包含（method の spec_refs が entry-level spec_refs の部分集合であること）、被覆（全 method の spec_refs の和が entry-level spec_refs を覆うこと）、および単一 method が entry-level の全 anchor を写すことを根拠に、構造検証で拒否しない。Add/Modify method の非空 spec_refs 要求、および親が reference または delete のとき子 spec_refs を空とする要求による拒否は、この非拒否の対象外とする。 [adr: knowledge/adr/2026-08-18-0055-entry-spec-refs-not-inventory.md#D1, knowledge/adr/2026-08-17-0340-method-declaration-action.md#D2, knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3, knowledge/adr/2026-08-18-0040-parent-forbids-method-spec-refs.md#D1] [tasks: T4, T15, T16, T17]
- [ ] [AC-03] コマンドが指名する catalogue では、action が add または modify の method は非空の spec_refs を持たなければならず、空または未宣言なら構造検証で拒否される。 [adr: knowledge/adr/2026-08-17-0340-method-declaration-action.md#D2, knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3] [tasks: T12, T11]
- [ ] [AC-04] trait_method 義務について、fulfillment 検証と生成される verifier instruction はその義務が所有する anchor だけを対象とする。その義務が所有しない anchor（別 method が所有する anchor、およびどの method も所有しない entry-only の anchor）は要求しない。複数 method が所有する共有 anchor は、その義務自身が所有する限り対象に含む。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D2, knowledge/adr/2026-08-18-0055-entry-spec-refs-not-inventory.md#D1] [tasks: T5, T17]
- [ ] [AC-10] PhaseCommandService の validate は validate 自身が担当する spec anchor に method-scoped に再 grounding され、validate の trait_method 義務の fulfillment は explain または enter の test を cross-populate せずに検証できる。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D3] [tasks: T6]
- [ ] [AC-11] PhaseCommandService の explain は explain 自身が担当する spec anchor に method-scoped に再 grounding され、explain の trait_method 義務の fulfillment は validate または enter の test を cross-populate せずに検証できる。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D3] [tasks: T7]
- [ ] [AC-12] PhaseCommandService の enter は enter 自身が担当する spec anchor に method-scoped に再 grounding され、enter の trait_method 義務の fulfillment は validate または explain の test を cross-populate せずに検証できる。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D3] [tasks: T8]
- [ ] [AC-13] `.harness/custom/review-prompts/harness-policy.md` に、track `scope-conditional-pre-review-gates-2026-07-31` に限定された PhaseCommandService の cross-populated fulfillment を受容する conditional Known Accepted Deviations 条項が残らない。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D3] [tasks: T9]
- [ ] [AC-06] コマンドが指名する catalogue が MethodDeclaration を D1 のために宣言するとき、その宣言は public-field と primitive-reference に関する適用中の catalogue lint 要件を満たす。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1, knowledge/adr/2026-07-04-0525-catalogue-v2-entry-lint-conformance.md#D1, knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3] [tasks: T1, T2]
- [ ] [AC-14] TraitEntry.methods、TypeEntry.methods、inherent_impls[].methods の各 MethodDeclaration は独立した action を持ち、add・modify・reference・delete の義務ゲート上の意味で扱われる。action を省略した method は add とし、親 entry の action は継承しない。 [adr: knowledge/adr/2026-08-17-0340-method-declaration-action.md#D1] [tasks: T10, T13, T11]
- [ ] [AC-15] method action と Add/Modify の非空 spec_refs を含む新 schema は、コマンドが path で指名する catalogue にだけ要求する。コマンドが指名しない catalogue は書き換えず、不適合として失敗させない。 [adr: knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3] [tasks: T11]
- [ ] [AC-16] コマンドが指名する catalogue で、親が reference または delete のとき子 MethodDeclaration の spec_refs が非空なら構造検証で拒否される。add または modify の method は非空 spec_refs を要求されるため、親が reference または delete のとき add または modify の method は宣言できない。 [adr: knowledge/adr/2026-08-18-0040-parent-forbids-method-spec-refs.md#D1, knowledge/adr/2026-08-17-0340-method-declaration-action.md#D2, knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3] [tasks: T14]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 34  🟡 0  🔴 0

