<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# review hash スコープ再設計 — review-scope manifest hash + 設定ファイル駆動ポリシー

review hash を index 全体の tree hash から review-scope manifest hash に置き換える。
track/review-scope.json で hash スコープポリシーを設定ファイル駆動にし、プロジェクト構造に依存しない汎用設計にする。
worktree から直接読むことで未ステージ問題を解消し、review.json を scope 外にすることで並列レビュー干渉を解消する。
stored hash format を rvw1:sha256:<hex> に version 付けし、legacy hash は migration error として検出する。

## Review Scope Policy 設定

track/review-scope.json にスコープポリシーを定義。
review_operational, planning_only, other_track セクションで glob パターンを指定。
<track-id> と <other-track> はランタイム展開。
マッチしないパスはデフォルトで Implementation（hash に含む）。

- [ ] track/review-scope.json スキーマ定義 + 初期設定ファイル作成（review_operational, planning_only, other_track, normalize セクション）
- [ ] Infrastructure: ReviewScopePolicyConfig serde 型 + review-scope.json ローダー
- [ ] Infrastructure: ReviewScopePolicy — config + track_id から構築、glob パターンによるパス分類 (classify/includes)

## Review Scope Hash 計算

git diff (merge-base + staged + unstaged + untracked) でパス収集。
ReviewScopePolicy でパス分類、included のみ残す。
worktree からファイル内容を直接読み、metadata.json は volatile フィールド正規化。
ソート済み manifest を JSON → SHA-256 → rvw1:sha256:<hex>。

- [ ] Infrastructure: collect_review_scope() — git diff (merge-base + staged + unstaged + untracked) からスコープ内パスを収集
- [ ] Infrastructure: normalize_track_file_for_hash() — metadata.json の volatile フィールド正規化（設定ファイル駆動）
- [ ] Infrastructure: build_review_scope_manifest() + hash_review_scope() — manifest 構築 → SHA-256 → rvw1:sha256:<hex> 形式

## Port / Adapter 更新

GitHasher trait を review_hash(ReviewHashInput) に更新。
SystemGitHasher に新 hash アルゴリズムを実装。
RecordRoundProtocolImpl を single-phase に簡素化。

- [ ] Usecase: GitHasher trait を review_hash(ReviewHashInput) に更新 + normalized_hash 互換 shim
- [ ] Infrastructure: SystemGitHasher に review_hash() を実装（collect → classify → read worktree → normalize → manifest → sha256）
- [ ] Infrastructure: RecordRoundProtocolImpl を single-phase に簡素化（Pending 不要、hash 1 回計算のみ）

## Domain + Migration

CodeHash::Pending の使用停止。
StoredReviewHash (Legacy/ReviewScopeV1) で hash version を判別。
legacy hash は check_commit_ready で明確な migration error を返す。

- [ ] Domain: CodeHash::Pending の使用停止 + StoredReviewHash (Legacy/ReviewScopeV1) による legacy 判別

## テスト + Legacy 削除

ReviewScopePolicy 分類テスト、review_hash contract テスト、legacy 検出テスト。
index_tree_hash_normalizing を production path から除去。

- [ ] テスト: ReviewScopePolicy 分類テスト（TrackContent, Implementation, ReviewOperational, PlanningOnly, OtherTrack）
- [ ] テスト: review_hash contract テスト — 未ステージ成功、review.json 変更で hash 不変、無関係ファイル変更で hash 不変、実装ファイル変更で hash 変化
- [ ] テスト: legacy hash 検出 + migration error メッセージの検証
- [ ] Legacy 削除: index_tree_hash_normalizing を production path から除去 + record_round_with_pending の非推奨化
