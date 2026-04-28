---
adr_id: 2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap
decisions:
  - id: 2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap_grandfathered
    status: accepted
    grandfathered: true
---
# scripts/ Python ヘルパーの段階的 Rust 移行ロードマップ

## Status

Proposed

## Context

このプロジェクトは Python スクリプト (`scripts/` 配下) から Rust CLI (`bin/sotp` / `apps/cli` / `libs/infrastructure`) への段階的移行を進めてきた。既に削除済みの `scripts/verify_*.py` 系は `sotp verify *` subcommand に完全置換され、ADR `2026-04-09-2323-python-hooks-removal.md` で `.claude/hooks/*.py` も全削除される予定になっている。

残るは `scripts/` 配下の helper 群である。2026-04-13 時点で、`scripts/` には 12 個の実装ファイルと 13 個のテストファイルが存在する。

### 2026-04-13 の実態調査結果

`scripts/` 配下の実装 12 ファイルについて「呼び出し元」と「同等の Rust 実装の有無」と「Python ファイル間の import 依存グラフ」を徹底調査したところ、以下が判明した。

#### 1. 「現役」と「必要」の区別がなされていない

`scripts-selftest-local` (`Makefile.toml:109-128`) の対象は 11 個のテストファイルであり、表面的にはほぼ全ての実装ファイルが「現役」として扱われてきた。しかし内訳を追うと、実装ファイル 12 個のうち **本番パスで動作している Python 固有ロジックを持つのは 2.5 ファイルのみ** であり、残りは Rust 側に同等実装があるか、Python fallback が selftest 経由でしか到達しない dead path になっている。

#### 2. Python 内部の依存グラフ

実際の `from track_* import` / `import scripts.*` を grep した結果、scripts/ 内部の依存は以下の通り (try/except 内のインデント import を含む):

```
track_schema           (leaf, 5 ファイルから依存される)
  ├── track_markdown
  ├── track_registry
  ├── track_state_machine
  ├── track_resolution
  └── external_guides

track_branch_guard ──── track_state_machine
atomic_write ──┬─ track_registry
               └─ external_guides
track_resolution ─┬─ external_guides
                  └─ track_state_machine (function-internal)
```

つまり、`track_schema.py` と `track_resolution.py` は Phase 3 対象の `external_guides.py` から依存されているため、`external_guides.py` が Rust 化されるまで削除できない。

#### 3. 実装ファイルの分類

| ファイル | パターン | sotp 対応 | 証拠 |
|---|---|---|---|
| `external_guides.py` | **A: 完全必要** | 対応 subcommand なし | `guides_codec.rs` は読み込みのみ。HTTP fetch / retry / `guides.json` の add/list/clean/fetch/setup / `derive_raw_url` は Rust 側に未実装 |
| `convention_docs.py` | **D: 一部必要** | `verify-index` は Rust 移行済み | `libs/infrastructure/src/verify/convention_docs.rs` は verify のみ。`add` (テンプレ生成) と `update-index` (README 書換) が Python 独自 |
| `architecture_rules.py` | **D: 一部必要** | `verify-sync` は Rust 移行済み | `libs/infrastructure/src/verify/architecture_rules.rs` に「Rust port of architecture_rules.verify_sync()」と明記。`workspace-tree` / `workspace-members` / `direct-checks` は Python 独自 |
| `atomic_write.py` | C: sotp wrapper | `sotp file write-atomic` | `atomic_write.py:60-73` で `subprocess.run([sotp, "file", "write-atomic", ...])` 呼び出し。pure-Python fallback は sotp バイナリ不在時のみ到達する dead path |
| `track_state_machine.py` | D: 本番は sotp 委譲 | `sotp track transition` / `sotp make track-transition` | 本番パス (`now=None`) は必ず `_try_sotp_transition()` → sotp 呼び出し。Python fallback `_transition_task_python()` は `now=datetime` 指定のテスト時のみ到達 |
| `track_schema.py` | B: 重複 (依存により Phase 1 では残す) | Rust domain 型が完全実装 | `TrackMetadata` / `TrackTask` / `PlanSection` / `validate_plan_invariants()` 等が Rust 側に存在。**ただし `track_resolution.py` と `external_guides.py` から import されているため Phase 1 では削除不可** |
| `track_markdown.py` | B: 重複 | `sotp track views sync` (render::render_plan) | `libs/infrastructure/src/track/render.rs` が plan.md renderer として完全実装済み |
| `track_registry.py` | B: 重複 | `sotp track views sync` (render::sync_rendered_views) | render.rs が完全カバー。`atomic_write.py` 経由で sotp を再呼び出しする二重委譲構造 |
| `track_branch_guard.py` | B: 重複 | `apps/cli/src/commands/track/transition.rs::verify_branch_guard()` | 本番では未使用。Python fallback 補助関数として残存 |
| `track_resolution.py` | B: 間接依存 (Phase 1 では残す) | `views.rs::detect_track_id_from_branch` 等 | ロジックは Rust 側に存在。**`external_guides.py` から `latest_legacy_track_dir()` を import されているため Phase 1 では削除不可** |
| `__init__.py` | Utility | — | パッケージマーカー |
| `conftest.py` | Utility | — | pytest の `sys.path` 設定 |

