# Planner Output — scripts/ Python ヘルパー削減 フェーズ1

- Date: 2026-04-13T07:26:02Z (revised after dependency graph audit)
- Capability: planner (Claude Opus subagent) + reviewer feedback (Codex CLI)
- ADR: `knowledge/adr/2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md`
- Scope: Phase 1 only (track_branch_guard / track_markdown / track_registry / track_state_machine 系の Rust 移植 + test_check_layers 削除)

## Summary

フェーズ1の対象は「Rust 側に同等実装が既にある」Python コード群の撤去である。Python selftest で検証されている観点を Rust `#[test]` に移植した後、Python ファイルを削除する TDD 順序が鍵になる。

依存グラフ調査により判明した重要事実:
- `scripts/track_schema.py` は `scripts/track_resolution.py:16` と `scripts/external_guides.py:20` から `from track_schema import ...` (try/except 内のインデント import) で参照されているため、Phase 1 では削除不可。本 track では impl を残し、`test_track_schema.py` のみ削除する
- `scripts/track_resolution.py` は `scripts/external_guides.py:19` から `from track_resolution import latest_legacy_track_dir` で参照されているため、Phase 1 では削除不可
- `libs/domain/src/track.rs` の `validate_plan_invariants()` 関数で **unreferenced task / cross-section duplicate 拒否は既に実装済み**。`codec::decode()` 経由で `validate_track_document()` に伝播する。これらは新規ロジック追加ではなく、テストカバレッジ追加のみ
- `scripts/test_track_state_machine.py` の 5 つの CLI level regression test (`test_transition_subcommand_*`, `test_sync_views_subcommand`) は Rust 側に同等カバレッジが無い。`apps/cli/tests/transition_integration.rs` を新規作成して移植する

## Step 1: Python selftest 観点抽出

### test_track_branch_guard.py (8 tests, うち 5 観点を Rust に移植)

**Rust に移植する 5 観点:**
- `test_matching_branch_passes` — 一致時に通過
- `test_mismatched_branch_raises` — 不一致でエラー
- `test_null_branch_skips_guard` — metadata.json の `branch: null` でスキップ
- `test_detached_head_raises` — `current_branch="HEAD"` でエラー
- `test_corrupt_metadata_raises` — 破損 JSON でエラー

**Rust では別レイヤでカバー (helper 移植対象外):**
- `test_none_current_branch_raises` — Rust の `current_git_branch() -> Result<String, String>` のエラー伝播でカバー
- `test_missing_metadata_skips` — Rust の `TrackReader::find()` が None を返すと `verify_branch_guard()` 自体がエラー (Python の skip 動作とセマンティクスが異なる現状を保つ)
- `test_skip_branch_check_bypasses` — `execute_transition()` の `if !skip_branch_check { ... }` で既実装

### test_track_schema.py (45 tests)

- `TestEffectiveTrackStatus` (10 tests): `TrackMetadata::status()` 導出ロジックの 10 観点 → Rust の `libs/domain/src/track.rs::tests` に移植 (T006)
- `TestValidateMetadataV2` (27 tests): validate_track_document の検証観点 → 既存実装テストとして 5 件追加 (T004)
- `TestReservedIdWords` (4 tests): `git` segment 拒否 + `legit-cleanup` substring 許可 → 既存 `validate_track_document_*` テストでカバー、substring false positive のみ追加
- `TestCommitHashRegex` (5 tests): 7/40 char hex / uppercase reject 等 → `domain::CommitHash` newtype 制約で構造的にカバー、必要なら `libs/domain/src/ids.rs::tests` に追加

### test_track_markdown.py (35 tests)

