<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# review hash スコープ再設計 — review-scope manifest hash + 設定ファイル駆動ポリシー

review hash を index 全体の tree hash から review-scope manifest hash に置き換える。
track/review-scope.json で hash スコープポリシーを設定ファイル駆動にし、プロジェクト構造に依存しない汎用設計にする。
worktree から直接読むことで未ステージ問題を解消し、metadata.json.review サブツリーを normalize で除外することで並列レビュー干渉を解消する。
stored hash format を rvw1:sha256:<hex> に version 付けし、legacy / non-rvw1 hash は stale code hash として検出する。

## Review Scope Policy 設定

track/review-scope.json にスコープポリシーを定義。
review_operational, planning_only, other_track セクションで glob パターンを指定。
<track-id> と <other-track> はランタイム展開。
マッチしないパスはデフォルトで Implementation（hash に含む）。

- [x] track/review-scope.json スキーマ定義 + 初期設定ファイル作成（review_operational, planning_only, other_track, normalize セクション）
- [x] Infrastructure: ReviewScopePolicyConfig serde 型 + review-scope.json ローダー
- [x] Infrastructure: ReviewScopePolicy — config + track_id から構築、glob パターンによるパス分類 (classify/includes)
- [x] check_approved review-scope 整合: base_ref configurable, lazy hash, stale code hash invalidation 永続化
- [x] is_planning_only_path review-scope 整合: track_id パラメータ, review-operational 判定
- [x] CodeHash::computed rvw1 validation + computed_unchecked + NotRecorded bypass 修正
- [x] infrastructure hardening: discover_from, symlink, no-renames, rehash, canonicalize

## Review Scope Hash 計算

git diff (merge-base + staged + unstaged + untracked) でパス収集。
ReviewScopePolicy でパス分類、included のみ残す。
worktree からファイル内容を直接読み、metadata.json は volatile フィールド正規化。
ソート済み manifest を JSON → SHA-256 → rvw1:sha256:<hex>。

- [x] Infrastructure: collect_review_scope() — git diff (merge-base + staged + unstaged + untracked) からスコープ内パスを収集
- [x] Infrastructure: normalize_track_file_for_hash() — metadata.json の volatile フィールド正規化（設定ファイル駆動）
- [x] Infrastructure: build_review_scope_manifest() + hash_review_scope() — manifest 構築 → SHA-256 → rvw1:sha256:<hex> 形式

## Port / Adapter 更新

GitHasher trait を review_hash(ReviewHashInput) に更新。
SystemGitHasher に新 hash アルゴリズムを実装。
RecordRoundProtocolImpl を single-phase に簡素化。

- [x] Usecase: GitHasher trait を review_hash(ReviewHashInput) に更新 + normalized_hash 互換 shim
- [x] Infrastructure: SystemGitHasher に review_hash() を実装（collect → classify → read worktree → normalize → manifest → sha256）
- [x] Infrastructure: RecordRoundProtocolImpl を single-phase に簡素化（Pending 不要、hash 1 回計算のみ）

## Domain + Migration

新規記録では CodeHash::Pending を使わず、rvw1:sha256:<hex> の Computed hash を使う。
CodeHash::computed で rvw1:sha256: format を strict validation。computed_unchecked で codec 用。
legacy hash は check_commit_ready で StaleCodeHash として拒否。

- [x] Domain: 新規記録では CodeHash::Pending を使わず、CodeHash::computed rvw1 validation + computed_unchecked for codec を使う

## テスト + Legacy 削除

ReviewScopePolicy 分類テスト、review_hash contract テスト、legacy 検出テスト。
index_tree_hash_normalizing を production path から除去。

- [x] テスト: ReviewScopePolicy 分類テスト（TrackContent, Implementation, ReviewOperational, PlanningOnly, OtherTrack）
- [x] テスト: review_hash contract テスト — 未ステージ成功、review.json 変更で hash 不変、無関係ファイル変更で hash 不変、実装ファイル変更で hash 変化
- [x] テスト: legacy / non-rvw1 hash が stale code hash として拒否されることを検証
- [x] Legacy 経路整理: index_tree_hash_normalizing + record_round_with_pending を production review path から除去
- [-] review cycle の収束: metadata.json.review hash 除外の仕様変更（parallel auto-record 安全化）+ cross-group round alignment 制約撤廃 + verification.md / metadata 整合の最終更新。2026-03-29 に superseded by new review-state architecture として凍結し、残課題は後継トラックへ移管