#### 4. Rust 側の既存検証ロジック

planning 段階で見落としていた重要な事実:
- `libs/domain/src/track.rs` の `validate_plan_invariants()` 関数で **unreferenced task 拒否** (`UnreferencedTask`) と **cross-section duplicate 拒否** (`DuplicateTaskReference`) が既に実装されている
- これらは `codec::decode()` 経由で `libs/infrastructure/src/track/render.rs::validate_track_document()` に伝播する
- つまり Phase 1 で「Rust 側に追加する」検証ロジックは無く、テストカバレッジを増やすだけ

### 問題点

#### P1. 二重実装の固定化

本番パスは全て sotp 委譲されているにもかかわらず、`scripts/` 配下に Python 実装本体相当のコードが残っている。スキーマ変更のたびに Rust と Python の両方を更新する必要があり、不整合リスクと認知負荷が蓄積する。

#### P2. 「テストのためだけに生きている本番未使用コード」のねじれ

`track_branch_guard` / `track_markdown` / `track_registry` は本番パスで動作していない。Python fallback は `now=datetime` を指定するテスト時のみ到達する。実質的に「selftest が import するためだけに残存」しており、保守コストが機能的価値を上回っている。

#### P3. 削除済み実装へのテスト残骸

`test_check_layers.py` は実行しても import エラーで失敗する状態で放置されている。selftest 対象からは除外済みだが、ファイル自体は残っており、将来の読み手を混乱させる。

#### P4. docker 内 python3 依存の継続

`scripts/` に Python 実装がある限り、tools コンテナに `python3` を同梱する必要がある。ADR `2026-04-09-2323-python-hooks-removal.md` で `.claude/hooks/*.py` を全削除しても、`scripts/` が存在する限り Python runtime 依存は消えない。

## Decision

`scripts/` 配下の Python ヘルパーを **3 フェーズに分けて段階的に削減** し、最終的に `scripts/` ディレクトリ自体を削除する。各フェーズは独立したトラックとして `/track:plan` → `/track:implement` のサイクルで実施する。

### フェーズ 1: track_branch_guard / markdown / registry / state_machine の Rust 移植 (低コスト)

**対象削除ファイル (実装 4 + テスト 5 + dead code 1 = 10 ファイル)**:

- `scripts/track_branch_guard.py` + `scripts/test_track_branch_guard.py`
- `scripts/track_markdown.py` + `scripts/test_track_markdown.py`
- `scripts/track_registry.py` + `scripts/test_track_registry.py`
- `scripts/track_state_machine.py` + `scripts/test_track_state_machine.py` (ファイル全体削除: 選択肢 B 採用)
- `scripts/test_track_schema.py` (impl `track_schema.py` は依存により残す)
- `scripts/test_check_layers.py` (dead code 整理)

