<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 29, yellow: 0, red: 0 }
---

# CLI→domain 直接参照禁止と usecase 経由への一本化

## Goal

- [GO-01] `architecture-rules.json` の `apps/cli` レイヤーの `may_depend_on` を `["usecase", "infrastructure"]` に変更し、cli crate の `Cargo.toml` から `domain = { path = "../../libs/domain" }` を削除することで、cli → domain の直接依存を layer policy の SSoT 更新のみで恒久的に禁止する [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D1]
- [GO-02] cli が必要とするすべてのデータ・操作を usecase 層経由で提供できるよう、usecase 側に DTO / Command / Query / Result 型および service / facade を整備し、cli が domain 型の存在を知らずに動く状態を実現する [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D1]
- [GO-03] apps/cli/src/ 配下 25 ファイルにわたる cli → domain 直接参照を専用の単一 track で一括 refactor し、移行完了時点で `cargo make deny` / `cargo make check-layers` / `bin/sotp verify check-layers` / `cargo make ci` がすべて pass する状態を確立する [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D2, knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D3]

## Scope

### In Scope
- [IN-01] `architecture-rules.json` の `apps/cli` レイヤー `may_depend_on` を `["usecase", "infrastructure"]` に変更する (domain を除外する) [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D1] [tasks: T001]
- [IN-02] `apps/cli/Cargo.toml` から `domain = { path = "../../libs/domain" }` 依存を削除する [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D1] [tasks: T001]
- [IN-03] usecase 層に、cli が消費する形に整えた DTO / Command / Query / Result 型を新設する。usecase の public API に domain 型を直接公開しない (`pub use domain::...` による re-export を禁ずる) [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D1] [tasks: T002, T003, T004, T005, T006, T007, T008]
- [IN-04] apps/cli/src/ 配下の 25 ファイルにわたる `use domain::...` による直接 import を、usecase 層の新 API 経由に置き換える。対象 symbol: `domain::Decision`, `domain::guard::ShellParser`, `domain::tddd::*`, `domain::review_v2::*`, `domain::ImplPlanReader`, `domain::CommitHash`, `domain::TrackId`, `domain::TrackMetadata`, `domain::DomainError`, `domain::derive_track_status`, `domain::schema::SchemaExporter`, `domain::hook::HookContext`, `domain::ConfidenceSignal` およびそれらに付随する型 [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D2] [tasks: T009, T010, T011, T012, T013]
- [IN-05] 移行完了後、`architecture-rules.json` の更新だけで `cargo make deny` / `cargo make check-layers` / `bin/sotp verify check-layers` の enforcement が機能することを確認する (追加の lint・grep ガード・allowance は不要) [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D3] [tasks: T014]
- [IN-06] IN-03 で導入する usecase secondary port trait (ShellParserPort, SchemaExporterPort) の実装を、既存の infrastructure adapter (ConchShellParser, RustdocSchemaExporter) に追加する。これらは usecase port と CLI composition root をつなぐ必要最小限の infrastructure 側変更であり、domain 層の実装は変更しない (OS-05 を遵守) [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D1] [tasks: T002, T004]

### Out of Scope
- [OS-01] cli → infrastructure 直接依存の禁止 (composition root での adapter wiring): `apps/cli` の `may_depend_on` は `["usecase", "infrastructure"]` のままにし、cli → infrastructure 直接参照は本 track では禁止しない。この再設計は composition root の全体見直しと組み合わせた別判断とする [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D1] [tasks: T001]
- [OS-02] usecase が domain 型を `pub use domain::...` で re-export することによる boundary 型の共有: D1 で明示的に却下された案 (Rejected Alternative B) であり採用しない [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D1] [tasks: T009]
- [OS-03] 移行の段階的実施 (command module 単位・concern 単位の複数 track 分割): D2 で却下された案 (Rejected Alternative D) であり、1 track で一括移行する方針を採る [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D2] [tasks: T001]
- [OS-04] 移行途中の期間限定 allow list や、追加の cli 専用 lint / grep ガードの導入: D3 で明示的に採用しないと決定した。`architecture-rules.json` の SSoT 更新のみで enforcement を完結させる [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D3] [tasks: T001]
- [OS-05] domain 層の内部 API surface 変更 (domain 型の field・variant・不変条件の変更): 本 track は cli からの参照経路を切り替えるものであり、domain 層の実装には変更を加えない [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D1] [tasks: T002, T003, T004, T005, T006, T007, T008]
- [OS-06] cli 以外の delivery adapter (web server / gRPC 等) の追加: 本 track は cli 経路の整備に限定し、multi-adapter 対応は future track とする [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D1] [tasks: T001]