- `TestRenderPlan` (10 tests): render_plan の個別マーカー → Rust の `render.rs::tests` に 7 件追加 (T008)。決定論性テストは Rust では純関数で構造的保証のため不要
- `TestSummarizePlanNormalization` (20 tests): Python レガシー Markdown 双方向パース → Rust では一方向 render_plan のため陳腐化、移植対象外
- `TestSummarizePlanRejection` (7 tests): 同上、対象外
- `TestSummarizePlanEdgeCases` (11 tests): 同上、対象外
- `TestTrackDocsReviewerContract` (1 test): plan.md 内の文字列チェック → CI のファイル検索で代替可能、対象外

### test_track_registry.py (20 tests)

大半は既に `render_registry_*` / `sync_rendered_views_*` / `validate_track_snapshots_*` テスト群でカバー済み。未カバーの境界観点 3 件のみ移植 (T011):
- `track/items` 配下にファイルがある場合の空リスト
- 同時刻 updated_at の安定ソート (track_id tie-break)
- `sync_rendered_views()` が unchanged registry を changed set に含めない

### test_track_state_machine.py (30+ tests)

- 大半は `_transition_task_python()` 系 Python fallback (`now=datetime` 指定) のテスト → 本番パスは sotp 委譲済みのため移植対象外
- **CLI level regression 5 件** (これらは Rust に integration test として移植が必須):
  - `test_transition_subcommand_success`
  - `test_transition_subcommand_invalid_transition`
  - `test_transition_subcommand_missing_dir`
  - `test_transition_subcommand_with_commit_hash`
  - `test_sync_views_subcommand`

### test_check_layers.py (9 tests, dead code)

`import scripts.check_layers as check_layers` が即 ImportError。`check_layers.py` は既に `libs/infrastructure/src/verify/layers.rs` に移植・削除済み。同等の Rust テストが網羅されているため単純削除する。

## Step 2: Rust 側ギャップ分析

### test_track_branch_guard.py → `transition.rs::verify_branch_guard()`

`verify_branch_guard()` は `#[cfg(test)]` ブロックを持たない。**5 観点が移植必要** (T001/T002)。`MockTrackReader` を使ったユニットテストで、`verify_branch_guard_with_branch(reader, track_id, current_branch: &str)` という mock 化可能な内部関数を新設する。

### test_track_schema.py → domain/track.rs + render.rs::validate_track_document()

| Python 観点 | 状況 | カテゴリ |
|---|---|---|
| effective_track_status 系 10 tests | `TrackMetadata::status()` は実装済みだが domain tests なし | **移植必要** (domain/track.rs tests) |
| reserved ID "git" の拒否 | `validate_track_document_*` テストあり | **既にカバー済み** |
| reserved ID 'legit-cleanup' substring 許可 | explicit テストなし | **移植必要** (1 件追加) |
| commit_hash regex 5 tests | `domain::CommitHash` newtype 制約 | **構造的カバー** (必要なら 1-2 件追加) |
| non-string/None フィールドで TypeError/AttributeError | Rust では型システム構造的排除 | **テスト不要** |
| **unreferenced task 拒否** | **既に `libs/domain/src/track.rs::validate_plan_invariants()` で実装、`codec::decode()` 経由で `validate_track_document()` に伝播** | **既存実装、テストのみ移植 (T004)** |
| **cross-section duplicate (exactly-once) 拒否** | **同上 (`DuplicateTaskReference` variant)** | **既存実装、テストのみ移植 (T004)** |
| status drift 拒否 | `render.rs::validate_track_document()` で実装済み | **既存実装、テストのみ移植 (T004)** |
| archived + incomplete 拒否 | 同上 | **既存実装、テストのみ移植 (T004)** |
| all_track_directories 5 tests | `collect_track_snapshots` でカバー | **既にカバー済み** |
| v3 branch validation | `validate_track_document_*` 系でカバー | **既にカバー済み** |

**重要**: 当初の planner draft では「validate_track_document() に新規 logic 追加が必要」と書いていたが、これは誤り。`validate_plan_invariants()` 関数 (`libs/domain/src/track.rs` 内) で既に実装済みで、`codec::decode()` 経由で伝播する。テスト追加のみで観点をカバーできる。

