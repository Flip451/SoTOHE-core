<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# feature バッチ消化への既定反転 — per-layer 並列レビューを始動させる

## Summary

4 つの変更面 (review-scope.json 天井設定, domain/infrastructure Rust 拡張, full-cycle.md バッチループ反転, adr2pr.md Constraint 3 反転) に加え、TDDD tooling unblock の横断的 prerequisite (#[doc(hidden)] 削除) を実装する。T001 prerequisite は前 commit `5c159af139735232dc538b54b8fd886c34702fc1` で完了済み。Batch A (T002-T004): 設定 JSON 拡張 + Rust 実装。Batch B (T005-T006): markdown コマンド書き換え。
Track-wide guardrail (T002〜T006 全タスク共通): このトラックでは新たな `#[doc(hidden)]` 属性を一切導入してはならない。T001 で削除した 5 件 (recording_agent.rs ×2、null_insert_proxy.rs ×3) は 'pub だが TDDD カタログ除外' 目的で付与されていたが、bin/sotp signal calc-impl-catalog の Phase 1.6 (DanglingId) を発火させ track-active-gate を阻んでいた。再導入すると同じ tooling 退行が再発する。

## Tasks (6/6 resolved)

### S1 — Prerequisite: TDDD tooling unblock

> TDDD `calc-impl-catalog` の Phase 1.6 DanglingId 退行を解消するための横断的 prerequisite。
> `libs/infrastructure/src/dry_check/recording_agent.rs` 2 箇所と `libs/infrastructure/src/semantic_dup/null_insert_proxy.rs` 3 箇所の `#[doc(hidden)]` を削除する。
> 前 commit `5c159af139735232dc538b54b8fd886c34702fc1` で実装済みとして記録する。
> 本変更は ADR D5 が宣言する 4 領域には属さない。

- [x] **T001**: Prerequisite (横断的 tooling unblock): `libs/infrastructure/src/dry_check/recording_agent.rs` の 2 箇所および `libs/infrastructure/src/semantic_dup/null_insert_proxy.rs` の 3 箇所から `#[doc(hidden)]` を削除する。これは `bin/sotp signal calc-impl-catalog` が Phase 1.6 DanglingId で落ちる pre-existing tooling 退行への unblock 修正であり、本 ADR の変更面 (D5 宣言の 4 領域) には属さない横断的 prerequisite。前 commit `5c159af139735232dc538b54b8fd886c34702fc1` で実装済みであることを確認し、タスク完了として記録する。本タスクは spec 要素への直接紐付けを持たない (task-coverage 上は go-grounding なし)。 (`5c159af139735232dc538b54b8fd886c34702fc1`)

### S2 — Batch A: 設定 JSON + Rust 拡張 (T002-T004)

> `.harness/config/review-scope.json` にグローバル diff 天井 (`default_diff_ceiling_lines`) と per-group diff 天井 (`diff_ceiling_lines`) フィールドを追加する (T002)。
> domain `ReviewScopeConfig::new` の `entries` tuple に ceiling 要素を追加し、`default_diff_ceiling` パラメータを追加する。`diff_ceiling_for_scope` メソッドを実装する (T003)。
> infrastructure `load_v2_scope_config` の JSON DTO に ceiling フィールドを追加し、`ReviewScopeConfig::new` へ引数を追加する (T004)。
> T002-T004 は同一 commit (Batch A) とし、loader と domain 型の signature 不整合がコミット間に存在しないようにする。

- [x] **T002**: `.harness/config/review-scope.json` に per-scope diff 天井設定フィールドを追加する。トップレベルに `"default_diff_ceiling_lines": 500` グローバル既定フィールドを追加し、各 group エントリに任意フィールド `"diff_ceiling_lines": <u32>` を追加できるよう JSON を拡張する。既存の group エントリ (domain, usecase, infrastructure, cli, cli_composition, cli_driver, plan-artifacts, harness-policy) は既定のまま `diff_ceiling_lines` を追加しない (グローバル既定を継承させる)。これにより AC-04 (コード変更なしに天井値を設定ファイル変更のみで調整可能) を充足する。T003/T004 と同一 commit (Batch A) にまとめる。[guardrail] 本タスクは JSON 設定のみの変更であり Rust コードを変更しない。Rust を変更する場合は新たな `#[doc(hidden)]` を一切導入してはならない (tooling 退行再発防止)。 (`31d7cbb2616cd75f2cd039e69564e175db74aab8`)
- [x] **T003**: domain crate `libs/domain/src/review_v2/scope_config.rs`: `ReviewScopeConfig::new` の `entries` パラメータ型を `Vec<(String, Vec<String>, Option<String>)>` から `Vec<(String, Vec<String>, Option<String>, Option<u32>)>` へ変更し (4th element = per-scope diff ceiling)、`default_diff_ceiling: Option<u32>` パラメータを末尾に追加する。`ScopeEntry` struct に `diff_ceiling: Option<u32>` フィールドを追加する。`diff_ceiling_for_scope(&self, scope: &ScopeName) -> Option<u32>` メソッドを実装する: configured review scope に per-scope override がある場合はそれを返し、なければ configured review scope に限って `default_diff_ceiling` を返し、どちらも None なら None (unconstrained) を返す。`ScopeName::Other` は configured review scope ではないためグローバル既定を継承せず常に None を返す。既存メソッド (`classify`, `get_scope_names`, `contains_scope`, `all_scope_names`, `briefing_file_for_scope`) の動作・シグネチャは変更しない。T004 と同一 commit (Batch A) にまとめる。単体テストを追加: (a) per-scope override が存在するとき override 値が返る, (b) override なしでグローバル既定が返る, (c) どちらも None のとき None が返る, (d) `ScopeName::Other` は常に None (既存 Other スコープに ceiling 設定不可)。[guardrail] 新たな `#[doc(hidden)]` 属性を一切追加してはならない。`pub` メンバを TDDD カタログから除外したい場合は `#[doc(hidden)]` を使わず、カタログ除外の正しい経路 (informal_grounds など) を使うこと。 (`31d7cbb2616cd75f2cd039e69564e175db74aab8`)
- [x] **T004**: infrastructure crate `libs/infrastructure/src/review_v2/scope_config_loader.rs`: `GroupEntry` serde struct に `#[serde(default)] diff_ceiling_lines: Option<u32>` フィールドを追加する。`ReviewScopeJsonV2` serde struct に `#[serde(default)] default_diff_ceiling_lines: Option<u32>` フィールドを追加する。`load_v2_scope_config` 内の `entries` 構築ロジックを更新し、各 entry の tuple に `entry.diff_ceiling_lines` を 4th element として含める。`ReviewScopeConfig::new` 呼び出しに `doc.default_diff_ceiling_lines` を `default_diff_ceiling` 引数として追加する。public 関数シグネチャ (`load_v2_scope_config`) は変更しない。既存テストを通過させ、新規テストを追加: (a) `diff_ceiling_lines` を持つ group エントリが正しく `diff_ceiling_for_scope` に反映される, (b) `default_diff_ceiling_lines` がトップレベルに設定されたとき override なし scope に既定値が返る, (c) `diff_ceiling_lines` のない既存 JSON は backward compatible (None)。T003 と同一 commit (Batch A) にまとめる。[guardrail] 新たな `#[doc(hidden)]` 属性を一切追加してはならない。 (`31d7cbb2616cd75f2cd039e69564e175db74aab8`)

### S3 — Batch B: markdown コマンド書き換え (T005-T006)

> `.claude/commands/track/full-cycle.md` のループ構造を feature バッチ消化モデルへ書き換える (T005)。
> per-scope 天井による分割ロジックと D4 hash 一括記録ステップを追加する (T005)。
> `.claude/commands/track/adr2pr.md` Constraint 3 をバッチ既定に反転する (T006)。
> T005-T006 は同一 commit (Batch B) とする。Rust 変更 (Batch A) から分離し、markdown のみの commit で差分を小さく保つ。

- [x] **T005**: `.claude/commands/track/full-cycle.md` のループ構造を feature バッチ消化モデルへ書き換える。現在の「各タスクについて implement → DFP → review → commit をループ」構造を「feature 内の全 todo/in_progress タスクを依存順に一括実装 → review → commit」へ反転する。具体的には: (1) 実装フェーズを per-task から batch (全タスクを依存順に `/track:implement` へ一括委譲) に変更する; (2) DFP フェーズをバッチ実装後に 1 回呼ぶ; (3) review フェーズをバッチ全体の差分に対して 1 回呼ぶ; (4) commit フェーズでバッチを 1 commit にまとめる; (5) D4 hash 記録ステップ (`bin/sotp track transition <id> done --commit-hash <hash>` をバッチ内の全タスクに適用) を commit 直後に追加する; (6) per-scope 天井による分割ロジック (バッチ sizing: 次タスクを追加したときいずれかのレイヤーの累積差分が `diff_ceiling_for_scope` の天井を超過する場合は現バッチを commit して新バッチを開始) を記述する。CN-01 の「別レイヤーのみのタスクが控えている場合は天井超過済みレイヤーの累積は増えないため継ぎ足しを続けてよい」という観点を明示する。`done` with non-null `commit_hash` / `skipped` タスクのスキップ条件は維持する。T006 と同一 commit (Batch B) にまとめる。[guardrail] 本タスクは markdown のみの変更であり Rust コードを変更しない。Rust コードを変更する場合は新たな `#[doc(hidden)]` を一切導入してはならない。 (`635f0b6fc33916e80537b62c56e5dfd3c1f4f9d5`)
- [x] **T006**: `.claude/commands/track/adr2pr.md` の Constraint 3 (line 41) を既定反転する。現行: 「CI-driven bundling allowed. `/track:full-cycle` is per-task by default; when tightly-coupled tasks cannot pass `cargo make ci` individually, bundling them into a single implement → review → commit cycle is permitted.」を「Batch-first execution. `/track:full-cycle` implements the full feature batch (all tasks in dependency order) before review and commit by default. Per-task commit split is the exception, triggered only when a layer's cumulative diff would exceed its per-scope ceiling (configured in `.harness/config/review-scope.json`).」相当の文言に書き換える。T005 と同一 commit (Batch B) にまとめる。[guardrail] 本タスクは markdown のみの変更であり Rust コードを変更しない。Rust コードを変更する場合は新たな `#[doc(hidden)]` を一切導入してはならない。 (`635f0b6fc33916e80537b62c56e5dfd3c1f4f9d5`)
