---
adr_id: 2026-03-11-0080-guard-policy-ban-patterns
decisions:
  - id: 2026-03-11-0080-guard-policy-ban-patterns_grandfathered
    status: accepted
    grandfathered: true
---
# Guard policy: ban edge-case-producing patterns

## Status

Accepted

## Context

Shell コマンドガードで、バイパスベクトルとなるパターンをどう扱うか。個別に再帰パースするか、一括ブロックするか。

## Decision

テンプレートワークフローで不要なパターンを無条件ブロック:
1. `env` コマンド → 即ブロック
2. `$VAR`/`$(cmd)`/`` `cmd` `` を任意位置（argv + redirect texts + heredoc body）で → 即ブロック
3. `.exe` suffix → basename で strip
4. effective command が `git` でなく、かつ argv/redirect token に "git"（case-insensitive）を含む → ブロック

Rules (2) と (4) で per-tool nesting analysis を完全に排除。

## Rejected Alternatives

- Per-pattern recursive parsing and validation: 複雑（~200 行の per-tool option parsing）、エラーしやすい

## Consequences

- Good: バイパスベクトルを構造的に排除。ガードロジックがシンプル
- Bad: 正当な `$VAR` 使用もブロック（false positive）。テンプレートワークフロー内では問題にならない前提
- Bad: SEC-11（git 部分文字列過剰ブロック）として known issue

## Reassess When

- テンプレートワークフローで `$VAR` の正当な使用が必要になった場合
- SEC-11 の false positive がユーザー体験に影響する場合
