<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.3"
signals: { blue: 32, yellow: 0, red: 0 }
---

# trait_method obligations own method-scoped anchors

## Goal

- [GO-01] trait_method ごとの test-obligation が自 method の仕様 anchor だけを所有・検証できるようにし、複数 method の ApplicationService trait を method-relevant な fulfillment で test-obligation gate に通せるようにする。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1, knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D2]
- [GO-02] D1 の method-level spec_refs をコマンドが指名する catalogue で宣言・構造検証できるよう、対象となる MethodDeclaration の宣言を、宣言された entry に適用される catalogue lint 規則に適合する状態にする。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1, knowledge/adr/2026-07-04-0525-catalogue-v2-entry-lint-conformance.md#D1, knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3]

## Scope

### In Scope
- [IN-01] trait method ごとの spec anchor 担当を catalogue で宣言できるようにし、trait_method 義務が entry-level の全 anchor を一律に継承せず、自 method の担当分だけを所有するようにする。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1] [tasks: T1, T2, T3, T4]
- [IN-02] method-level の担当 anchor が entry-level の目録の部分集合であり、全 method の担当分が目録全体を覆うことを、単一 method の明示規則を含めて構造検証する。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1] [tasks: T4]
- [IN-03] fulfillment 検証と verifier instruction を、各 trait_method 義務が所有する anchor に限定する。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D2] [tasks: T5]
- [IN-05] D1 のためにコマンドが指名する catalogue へ宣言する MethodDeclaration を、宣言された entry に適用される catalogue lint 規則へ適合させる。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1, knowledge/adr/2026-07-04-0525-catalogue-v2-entry-lint-conformance.md#D1, knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3] [tasks: T1]
- [IN-09] PhaseCommandService の validate を、validate 自身が担当する spec anchor へ method-scoped に再 grounding し、validate の fulfillment を validate 自身の test で実証できる binding に戻す。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D3] [tasks: T6]
- [IN-10] PhaseCommandService の explain を、explain 自身が担当する spec anchor へ method-scoped に再 grounding し、explain の fulfillment を explain 自身の test で実証できる binding に戻す。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D3] [tasks: T7]
- [IN-11] PhaseCommandService の enter を、enter 自身が担当する spec anchor へ method-scoped に再 grounding し、enter の fulfillment を enter 自身の test で実証できる binding に戻す。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D3] [tasks: T8]
- [IN-12] `.harness/custom/review-prompts/harness-policy.md` から、track `scope-conditional-pre-review-gates-2026-07-31` に限定された PhaseCommandService の cross-populated fulfillment を受容する conditional Known Accepted Deviations 条項だけを撤去し、他の deviation 記述や他 track の記録は変更しない。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D3] [tasks: T9]
- [IN-13] TraitEntry.methods、TypeEntry.methods、inherent_impls[].methods の MethodDeclaration に独立した action を導入し、action 省略を add として扱い、親 entry action を継承しない。 [adr: knowledge/adr/2026-08-17-0340-method-declaration-action.md#D1] [tasks: T10]
- [IN-14] コマンドが指名する catalogue で、add または modify の method に非空の spec_refs を要求し、兄弟 method による代替を認めない。 [adr: knowledge/adr/2026-08-17-0340-method-declaration-action.md#D2, knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3] [tasks: T12, T11]

### Out of Scope
- [OS-01] コマンドが指名しない既存 catalogue 全体を新 schema へ遡及的に書き換えたり不適合として失敗させたりすることは本 track の対象外とする。 [adr: knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3] [tasks: T4, T11]
- [OS-02] cross-populated fulfillment を entry 単位の正規形として恒久化すること、または merged track にある歴史的な cross-populated binding record を書き換えることは本 track の対象外とする。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1] [tasks: T4, T5]
- [OS-03] trait_method 義務を廃止して型単位の粗い義務へ置き換えることは本 track の対象外とする。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1] [tasks: T4]
- [OS-04] D1 の coverage 検証を通さずに、fulfillment 検証から未担当の entry-level anchor を除外することは本 track の対象外とする。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D2] [tasks: T5]
- [OS-05] MethodDeclaration 以外の legacy catalogue entry を一律に remediation すること、または宣言済み entry への lint 適用を免除する一般方針を導入することは本 track の対象外とする。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1] [tasks: T1, T3]

