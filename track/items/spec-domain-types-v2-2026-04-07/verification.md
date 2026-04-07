# Verification: spec-domain-types-v2-2026-04-07

## Scope Verified

- [ ] DomainTypeKind 5 variant が正しく定義されている
- [ ] DomainTypeEntry が DomainStateEntry を完全に置換している
- [ ] domain-types.json (schema_version 1) のエンコード/デコードが正常動作
- [ ] spec.json から domain_states / domain_state_signals が完全除去されている
- [ ] 信号評価が Blue/Red 2値で動作 (Yellow 不使用)
- [ ] 既存 track の spec.json から domain-types.json がマイグレーション済み

## Manual Verification Steps

1. `cargo make ci` が通過すること
2. `cargo make export-schema -- --crate domain --pretty` の出力で新型が確認できること
3. `sotp track domain-type-signals <track-id>` が kind ごとの評価結果を出力すること
4. rendered domain-types.md の Domain Types テーブルが正しく表示されること
5. `approved` フィールドが domain-types.json に含まれ、デフォルト true でシリアライズされること

## Result / Open Issues

(未検証)

## Verified At

(未検証)
