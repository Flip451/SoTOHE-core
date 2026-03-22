# spec: usecase-purity-error (INF-17)

## Goal

`sotp verify usecase-purity` を warning-only から error に昇格し、usecase 層の hexagonal violation を CI でブロックする。

## Scope

### IN scope

1. `libs/infrastructure/src/verify/usecase_purity.rs` の `Finding::warning` → `Finding::error`

### OUT of scope

- 禁止パターンの追加・変更
- usecase コードの修正（INF-16 で violation ゼロ達成済み）

## Constraints

- `sotp verify usecase-purity` が error ゼロで pass すること（INF-16 で violation 解消済み）
- `cargo make ci` が pass すること

## Related Conventions (Required Reading)

- `project-docs/conventions/hexagonal-architecture.md`

## Acceptance Criteria

1. `Finding::warning` が `Finding::error` に変更されていること
2. `bin/sotp verify usecase-purity` が error ゼロで pass
3. `cargo make ci` が pass
4. usecase 層に新たな禁止パターン使用を追加すると CI が fail すること（テストで確認）
