---
adr_id: 2026-03-11-0060-shell-guard-in-domain
decisions:
  - id: 2026-03-11-0060-shell-guard-in-domain_grandfathered
    status: accepted
    grandfathered: true
---
# Shell guard を domain 層に配置 (no trait)

## Status

Superseded by [2026-03-23-1000-shell-parser-port.md](2026-03-23-1000-shell-parser-port.md)

## Context

Shell コマンドのガードポリシー（git 直接操作のブロック等）をどの層に置くか。domain 層に pure computation として配置するか、trait で抽象化するか。

## Decision

Domain 層に trait なしの pure computation として配置。I/O なし、実装バリエーションなし。

## Rejected Alternatives

- tree-sitter-bash (C dep): C 依存が workspace の純粋性を損なう
- domain trait (over-engineering): 実装が 1 つしかないのに trait を作るのは過剰

## Consequences

- Good: domain 層で完結する pure computation
- Bad: conch-parser が domain の依存に入り、hexagonal violation（→ INF-20 で解消）

## Reassess When

- Superseded: INF-20 (PR #54) で ShellParser port trait + ConchShellParser adapter パターンに移行
