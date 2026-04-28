---
adr_id: 2026-03-23-1000-shell-parser-port
decisions:
  - id: 2026-03-23-1000-shell-parser-port_grandfathered
    status: accepted
    grandfathered: true
---
# INF-20: ShellParser port + ConchShellParser adapter

## Status

Accepted

## Context

conch-parser が domain 層に直接依存しており hexagonal violation。INF-19（domain-purity CI）で今後の I/O 混入を防止するゲートが確立されたため、パーサーを infrastructure に移動する。

## Decision

- domain に `ShellParser` port trait を定義（`parse(&str) -> Result<Vec<SimpleCommand>>`）
- infrastructure に `ConchShellParser` adapter を実装
- policy は `&[SimpleCommand]` を受け取る（parse/evaluate 分離）
- DI は `Arc<dyn ShellParser>` で CLI 層から注入

Supersedes: [2026-03-11-0060-shell-guard-in-domain.md](2026-03-11-0060-shell-guard-in-domain.md)

## Rejected Alternatives

- Keep conch-parser in domain: dependency direction violation が残る
- cfg feature flag: 依存を物理的にデカップルしない

## Consequences

- Good: domain 層から外部クレート依存を除去。hexagonal purity 達成
- Good: パーサー差し替え（tree-sitter-bash 等）が adapter 追加で可能に
- Bad: DI の wiring コストが増加（Arc<dyn ShellParser>）

## Reassess When

- 別のシェルパーサー（tree-sitter-bash, shlex 等）への移行を検討する場合