### test_track_markdown.py → `render_plan()`

| Python 観点 | 状況 | カテゴリ |
|---|---|---|
| render_plan 基本 | `render_plan_matches_expected_layout` で 1 件カバー | **既にカバー済み** |
| 個別マーカー (todo / in_progress / done+hash / done-no-hash / skipped) | todo のみ網羅 | **移植必要 7 件 (T008)** |
| summarize_plan 系 35 tests | Rust 側に summarize_plan 相当が不存在 (一方向 render_plan) | **観点が陳腐化、テスト不要** |

### test_track_registry.py → `render_registry()` / `collect_track_snapshots()`

| Python 観点 | 状況 | カテゴリ |
|---|---|---|
| 各種 sync / render / validate 系 | 既存テスト群でカバー | **既にカバー済み** |
| items がファイルの場合 | explicit test なし | **移植必要 (T011)** |
| 同時刻 updated_at の安定ソート | tie-break 不安定の可能性 | **移植必要 + 実装修正の可能性 (T011/T012)** |
| sync_rendered_views unchanged registry skip | explicit test なし | **移植必要 (T011)** |

### test_track_state_machine.py → CLI level

- Python fallback 系: 本番パスは sotp 委譲済みのため移植不要
- **CLI level regression 5 件**: Rust 側に同等 integration test なし → `apps/cli/tests/transition_integration.rs` を新規作成して移植 (T014)

## Step 3: test_check_layers.py の扱い

`libs/infrastructure/src/verify/layers.rs` の `#[cfg(test)] mod tests` を確認した結果、同等の Rust テストが網羅されている。**単純削除で OK**。`scripts-selftest-local` 対象外であることは既に確認済み。

## Step 4: track_state_machine.py の扱い

### 選択肢 A: Python fallback コードのみ削除
`_transition_task_python()` / `_save_metadata()` / `_load_metadata()` などを削除し、`transition_task()` を sotp 失敗時に即 `TransitionError` に。`test_track_state_machine.py` の 30+ テスト (全て `now=datetime.now(UTC)` 指定で fallback を呼ぶ) が壊れるためファイル自体も大幅改修または削除が必要。

### 選択肢 B: ファイル全体削除 + CLI integration test 5 件を Rust に移植 (推奨・採用)

根拠:
1. 本番パスは全て sotp 委譲済み (`transition_task()` → `_try_sotp_transition()` / `sync_rendered_views()` → `_try_sotp_sync_views()`)
2. `test_track_state_machine.py` の Python fallback 依存テストの修正量は部分削除でも全体削除でも同じ
3. **CLI level regression 5 件** (`test_transition_subcommand_*`, `test_sync_views_subcommand`) は `apps/cli/tests/transition_integration.rs` (新規) に移植する。これにより CLI level coverage の喪失を防ぐ
4. `add_task()` / `next_open_task()` / `task_counts()` / `set_track_override()` 相当は `libs/usecase` / `libs/domain` + `apps/cli/src/commands/track/state_ops.rs` で既に Rust 実装あり

## Step 5: TDD タスク分解 (17 tasks)

- **T001 [Red]**: `verify_branch_guard_with_branch()` の 5 観点ユニットテストを `transition.rs::tests` に追加  
  レイヤー: CLI / 規模: small / 依存: なし
- **T002 [Green]**: `verify_branch_guard()` の内部を `verify_branch_guard_with_branch(reader, track_id, current_branch: &str)` に切り出し、T001 の 5 件 pass  
  レイヤー: CLI / 規模: small / 依存: T001
- **T003 [Cleanup]**: `track_branch_guard.py` + test 削除 + `scripts-selftest-local` 除外 (T002 後)  
  レイヤー: N/A / 規模: small / 依存: T002
- **T004 [Red]**: `render.rs::tests` に validate_track_document の 5 観点確認テスト追加 (UnreferencedTask 伝播 / DuplicateTaskReference 伝播 / status drift / archived+incomplete / 'legit-cleanup' substring)  
  レイヤー: infrastructure / 規模: small / 依存: なし