**Phase 1 で残す (Phase 2/3 で削除予定)**:

- `scripts/track_schema.py` — `track_resolution.py` / `external_guides.py` から `from track_schema import ...` (try/except 内インデント import) で参照されているため Phase 1 では削除不可
- `scripts/track_resolution.py` + `scripts/test_track_resolution.py` — `external_guides.py` から `from track_resolution import latest_legacy_track_dir` で参照されているため Phase 1 では削除不可

**作業内容**:

1. 各 Python selftest が検証している観点を整理し、Rust の現状動作と意味的に対応する観点のみを移植する (Python ↔ Rust セマンティクス差は読み替えるか別レイヤでカバー)
2. 対応する Rust テスト (`libs/infrastructure/src/track/render.rs` の `#[cfg(test)]`、`libs/domain/src/track.rs::tests`、`apps/cli/src/commands/track/transition.rs::tests`) に追加
3. `scripts/test_track_state_machine.py` の 5 つの CLI level regression (`test_transition_subcommand_success` / `_invalid_transition` / `_missing_dir` / `_with_commit_hash` / `test_sync_views_subcommand`) を `apps/cli/tests/transition_integration.rs` (新規) に Rust integration test として移植する
4. `scripts/test_track_resolution.py:47` の `test_package_style_imports_work_from_repo_root` から `import scripts.track_registry` を除外する (track_registry.py 削除前の必須修正)
5. `scripts-selftest-local` の対象リストから 5 ファイル (`test_track_branch_guard.py` / `test_track_schema.py` / `test_track_markdown.py` / `test_track_registry.py` / `test_track_state_machine.py`) を除外
6. Python ファイル 10 個を削除

**削除順序 (依存的に安全)**:

1. `track_branch_guard.py` + `test_track_branch_guard.py` を削除し、同時に `test_track_state_machine.py` を `scripts-selftest-local` から除外する (track_state_machine.py が `from track_branch_guard import ...` で依存しているため、selftest が ImportError になるのを防ぐ)
2. `test_track_schema.py` を削除し `scripts-selftest-local` から除外
3. `test_track_resolution.py:47` の smoke test 修正 (`scripts.track_registry` import を除外)
4. `track_markdown.py` + `test_track_markdown.py` を削除し `scripts-selftest-local` から除外
5. `track_registry.py` + `test_track_registry.py` を削除し `scripts-selftest-local` から除外
6. `track_state_machine.py` + `test_track_state_machine.py` を削除 (selftest 除外は既に完了)

**リスク**: 低。本番パスは既に sotp に委譲されているため、削除しても機能的影響はない。Python テスト観点が Rust 側で完全にカバーされているか (CLI integration test 5 件含む) を移植時に確認するだけでよい。

### フェーズ 2: atomic_write + architecture_rules 部分移行 (中コスト)

**対象**:

- `scripts/atomic_write.py` + `scripts/test_atomic_write.py`
- `scripts/architecture_rules.py` の `workspace-tree` / `workspace-members` / `direct-checks` コマンド

**作業内容**:

1. `sotp workspace tree` / `sotp workspace members` / `sotp arch direct-checks` 相当の Rust subcommand を `apps/cli/src/commands/` に追加
2. `external_guides.py` と `track_registry.py` (フェーズ 1 で削除済み) の `from atomic_write import atomic_write_file` 呼び出しを `subprocess.run(["sotp", "file", "write-atomic", ...])` の直接呼び出しに置換
3. `atomic_write.py` と `test_atomic_write.py` を削除
4. `architecture_rules.py` を verify-sync 以外のコマンド実装が残らないように整理 (最終的にファイル削除は `external_guides.py` 等の他ファイルの依存解消後)

**リスク**: 中。`workspace-tree` 系は cargo workspace 解析が必要で、実装コストは非自明。ただし `cargo metadata` JSON 出力を使えば比較的容易。

