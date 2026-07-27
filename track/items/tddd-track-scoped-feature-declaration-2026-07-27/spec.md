<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 26, yellow: 0, red: 0 }
---

# TDDD chain ③ の rustdoc 抽出を track 単位の feature 宣言に基づかせる

## Goal

- [GO-01] 各 track が、各 layer crate を TDDD 抽出時にどの Cargo feature でビルドするかへ対応づける、commit 対象の専用宣言を持つ [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D1]
- [GO-02] baseline 取得と実測取得は同一の feature 宣言内容を観測し、feature-gated public item を TDDD の抽出面と signal 評価の対象にできる [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D3]

## Scope

### In Scope
- [IN-01] track 配下の commit 対象の専用成果物に、各 layer とその crate をビルド時に有効化する Cargo feature のリストとの対応を宣言する。feature を必要としない layer も空リストで明示する [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D1] [tasks: T001, T002]
- [IN-02] feature 宣言は type-designer capability が Phase 2 パイプラインの最初に、baseline 取得の直前に author する [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D2] [tasks: T003]
- [IN-03] rustdoc を実際に呼び出す baseline 取得と実測取得は feature 宣言を入力として読み、同一の宣言内容を観測する [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D3] [tasks: T002, T003, T004]
- [IN-04] 型を変更しない track を含むすべての track で feature 宣言を必須にし、型を変更しない track も空の catalogue と feature 宣言を持つ [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D4] [tasks: T001, T002, T003]
- [IN-05] 宣言した feature が対象 crate の Cargo.toml に存在すること、および catalogue の feature-gated type が宣言済み feature の下にあることを gate で検証する [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D5] [tasks: T002, T004]

### Out of Scope
- [OS-01] feature 宣言成果物の具体的なファイル名および JSON schema の決定。これらは Phase 2 の型設計で決定する [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D1] [tasks: T001]
- [OS-02] baseline 取得と実測取得を同一の宣言内容へ拘束する具体的な mechanism の決定。これは下流の型設計に委ねる [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D3] [tasks: T003, T004]
- [OS-03] feature 宣言を渡すための新規 CLI subcommand、argument、flag、または command-line 経路の追加 [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D7] [tasks: T003, T004, T005]
- [OS-04] 新たに可視化される既存 public item を catalogue 化せずに除外する grandfathering list または互換経過措置 [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D6] [tasks: T005]

## Constraints
- [CN-01] rustdoc を呼び出す baseline 取得または実測取得で feature 宣言が不在なら fail-closed で停止し、不在を暗黙の空宣言として扱わない [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D3] [tasks: T002, T003, T004]
- [CN-02] 既に永続化された JSON を読むだけの command は feature 宣言を要求しない [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D3] [tasks: T003, T004]
- [CN-03] declared feature が対象 crate の Cargo.toml に存在しない場合、または catalogue が undeclared feature 配下の type を記載する場合は、いずれも fail-closed で拒否する [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D5] [tasks: T001, T002, T004]
- [CN-04] feature を最初に宣言する track は、その feature により新たに可視化される既存 public item を catalogue に整備する [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D6] [tasks: T005]
- [CN-05] 既存 command の argument syntax、stdout/stderr output format、exit-code meaning を変更せず、feature を command line から入力する経路を設けない [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D7] [tasks: T003, T004, T005]
- [CN-06] feature 宣言では catalogue に declare する type を抽出面へ可視化するために必要な feature を選択する。catalogue entry がない feature-gated public item は、その item だけを理由に track が feature を宣言することを要求しない [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D2]

## Acceptance Criteria
- [ ] [AC-01] track には commit 対象の feature 宣言成果物があり、全 layer の feature リストを含み、feature を持たない layer は空リストとして宣言されている [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D1] [tasks: T001, T002, T003]
- [ ] [AC-02] Phase 2 では type-designer が feature 宣言を baseline 取得前に author する [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D2] [tasks: T003]
- [ ] [AC-03] feature 宣言を欠く track で baseline 取得または実測取得を実行すると、rustdoc を呼び出さず fail-closed の失敗として終了する [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D3] [tasks: T002, T003, T004]
- [ ] [AC-04] baseline 取得と実測取得は同一の feature 宣言内容を観測し、宣言された feature-gated public item は両方の extracted surface に現れる [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D3] [tasks: T003, T004]
- [ ] [AC-05] 対象 crate の Cargo.toml に存在しない feature を宣言した track は gate で fail-closed に拒否される [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D5] [tasks: T002, T004]
- [ ] [AC-06] catalogue に記載した feature-gated type の feature を track が宣言していない場合、track は gate で fail-closed に拒否される [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D5] [tasks: T002, T004]
- [ ] [AC-07] feature を最初に宣言する track では、その feature により可視化された既存 public item が catalogue に整備され、implementation-to-catalogue signal を blue と評価できる [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D6] [tasks: T005]
- [ ] [AC-08] 導入後も既存 command の subcommand、argument syntax、stdout/stderr output format、exit-code meaning は変わらず、feature を command line から渡す route は存在しない [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D7] [tasks: T003, T004, T005]
- [ ] [AC-09] layer の Cargo feature list の各要素は Cargo feature-name grammar に適合しなければならず、malformed token は fail-closed で拒否される。これは token の構文妥当性を検証する基準であり、構文上は妥当でも対象 crate の Cargo.toml に存在しない feature を拒否する AC-05 とは別の failure mode である [adr: knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md#D1] [tasks: T001]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/testing.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/type-designer-kind-selection.md#R1. Role × Layer Placement
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/no-backward-compat.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 26  🟡 0  🔴 0

