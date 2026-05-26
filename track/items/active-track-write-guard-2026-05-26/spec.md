<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 21, yellow: 0, red: 0 }
---

# 完了済みトラック保護を frozen から現在ブランチ紐付きバリデーションへ置換

## Goal

- [GO-01] アーティファクトを書き換えるアクション（catalogue-spec-signals / type-signals / sync-views など）に対するトラック保護機構を、track status（done か否か）ベースの frozen ブロックから、現在の git ブランチに紐づくトラックかどうかを判定基準とするバリデーションへ置き換える。これにより、full-cycle 途中で status=done になったトラックでも現在ブランチが当該トラックブランチである限りアーティファクト更新が通るようにし、かつ現在ブランチに紐づかない完了済みトラックのアーティファクトは引き続き保護する [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1]

## Scope

### In Scope
- [IN-01] アーティファクトを書き換えるすべてのアクション（catalogue-spec-signals / type-signals / sync-views、および同種の将来的なアクション）から、status=done/archived ベースの frozen ブロック判定ロジックを削除する [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T001, T002, T003, T004]
- [IN-02] 削除した frozen ブロックの代わりに、「対象トラックのブランチ名（`track/<id>`）が現在の git ブランチと一致する場合のみアクションを許容する」というブランチベースのバリデーションを導入する。一致しない場合は明示的なエラーで拒否する（fail-closed） [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T001, T002, T003]
- [IN-03] 背景 ADR `2026-04-15-1012-catalogue-active-guard-fix.md` が導入した catalogue active-track guard（`execute_type_signals` の status-based guard）を、今回のブランチベースバリデーションに置き換える。status ベースの frozen 判定ロジックを除去し、ブランチ紐付きバリデーションで同等の保護を実現する [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1, knowledge/adr/2026-04-15-1012-catalogue-active-guard-fix.md#2026-04-15-1012-catalogue-active-guard-fix_grandfathered] [tasks: T002, T003]
- [IN-04] sync-views コマンドが内部的に保持している `is_done_or_archived` ガード（`libs/infrastructure/src/track/render.rs` の文字列ベース `matches!` 判定）をブランチベースバリデーションに置き換える [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T004]
- [IN-05] ブランチベースバリデーションに対するテストを追加する: 現在ブランチが当該トラックブランチと一致するケース（許容される）、一致しないケース（拒否される）の両方を検証する [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T005]

### Out of Scope
- [OS-01] 複数トラックを同時に操作するユースケースへの対応: 「現在ブランチ = 単一トラック」という前提が崩れる複数トラック並行操作への対応は、本 track のスコープ外とする（ADR Reassess When に記載） [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1]
- [OS-02] CI 環境での detached HEAD やブランチ外からのバッチ処理への対応: ブランチに紐づかない文脈でのトラック操作が必要になる場合の判定基準拡張は本 track のスコープ外とする（ADR Reassess When に記載） [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1]
- [OS-03] rejected alternative A（frozen を残したまま done 判定に例外条件を追加）の実装: status ベース判定に条件分岐を重ねるアプローチは採用しない [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1]
- [OS-04] rejected alternative B（full-cycle の done マークを commit 後に遅延させる）の実装: done マークのタイミング変更は本 track のスコープ外とする。保護機構の置き換えのみを行う [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1]
- [OS-05] task の done マーク付けのタイミング変更: full-cycle が task を done マークするタイミングの変更は本 track のスコープ外とする（対症療法に留まるため、ADR で rejected alternative B として却下済み） [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1]

## Constraints
- [CN-01] ブランチベースのバリデーションは fail-closed で実装する: 対象トラックが現在のブランチに紐づかない場合、サイレントスキップや警告に留めず、明示的なエラーで拒否する [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T002, T003, T004]
- [CN-02] バリデーションの配置はヘキサゴナルアーキテクチャの層依存方向に従う: ブランチ状態の読み取りはインフラ層の責務であり、バリデーションロジックは apps/cli 層または infrastructure 層に置き、domain 層を変更しない [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [conv: knowledge/conventions/hexagonal-architecture.md#Layer Dependencies] [tasks: T001]
- [CN-03] アーティファクトを書き換えるすべての経路（catalogue-spec-signals / type-signals / sync-views など）に対してブランチベースバリデーションを一貫して適用する。経路ごとに保護レベルが異なる非対称な状態を残さない [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T002, T004]
- [CN-04] status フィールドを保護判定の主たる基準として使い続ける実装（status ベースの frozen 機構の温存）は行わない。判定基準を「現在ブランチとの紐付き」に一本化する [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [conv: knowledge/conventions/workflow-ceremony-minimization.md#Rules] [tasks: T001, T002, T003]

## Acceptance Criteria
- [ ] [AC-01] 現在ブランチが `track/<id>` である状態で、そのトラックの status が done であっても、catalogue-spec-signals / type-signals / sync-views が正常に実行されアーティファクトが更新される（full-cycle 途中の done マーク問題が解消される） [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T002, T003, T004]
- [ ] [AC-02] 現在ブランチが `track/<id-A>` である状態で、別トラック `<id-B>` に対して catalogue-spec-signals / type-signals / sync-views を実行しようとすると、明示的なエラーで拒否される（`<id-B>` の status に関わらず） [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T002, T003, T004]
- [ ] [AC-03] 「Completed tracks are frozen」という文言でブロックされていた操作（背景 ADR 2026-04-15-1012 の D1 ガードが出力していたメッセージ相当）が、ブランチベースバリデーションの導入後は発生しなくなる [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1, knowledge/adr/2026-04-15-1012-catalogue-active-guard-fix.md#2026-04-15-1012-catalogue-active-guard-fix_grandfathered] [tasks: T002, T003]
- [ ] [AC-04] `cargo make track-commit-message` の pre-commit フック内で実行される type signals / catalogue-spec-signals が、現在ブランチが当該トラックブランチである限り status=done のトラックでも「skipped (track is done — frozen)」とならずに実行される [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T003]
- [ ] [AC-05] アーティファクトを書き換える複数の経路（catalogue-spec-signals / type-signals / sync-views）すべてで同じブランチベースバリデーションが適用される。いずれかの経路だけが旧 frozen ロジックを保持する非対称状態が存在しない [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T002, T003, T004]
- [ ] [AC-06] `cargo make ci`（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T005]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/source-attribution.md#Source Tag Types
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/05-testing.md#Test Structure

## Signal Summary

### Stage 1: Spec Signals
🔵 21  🟡 0  🔴 0

