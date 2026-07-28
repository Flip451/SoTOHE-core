# Testing Convention

## この文書の所有権

この規約は **利用プロジェクトが所有する**。初期値としてテンプレートから供給されるが、以後の改稿・改名・削除はプロジェクトの裁量である。ハーネスが所有するのは、規約を capability へ届ける解決機構 (`required_for` frontmatter とその resolver) と、本文が案内する gate task (`cargo make test` / `cargo make test-doc`) の定義だけである。**どうテストするか** という方針そのもの — 以下の各ルール — はこの文書にあり、プロジェクトのものである。

この文書には `required_for` frontmatter がない。つまり出荷時点では、どの capability もこの規約を必読として解決しない。特定の capability に必読として届けたい場合は、ファイル先頭へ frontmatter を足して capability ID を列挙する。

数値の目安 (カバレッジ、1 テストの実行時間) はこの文書が持つ目標値であり、機械が読む値ではない。プロジェクトの実情に合わせて書き換えてよい。モジュールサイズ上限のように gate が読む値は `architecture-rules.json` が SSoT であり、本文はその値を書き写さない。

## Purpose

Rust コードベース全体に適用するテスト規約。TDD サイクル・テスト構造・命名規則・モック・実行コマンドを定める。

## Scope

- 適用対象: `architecture-rules.json` の `layers[].path` が指すクレート配下の Rust コード (プロダクションコードおよびテストコード)
- 適用外: Rust 以外のドキュメントおよび設定ファイル

---

## Rules

### TDD Cycle

テストを先に書く (Red → Green → Refactor)。実装コードを書く前に失敗するテストを用意する。

### Coverage Goal

新規コードのカバレッジ目標は 80% 以上。

### Test Speed

ユニットテストは高速に保つ (目安: 1 テスト 50ms 未満)。

### Test Structure: Unit Tests

モジュール内に `#[cfg(test)] mod tests { ... }` ブロックを置く。

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_with_valid_format_succeeds() {
        let result = Email::new("alice@example.com");
        assert!(result.is_ok());
    }

    #[test]
    fn test_email_with_missing_at_sign_returns_error() {
        let result = Email::new("not-an-email");
        assert!(matches!(result, Err(DomainError::InvalidEmail)));
    }
}
```

### Test Structure: Integration Tests

`tests/` ディレクトリに配置する。

```rust
// tests/user_integration.rs
#[test]
fn test_register_user_flow() {
    let repo = MockUserRepository::new();
    let use_case = RegisterUserUseCase::new(Arc::new(repo));
    let result = use_case.execute(RegisterUserCommand {
        email: "alice@example.com".to_string(),
    });
    assert!(result.is_ok());
}
```

> **Note**: async runtime の採用を ADR で決定した場合は `#[tokio::test]` + `async fn` に切り替える。

### Naming Convention

テスト関数名は `test_{target}_{condition}_{expected_result}` の形式にする。

```
test_email_with_valid_format_succeeds
test_email_with_missing_at_sign_returns_invalid_email_error
```

### Mocking

外部依存 (リポジトリ、外部 API など) は port 境界でモックに差し替える。port の配置は `type-designer-kind-selection.md` R1 が定める。

モックの生成手段はプロジェクトが選ぶ。derive マクロを使う crate を採用する場合は、workspace の依存に追加したうえで ADR に記録する。テンプレートは既定の mocking crate を出荷しない。

```rust
// 例: mockall を採用した場合
use mockall::automock;

#[automock]
pub trait UserRepository: Send + Sync {
    fn find_by_email(&self, email: &Email) -> Result<Option<User>, DomainError>;
}
```

> **Note**: async runtime の採用を ADR で決定した場合は native `async fn in trait` (Rust 1.75+) または `async-trait` crate と組み合わせる。`mockall` は `#[automock]` + `#[async_trait]` の順で属性を付与する。

---

## Commands

```bash
cargo make test                 # 標準の全体テスト
cargo make test-doc             # ドキュメントテスト (必要時のみ)
```

特定のテストだけを実行したい場合は、host toolchain 上で nextest のフィルタ構文を使う。

```bash
cargo nextest run -E 'test(test_email_with_valid_format)'
```

---

## Exceptions

- テストコード (`#[cfg(test)]`) では `unwrap()` / `expect()` / `assert!()` を使ってよい。
- モジュールサイズ上限はテスト専用ファイルには適用しない (`coding-principles.md` §Module Size 参照)。

## Review Checklist

- [ ] ハッピーパスのテストがある
- [ ] エラーケース (`Err` variant) のテストがある
- [ ] テストは独立している (実行順序に依存しない)
- [ ] 外部依存 (DB, API) は port 境界でモックされている
- [ ] `unwrap()` はテスト内でのみ使用
- [ ] テスト名が `test_{target}_{condition}_{expected_result}` 形式に従っている

## Related Documents

- `coding-principles.md` — エラーハンドリング・パニック禁止ルール (テスト例外を含む)、モジュールサイズ
- `security.md` — テストで扱う機密値と、機密ディレクトリの取り扱い
- `type-designer-kind-selection.md` R1 — port (Trait) の配置
- `architecture-rules.json` — 層 id、層 path、`module_limits` の SSoT
