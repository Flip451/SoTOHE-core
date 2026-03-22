# verification: usecase-purity-ci (INF-15)

## Scope Verified

- spec.md の acceptance criteria 1-4 をカバー

## Manual Verification Steps

1. `bin/sotp verify usecase-purity` が warning ゼロで pass すること
2. テスト入力で禁止パターン（std::fs::, chrono::Utc::now, println!）が検出されること
3. `cargo make ci` が全て pass すること
4. `#[cfg(test)]` ブロック内の禁止パターンは無視されること

## Result / Open Issues

1. `bin/sotp verify usecase-purity` 実行: 既存 2 件の warning を検出。warning-only のため CI はブロックしない。
   - `libs/usecase/src/pr_review.rs:78` — `std::io::Error`（I/O 型の usecase 漏洩）
   - `libs/usecase/src/pr_review.rs:373` — `std::fs::read_to_string`（ファイル I/O）
2. syn AST ベースの検出エンジン（2パス: UseCollector + PurityVisitor）。27 unit tests（infrastructure）+ 2 CLI wiring tests 全通過。
   - 禁止パス prefix: `std::fs`, `std::net`, `std::process`, `std::io`, `std::env`
   - 禁止パス exact: `chrono::Utc::now`, `std::time::SystemTime`, `std::time::Instant`
   - 禁止マクロ: `println!`, `eprintln!`, `print!`, `eprint!`
   - use import 検出: 通常 import, rename (`as`), `{self}`, glob (`*`) 対応
   - `#[cfg(test)]` / `#[test]` 除外: item, impl item, trait item レベル
   - コメント・文字列リテラル: syn が自動処理（false positive なし）
3. `cargo make ci` 全通過（verify-usecase-purity-local が ci-local + ci-container に組み込み済み）。

- Open: INF-16 で `pr_review.rs` の violation を CLI 層に移動後、INF-17 で warning → error 昇格予定。

## Verified At

- 2026-03-22
