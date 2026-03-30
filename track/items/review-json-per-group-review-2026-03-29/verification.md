# Verification

- [ ] review.json schema_version 1 と codec が cycle/group-round/stale-reason を lossless に encode/decode できる
- [ ] policy source から mandatory `other` を含む frozen partition を導出できる
- [ ] review cycle bootstrap が base_ref / policy_hash / frozen group scopes / mandatory other を review.json に end-to-end で書き込める
- [ ] `record-round` が review.json に append-only で記録される
- [ ] `check-approved` が latest-success per-group hash 判定で fast/final を判定する
- [ ] group 間 round 一致を要求しない
- [ ] ある group が zero_findings を達成した後、別 group が retry しても当該 group の successful round が失効しないことを直接テストする
- [ ] policy_changed / partition_changed / hash_mismatch を区別して表示できる
- [ ] 新規 track では `metadata.json` に review state が保存されない
- [ ] `metadata.json` に review state が保存されない
- [ ] named group に属さないパスが mandatory `other` group に吸収されることを具体的に検証する
- [ ] あるパスが複数の named group の glob にマッチした場合に fail-closed でエラーとなることを検証する（non-overlapping partition の negative case）
- [ ] `check-approved` は final round が欠けた group がある場合に fail を返す
- [ ] stale cycle の successful rounds は新 cycle の approval 判定に参入しない
- [ ] review.json 未作成の新規トラックで review status が NotStarted を返し、check-approved が planning-only commit のみ許可する
- [ ] group partition 対象が review_operational / planning_only / other_track 除外後の実装差分のみであることを検証する
- [ ] review.json が review_operational 分類を維持し、append-only round 書き込みが自身の cycle を stale にしないことを回帰テストする
- [ ] `cargo make ci` が通る

## Notes

- 旧 tracks への後方互換・dual-read 対応は対象外（新規トラック専用）
- tamper-proof / provenance は `tamper-proof-review-2026-03-26` の責務
