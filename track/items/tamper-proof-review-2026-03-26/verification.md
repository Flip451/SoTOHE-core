# Verification: tamper-proof-review-2026-03-26

## Scope Verified

- [ ] VerdictProvenance ADT がドメイン層に追加済み
- [ ] ReviewRoundResult に provenance フィールドが追加済み
- [ ] review.json schema_version 2 に provenance ドキュメントが含まれる
- [ ] review-artifacts/ にアーティファクトが永続化される
- [ ] RecordRound CLI サブコマンドが削除済み
- [ ] BlockProtectedReviewStateWrite hook が有効
- [ ] check-approved --require-provenance が機能する
- [ ] ReviewError の provenance variants (InvalidProvenance, MissingEvidence, EvidenceDigestMismatch) が実装済みで、それぞれ対応するケースで返される

## Manual Verification Steps

1. `cargo make test` — 全テスト通過
2. `cargo make ci` — CI ゲート通過
3. `sotp review codex-local --auto-record` 実行後、review.json に provenance が記録されていることを確認
4. `sotp review codex-local --auto-record` 実行後、review-artifacts/<id>/ に session.log, final-message.json, attestation.json が存在することを確認
5. `sotp review check-approved --require-provenance` が provenance 付きラウンドで通過することを確認
6. provenance なしの既存 review.json（schema_version 1 互換）が `LegacyUnverified` として正しくデシリアライズされることをユニットテストで確認
7. `sotp review check-approved --require-provenance` が `LegacyUnverified` ラウンドでブロックし、`ReviewEvidenceStatus::LegacyUnverified` を返すことを確認
8. `sotp review check-approved --require-provenance` が改ざんされたアーティファクト（digest mismatch）でブロックし、`ReviewEvidenceStatus::DigestMismatch` を返すことを確認
9. `sotp review check-approved --require-provenance` がアーティファクト欠損（missing artifact）でブロックし、`ReviewEvidenceStatus::MissingArtifact` を返すことを確認
10. `sotp review check-approved --require-provenance` が review.json の verdict と attested payload の verdict が不一致の場合にブロックし、`ReviewEvidenceStatus::VerdictMismatch` を返すことを確認
11. Write/Edit ツールで review.json に書き込もうとすると hook でブロックされることを確認
12. Write/Edit ツールで review-artifacts/ 配下のファイルに書き込もうとすると hook でブロックされることを確認
13. `sotp review record-round` コマンドが存在しないことを確認

## Result / Open Issues

- 結果: 未実施
- オープン課題: なし

## Verified At

- 未検証
