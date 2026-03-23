# INF-20: conch-parser を domain から infrastructure に移動

## Feature Goal

`conch-parser` 依存を domain 層から infrastructure 層に移動し、hexagonal architecture の
依存方向ルール（domain は外部依存最小化）を遵守する。

## Scope

### In Scope

- `libs/domain/src/guard/parser.rs` の分割:
  - `SimpleCommand` → `types.rs`
  - `ShellParser` trait → `port.rs`
  - `tokenize()`, `extract_command_substitutions()` → `text.rs`
  - conch-parser AST walking コード → `libs/infrastructure/src/shell/`
- `policy.rs` の API 変更: `check(input)` → `check_commands(&[SimpleCommand])` + `block_on_parse_error()`
- usecase hook handlers の DI 化: `Arc<dyn ShellParser>` 注入
- CLI composition root: `ConchShellParser` 構築・注入（hook + guard check）
- Cargo.toml 依存移動: domain → infrastructure
- ドキュメント同期: architecture-rules.json, shell-parsing.md, tech-stack.md, DESIGN.md

### Out of Scope

- conch-parser 自体のバージョンアップやパッチ変更
- policy ロジック（ブロック対象コマンド）の変更
- 新しいガード機能の追加
- vendor/conch-parser の移動やリネーム

## Constraints

- domain 層から `conch-parser` (`conch_parser::*`) への依存を完全に除去すること
- 既存テストがすべて pass すること（振る舞いの変更なし）
- `cargo make ci` が green であること
- domain-purity CI (INF-19) が pass すること
- `cargo make check-layers` / `cargo make deny` が pass すること

## Acceptance Criteria

1. `libs/domain/Cargo.toml` に `conch-parser` 依存がないこと
2. `libs/infrastructure/Cargo.toml` に `conch-parser` 依存が追加されていること
3. `domain::guard::ShellParser` trait が定義されていること
4. `infrastructure::shell::ConchShellParser` が `ShellParser` を実装していること
5. `policy::check_commands(&[SimpleCommand])` が `&[SimpleCommand]` を受け取ること
6. usecase hook handlers が `Arc<dyn ShellParser>` を保持していること
7. CLI が composition root パターンで `ConchShellParser` を注入していること
8. `cargo make ci` が pass すること（ci-rust + verify-arch-docs + verify-tech-stack + domain-purity 含む）
9. `docs/architecture-rules.json`, `project-docs/conventions/shell-parsing.md`, `track/tech-stack.md` が更新されていること
10. `.claude/docs/DESIGN.md` の Module Structure と Key Design Decisions が更新されていること

## Design Reference

- Codex planner レビュー結果: `tmp/codex-planner-inf20.md`
- 関連 convention: `project-docs/conventions/hexagonal-architecture.md`
- 関連 convention: `project-docs/conventions/shell-parsing.md`

## Related Conventions (Required Reading)

- `project-docs/conventions/hexagonal-architecture.md`
- `project-docs/conventions/shell-parsing.md`
