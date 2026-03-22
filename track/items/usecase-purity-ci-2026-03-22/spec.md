# spec: usecase-purity-ci (INF-15)

## Goal

usecase 層のヘキサゴナル純粋性を CI で自動検証する `sotp verify usecase-purity` サブコマンドを新設。

## Scope

### IN scope

1. `libs/infrastructure/src/verify/usecase_purity.rs` — スキャンロジック
2. `apps/cli/src/commands/verify.rs` — サブコマンド配線
3. `Makefile.toml` — `verify-usecase-purity` タスク + `ci-local` 統合

### OUT of scope

- usecase コードの修正（既に hexagonal 準拠済み）
- error 化（warning-only で開始、将来の lockdown トラックで error 化）

## Constraints

- `sotp verify module-size` / `domain-strings` と同じ `VerifyOutcome` + `Finding` パターンに従う
- `#[cfg(test)]` ブロック内はスキャン対象外
- warning-only（既存違反がある場合にも CI をブロックしない）

## Related Conventions (Required Reading)

- `project-docs/conventions/hexagonal-architecture.md`

## Acceptance Criteria

1. `bin/sotp verify usecase-purity` が禁止パターンを検出して warning を出力
2. クリーンな usecase コード（現在の状態）で warning ゼロ
3. `cargo make ci` が pass
4. テスト: 禁止パターンを含む入力 → warning 検出、クリーン入力 → pass
