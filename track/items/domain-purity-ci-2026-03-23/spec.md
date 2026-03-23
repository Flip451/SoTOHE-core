# spec: domain-purity-ci (INF-19)

## Goal

domain 層の I/O purity を CI で自動検証する `sotp verify domain-purity` サブコマンドを新設。

## Scope

### IN scope

1. `libs/infrastructure/src/verify/domain_purity.rs` — usecase_purity.rs を基に domain 向けに調整
2. `apps/cli/src/commands/verify.rs` — DomainPurity サブコマンド追加
3. `Makefile.toml` — `verify-domain-purity-local` タスク + `ci-local`/`ci-container` 統合

### OUT of scope

- `conch-parser` の移動（INF-20 で対応）
- usecase-purity の変更

## Constraints

- usecase-purity と同じ禁止パターン（std I/O 全面ブロック + 時刻依存 + 出力マクロ）
- I/O ゼロ確認済みのため `Finding::error`（即 CI ブロック）
- `conch-parser` の `use` は domain の `Cargo.toml` 依存であり、このlintのスコープ外（クレートレベルは `deny.toml` / `check-layers` で管理）

## Related Conventions (Required Reading)

- `project-docs/conventions/hexagonal-architecture.md`

## Acceptance Criteria

1. `bin/sotp verify domain-purity` が error ゼロで pass
2. `cargo make ci` が pass
3. domain 層に禁止パターン使用を追加すると CI が fail すること（テストで確認）
