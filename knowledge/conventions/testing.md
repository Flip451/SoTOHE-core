# Testing Convention

## Purpose

Rust コードベース全体に適用するテスト規約。仕様と型の約束に対するテストの対応、層ごとの責務、
テストダブルの選び方、およびテスト実行の標準を定める。

## Scope

- Applies to: `libs/`, `apps/` 配下の全 Rust コード（プロダクションコードおよびテストコード）

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_composition / cli_driver scope

- Does not apply to: `knowledge/`, `track/`, `.harness/` など非 Rust ドキュメント

> **強制先**: 強制なし (明記) — 非 Rust ドキュメントはこの規約の適用対象外

---

## Quality assurance

品質保証の正は、catalogue と spec の約束から導出された `test-obligation` と、各 obligation / edge に
対応するテストまたは理由付き waiver の binding とする。テストの本数や行数ではなく、約束を検証できる
証拠があるかで判断する。

> **強制先**: 宣言突合 (catalogue + verify) — `bin/sotp test-obligation check` / `bin/sotp test-obligation evaluate`（test-obligation gate。guarded commit は `check` を実行）

新規コードに `line coverage` の数値目標を置かず、coverage の値だけでテストの十分性を判定しない。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_composition / cli_driver scope

テストの完了判断は、該当する約束への binding と test-obligation gate の結果に基づける。

> **強制先**: 宣言突合 (catalogue + verify) — `bin/sotp test-obligation check` / `bin/sotp test-obligation evaluate`（test-obligation gate。guarded commit は `check` を実行）

---

## Test pyramid: layer responsibilities

テストは下位層を厚く、上位層を薄くする pyramid とする。下位層で個々の規則を確かめ、上位層では
境界の接続と代表的な流れだけを確かめる。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_composition / cli_driver scope

### `domain`

値オブジェクト、entity、aggregate、domain service の不変条件と純粋な domain rule を、外部 I/O なしの
unit test で確かめる。正常系だけでなく、入力境界と不変条件を破る失敗系も対象にする。

> **強制先**: review 観点 — domain scope

### `usecase`

interactor の入力、成功・失敗結果、および port を介したオーケストレーションをテストする。外部
システムには直接接続せず、必要な port は fake または recording double で置き換える。

> **強制先**: review 観点 — usecase scope

### `infrastructure`

codec、parser、evaluator、永続化 adapter など、外部表現との変換と失敗経路をテストする。仕様が定める
表現の境界、復元、拒否、およびエラーの変換を確認し、domain の規則をここで重複実装してテストしない。

> **強制先**: review 観点 — infrastructure scope

### `cli-driver` と `cli`

入力の変換、Command と Query それぞれの dispatch（happy path と、不正入力を error として返す経路の両方）、結果の表示、および exit code の境界をテストする。usecase や
infrastructure の内部規則を再テストするのではなく、adapter 間のデータとエラーの受け渡しを確認する。

> **強制先**: review 観点 — cli / cli_driver scope

### `cli-composition`

composition root では、必要な adapter と port が正しく接続されることを確認する最小限の wiring / smoke
test を置く。下位層の振る舞いを同じ形で繰り返すテストは置かない。

> **強制先**: review 観点 — cli_composition scope

---

## Fake first, mock when interaction is the specification

port や外部依存を差し替えるときは、まず deterministic な fake を使う。fake は入力に対する戻り値や
失敗を設定でき、テスト対象の結果と状態を確認できる形にする。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_composition / cli_driver scope

mock は、呼び出しそのものが仕様に含まれる場合だけ使う。たとえば呼び出し回数、順序、引数、retry、
timeout、cancel の扱いが仕様で定められている場合に限り、mock の期待値を検証する。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_composition / cli_driver scope

相互作用が仕様でない場合は、mock の呼び出し期待値を実装詳細の検査に使わず、fake または recording
double が記録した構造化された入力・結果・状態を検証する。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_composition / cli_driver scope

---

## Property-based testing