## Constraints
- [CN-01] entry-level spec anchors は trait と仕様を結ぶ完全な目録として維持し、method-level の担当宣言はその部分集合でなければならない。anchor は複数 method が担当してよいが、少なくとも一つの method に担当されなければならない。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1] [tasks: T4, T12]
- [CN-02] method action と Add/Modify の非空 spec_refs を含む新 schema は、コマンドが path で指名する catalogue にだけ適用する。コマンドが指名しない catalogue は書き換えず、不適合として失敗させない。 [adr: knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3] [tasks: T4, T11]
- [CN-03] fulfillment の method-scoped 検証は、D1 の coverage 検証後にのみ行い、entry-level anchor の未担当による検証漏れを許さない。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D2] [tasks: T5]
- [CN-05] MethodDeclaration の lint-compliance remediation は、D1 の method-level spec_refs をコマンドが指名する catalogue で宣言・評価できる状態にするため、既存宣言の name、receiver、params、returns、async/default-implementation status、generics、where predicates、docs という観測可能な method contract information を保持したまま、宣言された entry に適用される catalogue lint 規則へ適合させる。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1, knowledge/adr/2026-07-04-0525-catalogue-v2-entry-lint-conformance.md#D1, knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3] [tasks: T1, T2]

## Acceptance Criteria
- [ ] [AC-01] コマンドが指名する catalogue の複数 method trait は、entry-level の全 spec anchor を各 method の担当分として宣言でき、trait_method 義務が各 method 自身の担当 anchor だけを所有する。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1, knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3] [tasks: T1, T2, T3, T4]
- [ ] [AC-02] コマンドが指名する catalogue で、method が entry-level に存在しない anchor を担当する宣言、または全 method の担当分で entry-level anchor を覆わない宣言は構造検証で拒否される。entry-level anchor を持つ単一 method trait の未宣言または部分宣言も拒否される。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1, knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3] [tasks: T4]
- [ ] [AC-03] コマンドが指名する catalogue では、action が add または modify の method は非空の spec_refs を持たなければならず、空または未宣言なら構造検証で拒否される。 [adr: knowledge/adr/2026-08-17-0340-method-declaration-action.md#D2, knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3] [tasks: T12, T11]
- [ ] [AC-04] D1 の割り当て検証が通った trait_method 義務について、fulfillment 検証と生成される verifier instruction はその義務が所有する anchor だけを対象とし、その義務が所有しない anchor（別 method だけが担当する anchor）を要求しない。別 method も担当する共有 anchor は、その義務自身が担当する限り対象に含む。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D2] [tasks: T5]
- [ ] [AC-10] PhaseCommandService の validate は validate 自身が担当する spec anchor に method-scoped に再 grounding され、validate の trait_method 義務の fulfillment は explain または enter の test を cross-populate せずに検証できる。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D3] [tasks: T6]
- [ ] [AC-11] PhaseCommandService の explain は explain 自身が担当する spec anchor に method-scoped に再 grounding され、explain の trait_method 義務の fulfillment は validate または enter の test を cross-populate せずに検証できる。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D3] [tasks: T7]
- [ ] [AC-12] PhaseCommandService の enter は enter 自身が担当する spec anchor に method-scoped に再 grounding され、enter の trait_method 義務の fulfillment は validate または explain の test を cross-populate せずに検証できる。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D3] [tasks: T8]
- [ ] [AC-13] `.harness/custom/review-prompts/harness-policy.md` に、track `scope-conditional-pre-review-gates-2026-07-31` に限定された PhaseCommandService の cross-populated fulfillment を受容する conditional Known Accepted Deviations 条項が残らない。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D3] [tasks: T9]
- [ ] [AC-06] コマンドが指名する catalogue が MethodDeclaration を D1 のために宣言するとき、その宣言は public-field と primitive-reference に関する適用中の catalogue lint 要件を満たす。 [adr: knowledge/adr/2026-08-13-1720-test-obligation-method-anchor-ownership.md#D1, knowledge/adr/2026-07-04-0525-catalogue-v2-entry-lint-conformance.md#D1, knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3] [tasks: T1, T2]
- [ ] [AC-14] TraitEntry.methods、TypeEntry.methods、inherent_impls[].methods の各 MethodDeclaration は独立した action を持ち、add・modify・reference・delete の義務ゲート上の意味で扱われる。action を省略した method は add とし、親 entry の action は継承しない。 [adr: knowledge/adr/2026-08-17-0340-method-declaration-action.md#D1] [tasks: T10, T13, T11]
- [ ] [AC-15] method action と Add/Modify の非空 spec_refs を含む新 schema は、コマンドが path で指名する catalogue にだけ要求する。コマンドが指名しない catalogue は書き換えず、不適合として失敗させない。 [adr: knowledge/adr/2026-08-17-0340-method-declaration-action.md#D3] [tasks: T11]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 32  🟡 0  🔴 0