- **T005 [Verify]**: T004 のテストが既存実装 (validate_plan_invariants + validate_track_document) のみで pass することを確認。新規 logic 追加なし  
  レイヤー: infrastructure / 規模: small / 依存: T004
- **T006 [Red]**: `libs/domain/src/track.rs::tests` に `TrackMetadata::status()` 導出テスト 10 件追加  
  レイヤー: domain / 規模: small / 依存: なし
- **T007 [Cleanup]**: `test_track_schema.py` のみ削除 (impl は残す) + `scripts-selftest-local` 除外  
  レイヤー: N/A / 規模: small / 依存: T005, T006
- **T008 [Red]**: `render.rs::tests` に `render_plan` 詳細マーカーテスト 7 件追加  
  レイヤー: infrastructure / 規模: small / 依存: なし
- **T009 [Verify]**: `render_plan()` 既存実装で T008 pass 確認  
  レイヤー: infrastructure / 規模: small / 依存: T008
- **T010 [Cleanup]**: `track_markdown.py` + test 削除 + `scripts-selftest-local` 除外  
  レイヤー: N/A / 規模: small / 依存: T009
- **T011 [Red]**: `render.rs::tests` に render_registry / collect_track_snapshots / sync_rendered_views の境界テスト 3 件追加  
  レイヤー: infrastructure / 規模: small / 依存: なし
- **T012 [Green]**: 必要なら `collect_track_snapshots` に track_id tie-break 追加、T011 pass  
  レイヤー: infrastructure / 規模: small / 依存: T011
- **T013 [Cleanup]**: `track_registry.py` + test 削除 + `scripts-selftest-local` 除外 + `test_track_resolution.py:47` の smoke test から `scripts.track_registry` 除外  
  レイヤー: N/A / 規模: small / 依存: T012
- **T014 [Test]**: `apps/cli/tests/transition_integration.rs` 新規作成 + 5 つの CLI integration test 移植 (`assert_cmd` または `Command::cargo_bin('sotp')` 使用、tempdir で偽 track 作成して実 process spawn)  
  レイヤー: CLI / 規模: medium / リスク: integration test の環境セットアップ / 依存: なし
- **T015 [Cleanup]**: `track_state_machine.py` + test 削除 + `scripts-selftest-local` 除外  
  レイヤー: N/A / 規模: small / 依存: T014
- **T016 [Cleanup]**: `test_check_layers.py` 削除 (dead code)  
  レイヤー: N/A / 規模: trivial / 依存: なし (T001 と並列可)
- **T017 [Verify]**: `cargo make ci` + `cargo make scripts-selftest` + `scripts/` 残存 15 ファイルを確認  
  レイヤー: all / 規模: small / 依存: T003, T007, T010, T013, T015, T016

## Step 6: リスク評価

### R1. validate_track_document の後方互換性
新規 logic 追加なし (テスト追加のみ) のため後方互換性リスクは最小。ただし念のため既存 `track/items/*` の全 metadata.json が `validate_track_document()` を引き続き pass することを T005 で確認する。

### R2. atomic_write.py の削除順序
`track_registry.py` は `from atomic_write import atomic_write_file` (function 内 import) を含むが、フェーズ1で `track_registry.py` 全体を削除するため依存ごと消える。`atomic_write.py` 自体はフェーズ2で別途対処。問題なし。

### R3. track_state_machine.py 削除時の CLI coverage 喪失
選択肢 B (全体削除) を採用するが、`apps/cli/tests/transition_integration.rs` を T014 で新規作成して 5 件の CLI level regression を Rust に移植する。これにより CLI coverage は喪失しない。

### R4. フェーズ1 完了時点の scripts/ 残存整合