codec には、仕様が定める値を生成して encode / decode の不変条件（たとえば round-trip）を確かめる
property-based test を置く。境界値や不正な表現が仕様どおりに拒否されることも確認する。

> **強制先**: review 観点 — infrastructure scope

parser には、生成した入力について、受理された結果が typed な境界へ対応し、受理できない入力が仕様
どおりに失敗することを確かめる property-based test を置く。

> **強制先**: review 観点 — usecase / infrastructure / cli / cli_driver scope

evaluator には、生成した入力と状態について、仕様の不変条件を保った判定、結果の一貫性、および失敗時
の結果を確かめる property-based test を置く。

> **強制先**: review 観点 — domain / usecase / infrastructure scope

property-based test で得たテストも、対象の約束を検証する場合は該当する `test-obligation` の binding
に含める。property の例だけで binding のない約束を済ませたことにはしない。

> **強制先**: 宣言突合 (catalogue + verify) — `bin/sotp test-obligation check` / `bin/sotp test-obligation evaluate`（test-obligation gate。guarded commit は `check` を実行）

---

## Source assertions and dependency constraints

自分の source file を読み込み、その部分文字列の有無を assert して、依存関係や実装の振る舞いを証明
してはならない。`include_str!` や source の文字列検索を、テストの証拠として使わない。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_composition / cli_driver scope

crate の依存方向と許可される依存は `architecture-rules.json` に宣言し、source の文字列 assert ではなく
`cargo make check-layers` で確認する。

> **強制先**: 機械 lint — `cargo make check-layers`（`architecture-rules.json`）

振る舞いや相互作用を確認する場合は、recording double が記録した呼び出し、引数、順序、結果などの
構造化データを assert する。source の部分文字列は振る舞いの証拠とみなさない。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_composition / cli_driver scope

---

## Test design

各テストは独立して実行でき、実行順序、共有 mutable state、または外部ネットワークの状態に依存させない。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_composition / cli_driver scope

テスト関数名は `test_{target}_{condition}_{expected_result}` の形式にする。

```text
test_email_with_valid_format_succeeds
test_email_with_missing_at_sign_returns_invalid_email_error
```

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_composition / cli_driver scope

テストコードでは `unwrap()` / `expect()` / `assert!()` を使ってよいが、プロダクションコードのパニック
禁止を緩める理由にしてはならない。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_composition / cli_driver scope

---

## Commands

標準の Rust テストは `cargo make test` で実行する。この task は nextest を使い、全対象を同じ条件で
実行する。

> **強制先**: 機械 lint — `cargo make test`（nextest）

---

## Review Checklist

- [ ] 変更した仕様・型の約束に対して、対応するテストまたは理由付き waiver の binding がある
  > **強制先**: 宣言突合 (catalogue + verify) — `bin/sotp test-obligation check` / `bin/sotp test-obligation evaluate`（test-obligation gate。guarded commit は `check` を実行）
- [ ] テスト責務が変更対象の layer にあり、下位層を厚く上位層を薄くする pyramid を崩していない
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_composition / cli_driver scope
- [ ] 差し替えには fake を優先し、mock の期待値は相互作用が仕様の場合だけ使っている
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_composition / cli_driver scope
- [ ] codec、parser、evaluator の変更に property-based test があり、対象の約束へ binding されている
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver scope
- [ ] 自ソースの部分文字列 assert で依存関係や振る舞いを検証していない
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_composition / cli_driver scope
- [ ] 依存方向の確認を source assert で代用せず、`cargo make check-layers` に任せている
  > **強制先**: 機械 lint — `cargo make check-layers`（`architecture-rules.json`）
- [ ] テストが独立しており、正常系と仕様上必要な失敗系を確認している
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_composition / cli_driver scope
- [ ] テスト関数名が `test_{target}_{condition}_{expected_result}` 形式である
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_composition / cli_driver scope

## Decision Reference

- `knowledge/adr/README.md`: ADR 索引
- `knowledge/conventions/coding-principles.md`: エラーハンドリング・パニック禁止ルール
- `knowledge/conventions/type-designer-kind-selection.md` R1: 型の role × layer 配置
