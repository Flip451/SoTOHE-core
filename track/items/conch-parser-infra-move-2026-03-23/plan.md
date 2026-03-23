<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# INF-20: conch-parser を domain から infrastructure に移動

conch-parser 依存を domain 層から infrastructure 層に移動する。
domain に ShellParser trait (port) を定義し、infrastructure に ConchShellParser adapter を実装。
policy.rs を parse 済みデータ受け取りに変更し、parse と evaluate を分離。
usecase hook handlers と CLI guard check に DI で parser を注入。

## Domain guard モジュール再構成

parser.rs を types.rs, port.rs, text.rs に分離。
policy.rs の check() を check_commands(&[SimpleCommand]) に変更。

- [x] Domain guard モジュールの再構成 (types.rs, port.rs, text.rs 分離) 497e684
- [x] policy.rs を check_commands(&[SimpleCommand]) + block_on_parse_error() に変更 497e684

## Infrastructure adapter 実装

conch-parser AST walking コードを infrastructure/shell/ に移動。
ConchShellParser struct で ShellParser trait を実装。

- [x] Infrastructure に ConchShellParser adapter を実装 (shell/ モジュール) 497e684

## DI 配線 & ドキュメント同期

usecase handlers と CLI に Arc<dyn ShellParser> を注入。
Cargo.toml, architecture-rules.json, convention docs を更新。

- [x] Usecase hook handlers を DI 化 (Arc<dyn ShellParser> 注入) 497e684
- [x] CLI composition root で parser 注入 (hook + guard check) 497e684
- [x] Cargo.toml 更新 & ドキュメント同期 (architecture-rules, shell-parsing, tech-stack, DESIGN) 497e684

## 検証

cargo make ci で全テスト・lint・layer check・doc sync・purity check が通ることを確認。

- [x] CI green 確認 (cargo make ci) 497e684