## Constraints
- [CN-01] usecase の public API に domain 型を直接公開してはならない。cli が消費する型は usecase 自身が定義した DTO / Command / Query / Result 型に限定する (`pub use domain::...` による re-export を禁ずる) [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D1] [tasks: T002, T003, T004, T005, T006, T007, T008, T009]
- [CN-02] 追加の cli 専用 lint・grep ガード・移行期 allow list は導入しない。enforcement は `architecture-rules.json` の `may_depend_on` 更新のみで完結させる [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D3] [tasks: T001]
- [CN-03] 移行は 1 track で一括して完了させる。track 内での commit 粒度はサブタスク単位で構わないが、track 完了時に cli → domain 直接参照がゼロかつ `cargo make ci` 全 pass の状態を維持する [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D2] [tasks: T014]
- [CN-04] cli は composition root として infrastructure adapter を直接 wiring し続けてよい (`apps/cli` の `may_depend_on` に `infrastructure` を残す)。cli → domain を切る対象はオーケストレーション・整形・組み立てロジックであり、composition root での DI wiring は対象外 [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D1] [tasks: T010, T011, T012]

## Acceptance Criteria
- [ ] [AC-01] `architecture-rules.json` の `apps/cli` レイヤーの `may_depend_on` が `["usecase", "infrastructure"]` であり、`domain` が含まれない [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D1] [tasks: T001]
- [ ] [AC-02] `apps/cli/Cargo.toml` の `[dependencies]` に `domain = { path = "../../libs/domain" }` が存在しない [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D1] [tasks: T001]
- [ ] [AC-03] `apps/cli/src/` 配下のすべての Rust ファイルに `use domain::` / `domain::` の直接参照が存在しない [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D1, knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D2] [tasks: T010, T011, T012, T013]
- [ ] [AC-04] usecase の public API に `pub use domain::` 形式の re-export が存在しない [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D1] [tasks: T009]
- [ ] [AC-05] `cargo make check-layers` が pass する (cli → domain 依存が layer rule 違反として検出されない) [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D3] [tasks: T014]
- [ ] [AC-06] `cargo make deny` が pass する (`deny.toml` 経由で cli の domain 依存が reject されない状態を確認する) [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D3] [tasks: T014]
- [ ] [AC-07] `bin/sotp verify check-layers` が pass する [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D3] [tasks: T014]
- [ ] [AC-08] `cargo make ci` の全項目 (fmt-check + clippy + nextest + deny + check-layers + verify-*) が pass する [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D2, knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D3] [tasks: T014]
- [ ] [AC-09] cli の単体テストが usecase port / mock のみで完結する (domain 型を直接組み立てる test setup が不要になっている) [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D1] [tasks: T010, T011, T012]
- [ ] [AC-10] 既存 cli コマンドの外部 CLI 振る舞い (引数・出力フォーマット・exit code) が変化しない。既存の統合テストおよび acceptance test が pass する [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D2] [tasks: T014]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/hexagonal-architecture.md#CLI as Composition Root
- .claude/rules/04-coding-principles.md#Trait-Based Abstraction (Hexagonal Architecture)
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/05-testing.md#Test Structure

## Signal Summary

### Stage 1: Spec Signals
🔵 29  🟡 0  🔴 0