**削除後に残るべき (15 ファイル)**:
- `__init__.py`, `conftest.py`
- `architecture_rules.py`, `test_architecture_rules.py`
- `atomic_write.py`, `test_atomic_write.py`
- `convention_docs.py`, `test_convention_docs.py`
- `external_guides.py`, `test_external_guides.py`
- **`track_schema.py`** (Phase 3 で削除予定、external_guides/track_resolution が依存)
- **`track_resolution.py`**, `test_track_resolution.py` (Phase 3 で削除予定、external_guides が依存)
- `test_make_wrappers.py`, `test_verify_scripts.py`

**削除対象 (10 ファイル)**:
- `track_branch_guard.py`, `test_track_branch_guard.py`
- `test_track_schema.py` (impl は残す)
- `track_markdown.py`, `test_track_markdown.py`
- `track_registry.py`, `test_track_registry.py`
- `track_state_machine.py`, `test_track_state_machine.py`
- `test_check_layers.py`

ADR の「実装 4 + テスト 5 + dead code 1 = 10 ファイル」と一致。

### R5. test_track_resolution.py の smoke test 修正
`test_package_style_imports_work_from_repo_root` (line 47) が `import sys; sys.path = ['.', *sys.path]; import scripts.track_resolution, scripts.track_registry, scripts.external_guides` を含む。`track_registry.py` 削除前に `scripts.track_registry` を除外する必要あり。T013 で実施する。

### R6. 型設計パターン準拠
新規ドメイン型追加なし (既存の `TrackMetadata`, `TaskStatus`, `TrackStatus`, `CommitHash` を使用)。`verify_branch_guard()` の内部分離では戻り値型 `Result<(), String>` を継続使用 (CLI 境界表示用のため)。新 enum variant 不要。

## Step 7: 関連 Convention

| ファイル | 適用理由 |
|---|---|
| `knowledge/conventions/hexagonal-architecture.md` | 新規 Rust テストの配置判断 (`verify_branch_guard()` テストを CLI 層 vs infrastructure 層のどちらに置くか、port/adapter 境界を守る) |
| `knowledge/conventions/prefer-type-safe-abstractions.md` | 「TypeError/AttributeError にならないこと」を検証していた Python テストが Rust では型システムで構造的に不要である判断基準 |
| `knowledge/conventions/source-attribution.md` | spec.json の各観点に ADR source tag を付与するため |
| `knowledge/conventions/task-completion-flow.md` | `[Cleanup]` → `[Verify]` サイクルが `/track:commit` → `cargo make ci` と整合しているかの確認 |
| `knowledge/conventions/adr.md` | ADR 構造とフェーズ分割の判断基準 |

## Canonical Blocks

```rust
// ── apps/cli/src/commands/track/transition.rs への追加設計 (T002) ──
// verify_branch_guard() のテスト可能化のため内部関数を分離する設計方針:
//
// 現状:
//   pub(super) fn verify_branch_guard<R: TrackReader>(reader, track_id, repo_dir)
//   内部で current_git_branch(repo_dir) を直接呼ぶ
//
// 変更案:
//   pub(super) fn verify_branch_guard_with_branch<R: TrackReader>(
//       reader: &R,
//       track_id: &TrackId,
//       current_branch: &str,
//   ) -> Result<(), String>
//
//   pub(super) fn verify_branch_guard<R: TrackReader>(
//       reader: &R,
//       track_id: &TrackId,
//       repo_dir: &Path,
//   ) -> Result<(), String> {
//       let branch = current_git_branch(repo_dir)?;
//       verify_branch_guard_with_branch(reader, track_id, &branch)
//   }
//
// これにより #[cfg(test)] から verify_branch_guard_with_branch() を直接テスト可能。
// Python の none_current / missing_metadata_skips / skip_branch_check は Rust では
// 別レイヤで処理されるため helper レベルでは移植対象外 (5 観点のみ)。
```

