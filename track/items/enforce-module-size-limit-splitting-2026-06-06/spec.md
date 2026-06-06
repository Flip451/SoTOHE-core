<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 20, yellow: 0, red: 0 }
---

# モジュールサイズ制限の厳格化と分割リファクタリング

## Goal

- [GO-01] プロダクションコードのモジュールサイズ上限（700 行）を CI ゲートで機械的に強制し、規約文書の目安と実態の乖離を解消する。`bin/sotp verify` に行数チェックサブコマンドを追加して `cargo make ci` に組み込み、700 行を超えるプロダクションコードファイルが存在する場合に CI を失敗させる [adr: knowledge/adr/2026-06-06-1609-enforce-module-size-limit-splitting.md#D1]
- [GO-02] 本トラック内で既存の 700 行超過ファイル（domain 4 件・usecase 8 件・infrastructure 17 件以上、合計 29 件以上）を全て分割リファクタリングし、allowlist による免除を行わず CI ゲート導入と同時に全ファイルを制限内に収める [adr: knowledge/adr/2026-06-06-1609-enforce-module-size-limit-splitting.md#D2]

## Scope

### In Scope
- [IN-01] `bin/sotp verify` に行数チェックサブコマンドを追加する。サブコマンドはプロダクションコードファイルを走査し、テスト除外ルール（D3）を適用した上で 700 行を超えるファイルをリストアップして非ゼロ終了する [adr: knowledge/adr/2026-06-06-1609-enforce-module-size-limit-splitting.md#D1] [tasks: T001]
- [IN-02] `cargo make ci` のパイプラインに行数チェックサブコマンドを組み込む。CI 失敗時はどのファイルが何行超過しているかを出力する [adr: knowledge/adr/2026-06-06-1609-enforce-module-size-limit-splitting.md#D1] [tasks: T001]
- [IN-03] 既存の 700 行超過ファイル全件（domain / usecase / infrastructure の各層）を責務に従って複数モジュールへ分割リファクタリングする。分割後は各ファイルが 700 行以下となり、import パスの変更に伴う全参照箇所を同時に修正する [adr: knowledge/adr/2026-06-06-1609-enforce-module-size-limit-splitting.md#D2] [tasks: T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012, T013, T014, T015, T016, T017, T018, T019, T020]
- [IN-04] テストコードの行数除外ルール（`#[cfg(test)] mod tests` ブロック内の行、`*_tests.rs` 等のテスト専用ファイル、`tests/` ディレクトリ配下の統合テスト）を行数チェックサブコマンドの判定ロジックに実装する [adr: knowledge/adr/2026-06-06-1609-enforce-module-size-limit-splitting.md#D3] [tasks: T001]

### Out of Scope
- [OS-01] allowlist による既存超過ファイルの免除。CI ゲート導入と分割を同一トラックで完結させるため、allowlist は採用しない [adr: knowledge/adr/2026-06-06-1609-enforce-module-size-limit-splitting.md#Rejected Alternatives]
- [OS-02] 700 行上限値の変更。`04-coding-principles.md` で定義済みの上限値をそのまま CI ゲートに反映する。上限値の見直しは Reassess When 条件に基づいて別 ADR で行う [adr: knowledge/adr/2026-06-06-1609-enforce-module-size-limit-splitting.md#Reassess When]
- [OS-03] テストコードのサイズ制限導入。`04-coding-principles.md` のテスト除外ルールを現行のまま維持し、テストコードは行数チェックの対象としない [adr: knowledge/adr/2026-06-06-1609-enforce-module-size-limit-splitting.md#D3]
- [OS-04] 段階的な分割（一部ファイルを後続トラックへ持ち越す）。本トラックで全超過ファイルを一括処理し、CI ゲートと分割を同時完了させる [adr: knowledge/adr/2026-06-06-1609-enforce-module-size-limit-splitting.md#D2]

## Constraints
- [CN-01] 行数チェックロジックは `bin/sotp verify` サブコマンドとして Rust native に実装する。`cargo make` の `@shell` スクリプトに行数カウントのロジックを持ち込まない [adr: knowledge/adr/2026-06-06-1609-enforce-module-size-limit-splitting.md#D1] [conv: knowledge/conventions/enforce-by-mechanism.md#Rules] [tasks: T001]
- [CN-02] テスト除外の判定基準は `04-coding-principles.md` の Module Size セクションで定義済みのルールと完全に一致させる。除外対象は「`#[cfg(test)] mod tests` ブロック内の行」「`*_tests.rs` 等のテスト専用ファイル」「`tests/` ディレクトリ配下の統合テスト」の 3 種類とし、他の除外ルールを追加・変更しない [adr: knowledge/adr/2026-06-06-1609-enforce-module-size-limit-splitting.md#D3] [tasks: T001]
- [CN-03] 分割リファクタリングは公開 API を変更しない（public シグネチャの互換性を維持する）。モジュールのパス変更に伴う `pub use` 再エクスポートで既存の呼び出し側を壊さないよう調整する。ただし no-backward-compat convention が適用されるため、内部 (pub(crate)) パスの破壊的変更は許容する [adr: knowledge/adr/2026-06-06-1609-enforce-module-size-limit-splitting.md#D2] [conv: knowledge/conventions/no-backward-compat.md#Rules] [tasks: T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012, T013, T014, T015, T016, T017, T018, T019, T020]
- [CN-04] 行数チェックサブコマンドは、超過ファイルが存在する場合に fail-closed（非ゼロ終了）で CI をブロックする。超過ファイルが 0 件のときのみゼロ終了する [adr: knowledge/adr/2026-06-06-1609-enforce-module-size-limit-splitting.md#D1] [tasks: T001]

## Acceptance Criteria
- [ ] [AC-01] `bin/sotp verify module-size`（または相当するサブコマンド）が実装されており、プロダクションコードのファイルを走査してテスト除外ルールを適用した行数を計算し、700 行を超えるファイルを検出して非ゼロ終了する。700 行以下のみの場合はゼロ終了する [adr: knowledge/adr/2026-06-06-1609-enforce-module-size-limit-splitting.md#D1] [tasks: T001]
- [ ] [AC-02] `cargo make ci` のパイプラインに行数チェックが組み込まれており、700 行超過ファイルが存在する状態で `cargo make ci` を実行すると失敗する [adr: knowledge/adr/2026-06-06-1609-enforce-module-size-limit-splitting.md#D1] [tasks: T001]
- [ ] [AC-03] ADR の Context に記載された超過ファイル群（domain 4 件・usecase 8 件・infrastructure 17 件以上）が全て 700 行以下に分割されており、`bin/sotp verify module-size` がゼロ終了する [adr: knowledge/adr/2026-06-06-1609-enforce-module-size-limit-splitting.md#D2] [tasks: T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012, T013, T014, T015, T016, T017, T018, T019, T020]
- [ ] [AC-04] 分割後も `cargo make ci`（fmt-check + clippy + nextest + deny + check-layers + verify-* を含む全体 CI）が pass する。分割によってコンパイルエラー・既存テスト失敗・レイヤー依存違反が発生しない [adr: knowledge/adr/2026-06-06-1609-enforce-module-size-limit-splitting.md#D2] [tasks: T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012, T013, T014, T015, T016, T017, T018, T019, T020]
- [ ] [AC-05] `#[cfg(test)] mod tests` ブロック内の行・`*_tests.rs` 等のテスト専用ファイル・`tests/` 配下の統合テストが行数チェックの対象外となっており、テストコードを大量に含むファイルが超過ファイルとして誤検出されない [adr: knowledge/adr/2026-06-06-1609-enforce-module-size-limit-splitting.md#D3] [tasks: T001]
- [ ] [AC-06] allowlist ファイル（例: `.module-size-allowlist` や設定ファイル内の免除リスト）が存在しない。超過ファイルを免除する仕組みが実装に含まれない [adr: knowledge/adr/2026-06-06-1609-enforce-module-size-limit-splitting.md#Rejected Alternatives] [tasks: T001]

## Related Conventions (Required Reading)
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/no-backward-compat.md#Rules
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- .claude/rules/04-coding-principles.md#Module Size
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/04-coding-principles.md#Error Handling: Result and ? Operator

## Signal Summary

### Stage 1: Spec Signals
🔵 20  🟡 0  🔴 0

