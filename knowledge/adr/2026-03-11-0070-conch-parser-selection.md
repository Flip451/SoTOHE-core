---
adr_id: 2026-03-11-0070-conch-parser-selection
decisions:
  - id: 2026-03-11-0070-conch-parser-selection_grandfathered
    status: accepted
    grandfathered: true
---
# conch-parser for shell AST (vendored, patched)

## Status

Accepted

## Context

Shell コマンドの AST パースにどのクレートを使うか。

## Decision

conch-parser 0.1.1 を vendored + patched で採用。Full POSIX AST、minimal deps（void only）、構造的な env var/command 分離。

## Rejected Alternatives

- Hand-written parser: エッジケースの増殖が止められない
- tree-sitter-bash: C 依存。workspace の純粋性を損なう
- brush-parser: 依存が重い

## Consequences

- Good: Full POSIX shell AST。構造的にコマンドと環境変数を分離可能
- Good: 依存が minimal（void のみ）
- Bad: vendor ディレクトリ管理。上流パッチ追従が手動（SEC-13）
- Bad: Rust 2015 Edition + `#![allow(warnings)]`

## Reassess When

- brush-parser の依存が軽量化された場合
- tree-sitter-bash の Rust バインディングが C 依存なしで使えるようになった場合
- SEC-13（conch-parser ベンダリング保守方針）で代替検討が必要と判断された場合
