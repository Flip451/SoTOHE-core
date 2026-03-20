# Domain Semantics Hardening: Type-safe States, Eliminate Stringly-typed Fields

## Goal

domain 層のコードから「型で表現できる不変条件を String や Option で暗黙的に表現している」パターンを排除し、コンパイラで不正状態を防ぐ。

## Scope

### Phase A: TaskStatus Split (WF-40)
- `TaskStatus::Done { commit_hash: Option<CommitHash> }` → `DonePending` / `DoneTraced`
- `TaskTransition::BackfillHash` 追加
- `resolve_transition()` のインターフェース変更: `TaskStatusKind` → `&TaskStatus` を受け取り、`DonePending`/`DoneTraced` を区別
- usecase / infrastructure / CLI の match arm 更新

### Phase B: Track Phase Resolution
- `resolve_phase` と `resolve_phase_from_record` の両方で `TrackStatus` enum を使用
- `resolve_phase` は既に `TrackMetadata` を受け取るが、内部で status を文字列比較している部分を `TrackStatus` enum match に統一
- `resolve_phase_from_record` は `&str` → `TrackStatus` に変更、silent fallback 削除（本番呼び出し元なし、テスト用途のみ）
- 本番呼び出し元: `apps/cli/src/commands/track/resolve.rs` と `libs/infrastructure/src/track/render.rs` が `resolve_phase(&TrackMetadata)` を使用
- `TrackPhaseInfo.next_command: String` → `NextCommand` enum
- infrastructure/CLI への propagation（`resolve.rs`, `render.rs`）

### Phase C: Review System ADTs
- `Option<CodeHash>` → `CodeHash::NotRecorded` 追加、`Option` 除去
  - シリアライゼーション: `NotRecorded` は現行動作を維持（JSON からフィールドを省略、null ではない）
- `ReviewGroupState { fast: Option, final_round: Option }` → `ReviewGroupProgress` ADT（FinalOnly variant で既存 final-only データとの後方互換を維持）
- `ReviewEscalationResolution` の String フィールド → `NonEmptyString`（構築時検証）
- infrastructure/codec + CLI への propagation（`review_from_document` / `review_to_document`、`apps/cli/src/commands/review.rs` の `ReviewEscalationResolution` 構築箇所）

### Phase D: Minor Cleanups
- `AutoPhaseError` の String フィールド → `AutoPhase` enum
- `StatusOverride` → struct + `StatusOverrideKind` enum + `NonEmptyString`
- Review group name `String` → `ReviewGroupName` newtype（`Vec<String>`, `HashMap<String, ...>`, `record_round` の `group: &str` パラメータ、`expected_groups: &[String]` すべて）
- infrastructure/codec + CLI への propagation

## Out of Scope

- JSON の外部表現変更（後方互換を維持、フィールド省略パターンは保持）
- review ワークフローのロジック変更（型のみリファクタ）
- 新しい CLI サブコマンドの追加

## Constraints

- 後方互換: metadata.json の既存データはそのまま読み込める
- TDD: テストを先に書く
- Phase A → B → C → D の順で実装（各 Phase は domain + infrastructure/CLI propagation を含み独立にコミット可能）

## Acceptance Criteria

1. Phase A: `track-transition` で DonePending → DoneTraced backfill が成功する
2. Phase B: `resolve_phase_from_record` が `TrackStatus` enum を受け取り、呼び出し元はパース境界で未知ステータスをランタイムエラーとして処理する（exhaustive match により新 variant 追加時はコンパイルエラーで検出）
3. Phase B: `/track:resolve` 出力と `track/registry.md` の next_command が `NextCommand` enum 経由で生成される
4. Phase C: `ReviewState.code_hash` が `Option` なしで 3 状態を表現し、JSON シリアライゼーションは `NotRecorded` 時にフィールド省略を維持
5. Phase C: 既存の final-only review group データが `FinalOnly` variant で正常にデシリアライズされる
6. Phase D: `AutoPhaseError` の `from`/`phase`/`to` フィールドがすべて `AutoPhase` 型になる
7. 全 Phase: `cargo make ci` が通る
8. 全 Phase: JSON round-trip テストが通る（既存 metadata.json と互換）