### フェーズ 3: external_guides + convention_docs + track_resolution + track_schema 全移行 (高コスト)

**対象**:

- `scripts/external_guides.py` + `scripts/test_external_guides.py`
- `scripts/convention_docs.py` の `add` と `update-index`
- `scripts/track_resolution.py` + `scripts/test_track_resolution.py` (external_guides の Rust 化により依存が消える)
- `scripts/track_schema.py` (external_guides + track_resolution の Rust 化により依存が消える)
- `scripts/__init__.py` / `scripts/conftest.py`
- `scripts/test_verify_scripts.py` / `scripts/test_make_wrappers.py` (残存する構成ファイル検証テスト)
- `scripts/` ディレクトリ自体

**作業内容**:

1. `reqwest` / `ureq` 等の HTTP client を `apps/cli` に追加
2. `sotp guides add/list/fetch/clean/setup` subcommand を Rust 実装
3. `sotp conventions add/update-index` subcommand を Rust 実装
4. `Makefile.toml` の `guides-*` / `conventions-*` タスクを Rust subcommand 呼び出しに書き換え
5. `test_verify_scripts.py` と `test_make_wrappers.py` の構成ファイル検証ロジックを Rust 側の integration test に移植
6. `scripts/` ディレクトリを削除
7. tools コンテナの Dockerfile から `python3` / `pytest` / `ruff` を削除

**リスク**: 高。HTTP fetch 実装には依存追加とテスト整備が必要で、作業量が大きい。

### フェーズ 1 の先行実施理由

フェーズ 1 のみでも保守対象を 10 ファイル削減でき、かつ本番パスに影響しない。移行作業の中では「最も安全で効果が大きい部分」であり、先行して実施することで後続フェーズの実装時に track_branch_guard / markdown / registry / state_machine 系の二重実装を気にしなくてよくなる。

## Rejected Alternatives

### A. 現状維持

`scripts/` を放置する案。却下理由:

- 本番は sotp、テストは Python というねじれが固定化する
- スキーマ変更のたびに Rust/Python 両方の更新が必要になり、不整合リスクが蓄積する
- ADR `2026-04-09-2323-python-hooks-removal.md` で `.claude/hooks/*.py` を全削除しても、`scripts/` が残る限り tools コンテナの `python3` 依存は消えない

### B. 一気に全削除

全 Python ファイルを即削除する案。却下理由:

- `external_guides.py` の HTTP fetch / retry / `guides.json` 管理は Rust 側に同等実装がなく、削除すると機能損失が発生する
- `convention_docs.py` の `add` / `update-index` も Rust 側に未実装で、同様に機能損失になる
- `architecture_rules.py` の `workspace-tree` / `workspace-members` / `direct-checks` も未移行
- `track_schema.py` / `track_resolution.py` は Phase 3 対象ファイルから依存されており、依存元の Rust 化前に削除すると ImportError が発生する

### C. Python 側を主、sotp を副とする

Python 実装を正、sotp を wrapper とする案。却下理由:

- プロジェクトの移行方針 (sotp CLI への統一) と逆行する
- `sotp verify *` の移行成功実績と矛盾する
- 本番パスは既に sotp 側に寄っており、主副を逆転させるには逆向きの工数が必要

### D. `test_check_layers.py` だけ削除して他は放置

dead code 解消のみ行う最小対応案。却下理由:

- 短期的にはクリーンだが、二重実装という本質的問題が残る
- 「死んだファイルを整理する」という反応的な対応に留まり、移行ロードマップを示せない

### E. Python hook removal と本 ADR を 1 本化

ADR `2026-04-09-2323-python-hooks-removal.md` と本 ADR を統合する案。却下理由:

- スコープが大きく分かれる (`.claude/hooks/` は advisory 主体、`scripts/` は実装本体)
- 削除順序にも依存関係がある (hook 側が先、scripts 側が後)
- 別 ADR として段階的に実施する方が、実装トラックとの紐付けが明確になる

### F. Phase 1 で track_schema.py を削除する

