<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# auto-record review.json 書き込みパス接続 + sandbox 修正

前トラック (review-json-per-group-review) で構築した review.json の domain model と infrastructure を実際の write path に接続する。
RecordRoundProtocolImpl が metadata.json ではなく review.json に書くよう修正し、check-approved も review.json から読むよう移行する。
併せて Codex CLI の --full-auto が --sandbox workspace-write を暗黙適用する問題を修正する。

## Sandbox 安全性修正

--full-auto が --sandbox workspace-write を暗黙適用するため、planner/reviewer が read-only sandbox を使えていなかった問題を修正する。

- [x] Fix --full-auto sandbox override: remove --full-auto from planner and reviewer Codex invocations (it implies --sandbox workspace-write, overriding read-only). Update tests to assert --full-auto is never present.

## Write path 接続

RecordRoundProtocolImpl を FsReviewJsonStore 経由で review.json に書くよう書き換える。
cycle auto-creation、group round append を実装する。review.json は review_operational ファイルとして cargo make add-all でコミット時に staging する。

- [x] Wire RecordRoundProtocolImpl to write review.json via FsReviewJsonStore: auto-create cycle if none exists, append group round, persist via save_review. Remove metadata.json review state writes. Add TDD tests.
- [x] Add --add flag to private_index stage_bytes for future use. review.json is staged by cargo make add-all at commit time (review_operational file). Clean up metadata.json review section remnants. Verify cargo make ci passes end-to-end.
- [x] Implement per-group scope hash: replace placeholder normalized_tree_hash with review-scope manifest hash computed from group frozen scope files per ADR section 5. Wire ReviewPartitionSnapshot to cycle creation for frozen scope and to_cycle_groups on GroupPartition.

## Read path 移行

check-approved を ReviewJsonReader 経由で review.json から読むよう移行する。

- [x] Migrate check-approved to read from review.json via ReviewJsonReader instead of metadata.json legacy ReviewState. Update tests.
- [x] Implement check-approved per-group scope hash verification: validate each group latest round hash against current group-scope hash computed from frozen scope files and base_ref.