```rust
// ── apps/cli/tests/transition_integration.rs (新規, T014) の構造例 ──
//
// use assert_cmd::Command;
// use tempfile::TempDir;
// use std::path::PathBuf;
//
// /// project-root 構造を作成し、track/items dir を返す。
// /// CLI の --items-dir は `<project-root>/track/items` の形でないと
// /// `apps/cli/src/commands/track/mod.rs:25-35` の arg-shape guard で reject される。
// fn setup_project_root(track_id: &str) -> (TempDir, PathBuf) {
//     let tmp = TempDir::new().unwrap();
//     let items_dir = tmp.path().join("track").join("items");
//     let track_dir = items_dir.join(track_id);
//     std::fs::create_dir_all(&track_dir).unwrap();
//     std::fs::write(
//         track_dir.join("metadata.json"),
//         /* 偽 metadata.json content */,
//     ).unwrap();
//     (tmp, items_dir)
// }
//
// #[test]
// fn test_transition_subcommand_success() {
//     let (_tmp, items_dir) = setup_project_root("demo");
//     Command::cargo_bin("sotp").unwrap()
//         .args(["track", "transition", "--items-dir", items_dir.to_str().unwrap(), "demo", "T001", "in_progress"])
//         .assert()
//         .success();
//     // metadata.json 更新を検証
// }
//
// #[test]
// fn test_transition_subcommand_invalid_transition() { /* 不正遷移でエラー */ }
//
// #[test]
// fn test_transition_subcommand_missing_dir() {
//     // 存在するが空の track/items dir に存在しない track id を指定
//     // (または `/nonexistent/track/items` のような arg-shape guard を満たすが実在しないパス)
// }
//
// #[test]
// fn test_transition_subcommand_with_commit_hash() { /* --commit-hash persisted */ }
//
// #[test]
// fn test_sync_views_subcommand() {
//     let (tmp, _items_dir) = setup_project_root("demo");
//     // sync は --project-root を使う (--items-dir ではない)
//     Command::cargo_bin("sotp").unwrap()
//         .args(["track", "views", "sync", "--project-root", tmp.path().to_str().unwrap(), "--track-id", "demo"])
//         .assert()
//         .success();
//     // plan.md / registry.md render を検証
// }
```

```rust
// ── libs/infrastructure/src/track/render.rs::tests に追加が必要な代表的テスト例 (T008) ──

/// render_plan が in_progress マーカーを正しく出力すること
#[test]
fn render_plan_renders_in_progress_marker() {
    let json = sample_metadata_json(
        "track-a",
        "in_progress",
        "2026-03-13T01:00:00Z",
        r#"[{"id": "T001", "description": "wip task", "status": "in_progress"}]"#,
    );
    let (track, _) = codec::decode(&json).unwrap();
    let rendered = render_plan(&track);
    assert!(rendered.contains("- [~] wip task"));
}

/// render_plan が done + commit_hash なしで literal "None" を出力しないこと
#[test]
fn render_plan_renders_done_without_commit_hash_no_none_literal() {
    let json = sample_metadata_json(
        "track-a",
        "in_progress",
        "2026-03-13T01:00:00Z",
        r#"[{"id": "T001", "description": "done task", "status": "done"}]"#,
    );
    let (track, _) = codec::decode(&json).unwrap();
    let rendered = render_plan(&track);
    assert!(rendered.contains("- [x] done task"));
    assert!(!rendered.contains("None"));
}
```

## Critical Files for Implementation

- `/home/flip451/individual/t-rust/templates/SoTOHE-core/libs/infrastructure/src/track/render.rs`
- `/home/flip451/individual/t-rust/templates/SoTOHE-core/apps/cli/src/commands/track/transition.rs`
- `/home/flip451/individual/t-rust/templates/SoTOHE-core/apps/cli/tests/transition_integration.rs` (新規)
- `/home/flip451/individual/t-rust/templates/SoTOHE-core/libs/domain/src/track.rs`
- `/home/flip451/individual/t-rust/templates/SoTOHE-core/Makefile.toml`