`track_schema.py` を Phase 1 削除対象にする案。却下理由:

- `scripts/track_resolution.py:16` と `scripts/external_guides.py:20` が `from track_schema import ...` (try/except 内インデント import) で参照しており、削除すると Phase 2/3 対象ファイルが ImportError で動作不能になる
- これらの依存元を先に Rust 化する必要があり、Phase 3 まで持ち越すのが正しい順序

## Consequences

### Good

- **保守対象の段階的削減**: 実装ファイル 12 → フェーズ 1 後 8 → フェーズ 2 後 5 → フェーズ 3 後 0
- **本番/テストの実装一本化**: 本番パスが sotp、selftest が Python という二重実装のねじれが解消され、Rust テストに統合される
- **認知負荷の軽減**: 新規開発者が「なぜ Python と Rust の両方にほぼ同じロジックがあるのか」を悩む必要がなくなる
- **docker image 軽量化**: フェーズ 3 完了後、tools コンテナから `python3` / `pytest` / `ruff` を削除でき、image サイズと起動時間が改善する
- **CI 高速化**: `cargo make scripts-selftest` が段階的に縮小し、最終的には Rust test の中に吸収される
- **CLI integration test の確立**: フェーズ 1 で `apps/cli/tests/transition_integration.rs` を新設することで、CLI level の regression coverage が Rust 側に確立される
- **ADR `python-hooks-removal` の完結**: `.claude/hooks/` + `scripts/` の両方が削除されることで、プロジェクトから Python runtime 依存が完全に消える

### Bad

- **フェーズ 1 の移植コスト**: Python selftest を Rust `#[test]` に移植する一回限りの作業コスト。特に CLI integration test 5 件の新設は本 track のスコープを拡大する
- **フェーズ 3 の実装コスト**: HTTP fetch / JSON 管理 / テンプレ生成を Rust で書き直すコスト。`reqwest` 等の依存追加も必要
- **移行期間中の二重知識**: フェーズ 1 → 2 → 3 の途中では、Python と Rust の両方を理解する必要がある期間が続く (ただし現状も同じ)
- **リグレッションリスク**: テスト移植時に元の観点を取りこぼす可能性。フェーズごとに慎重なレビューが必要
- **track_schema.py / track_resolution.py の延命**: フェーズ 1 終了時点でも 2 ファイルは残存し、実質的な削除はフェーズ 3 で external_guides が Rust 化されるまで待つ必要がある

## Reassess When

- `bin/sotp` のビルドが恒常的に不可能になり、Python fallback が再び本番パスで必要になった場合
- フェーズ 1/2/3 の実施途中で `sotp` CLI の設計思想が変わり、subcommand 構成が大幅に再編される場合
- Python 側に新たな固有機能を追加する必要が生じた場合 (例: 複雑な外部 API との統合で Python エコシステムの方が成熟している場合)
- フェーズ 3 着手前に `external_guides.py` の HTTP fetch / retry 要件が大きく変わり、Rust 側で完全再実装するより Python を維持する方が合理的になった場合
- `test_verify_scripts.py` が検証している構成ファイル整合ロジックが Rust 側の `verify` 系 subcommand に吸収可能になった場合

## Related

- **ADR `2026-04-09-2323-python-hooks-removal.md`**: `.claude/hooks/*.py` の全削除。本 ADR はその続編として `scripts/` 側で同様の方針を段階的に適用する
- **2026-04-13 の scripts/ 使用状況調査 + 依存グラフ調査**: 本 ADR の Context セクションに反映された調査結果。最初の調査では try/except 内のインデント import を見落としていたため、reviewer 経由で track_schema/track_resolution の依存関係を確認した
- CLAUDE.md の「`sotp verify` subcommands (Rust CLI, replaces deleted `scripts/verify_*.py`)」記述 — 本 ADR が延長する移行方針の出発点
- `Makefile.toml:109-128` の `scripts-selftest-local` task — フェーズごとに対象リストが縮小する
