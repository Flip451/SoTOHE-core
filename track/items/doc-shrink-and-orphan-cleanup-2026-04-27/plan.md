<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Tier 1/2 narrative の縮約 + orphan 監査残項目を 1 track にまとめる (parent ADR D6 #3 + #4 + #5 統合)

## Summary

本 track は parent ADR D6 #3+#4+#5 の統合実装として、Tier 1/2 narrative 4 ファイルの heavy shrink、orphan 残項目 (a)(c)(d) の点訂正、および D6 #4 の /track:type-design 命名修正を 1 track でまとめて実施する。
作業性質: Rust ソースコードへの変更ゼロ。対象は doc / config ファイルのみ (OS-05 / CN-05)。
タスク構成: S001 (T001/T002: 非 DESIGN.md の orphan 点訂正) → S002 (T003/T004/T005: 小規模 doc shrink) → S003 (T006/T007/T008: DESIGN.md heavy shrink を 3 分割) → S004 (T009: CI gate 確認)。
DESIGN.md shrink (~1073→~150 行、~923 行削除) は 500 行コミット制約に対応するため 3 タスクに分割する (T006: keep-zone rework + Canonical Blocks 削除 ~360 行差分、T007: Security Hardening 削除 ~443 行差分、T008: 残余セクション削除 ~201 行差分)。
IN-01 の orphan 修正 (D6 #5 (c)(d)) は DESIGN.md 内の削除対象セクションに内包されているため、T008 の最終セクション削除で自然に解消される (T006-T008 の順序制約)。

## Tasks (5/9 resolved)

### S001 — 非 DESIGN.md orphan 点訂正 (T001, T002)

> T001: `.claude/commands/track/activate.md` / `.claude/commands/track/plan-only.md` / `track/workflow.md` の schema_version 旧値 (3→5) を点訂正する (IN-06 / IN-07 / AC-07)。Tier 0 ファイルの修正は schema_version 値のみに限定する (CN-04)。
> T002: `DEVELOPER_AI_WORKFLOW.md` (3 箇所) と `knowledge/strategy/tddd-implementation-plan.md` (1 箇所) の `domain-types.json` 単数形を現行複数 layer 表記に修正する (IN-05 / IN-08 / AC-05 部分)。DEVELOPER_AI_WORKFLOW.md の全体サイズは維持する (OS-06)。
> 2 タスクをまとめて commit することで orphan 点訂正セットを 1 差分として管理できる。diff は合計数行 (~10 行以下) のため atomicity 制約に十分収まる。

- [x] **T001**: `schema_version` 旧値 (3→5) の point fix を 3 ファイルに適用する。(1) `.claude/commands/track/activate.md` 13 行目の `schema_version: 3` を `schema_version: 5` に書き換える (IN-06)。(2) `.claude/commands/track/plan-only.md` 17 行目の `schema_version: 3` を `schema_version: 5` に書き換える (IN-06)。(3) `track/workflow.md` 156 行目の `schema_version: 3` を `schema_version: 5` に書き換える (IN-07)。各変更は値の点訂正のみに限定し、Tier 0 ファイルの構造・他の内容には変更を加えない (CN-04)。 (`3ad771e`)
- [x] **T002**: `domain-types.json` 単数形を `DEVELOPER_AI_WORKFLOW.md` と `knowledge/strategy/tddd-implementation-plan.md` で現行正式表記に修正する。(1) `DEVELOPER_AI_WORKFLOW.md` の 95 / 209 / 243 行付近に存在する `domain-types.json` 単数形の表記を `<layer>-types.json` などの現行複数 layer を示す正式表記に書き換える (IN-05)。(2) `knowledge/strategy/tddd-implementation-plan.md` の 80 行付近の `domain-types.json` 単数形を現行正式表記に書き換える (IN-08)。`DEVELOPER_AI_WORKFLOW.md` の全体サイズは維持し (OS-06)、細部修正のみ行う。 (`0b82af7`)

### S002 — 小規模 doc shrink (T003, T004, T005)

> T003: `START_HERE_HUMAN.md` を ~71→~60 行 (≤80 行 Tier 1 制約) に縮約する。存在しないディレクトリ (`docs/` / `project-docs/` / `.claude/docs/`) への言及を削除する (IN-03 / AC-03)。
> T004: `LOCAL_DEVELOPMENT.md` を ~176→~90 行 (≤100 行制約) に縮約する。Git Notes 節を workflow.md リンクに置換し、vague な移行記述を ADR/track id 明示に変換する (IN-04 / AC-04)。
> T005: `README.md` を ~140→≤80 行 (Tier 1 size limit) に縮約する。capability table 削除 / tmp/ 参照削除 / /track:design→/track:type-design 修正を適用する (IN-02 / AC-02)。
> 各ファイルは独立しており依存関係がないため、T003/T004/T005 を別々の commit として順次適用できる。各 diff は 100 行以下のため atomicity 制約に収まる。

- [x] **T003**: `START_HERE_HUMAN.md` を ~71 行から ~60 行 (≤80 行制約) に縮約する (IN-03)。存在しない `docs/` / `project-docs/` / `.claude/docs/` ディレクトリへの言及を削除し、実在ディレクトリのみを列挙する。保持: 最短 onboarding / 責務境界 / 必須レビュー・承認ポイント / 安全運用ルール。縮約後の行数が ≤80 行 (Tier 1 size limit) を満たすことを確認する (AC-03 / CN-01)。 (`6e6c258`)
- [x] **T004**: `LOCAL_DEVELOPMENT.md` を ~176 行から ~90 行 (≤100 行制約) に縮約する (IN-04)。Git Notes 節の詳細記述を `track/workflow.md` への参照リンクのみに変更する。'Phase 5/6 で Rust へ移行済み' 等の vague な表記は該当 ADR / track id を明示した記述に置換するか削除する。保持: Host Requirements / compose セットアップ / tools-daemon / Useful Commands / Troubleshooting。`tmp/` への永続的参照が残っている場合は削除する (CN-03)。縮約後の行数が ≤100 行を満たすことを確認する (AC-04)。 (`e0b7cbc`)
- [x] **T005**: `README.md` を ~140 行から ≤80 行 (Tier 1 size limit) に縮約する (IN-02)。capability table を削除して `.harness/config/agent-profiles.json` へのリンクのみに置換する。`tmp/` への参照行をすべて削除する (CN-03)。`/track:design` の表記を `/track:type-design` に修正する (AC-02)。ロードマップは `knowledge/strategy/TODO-PLAN.md` リンクのみ残す。保持: Project pitch / SoT Chain 4 階層図 / 信号機評価説明 / クイックスタート (新コマンド体系)。縮約後の行数が ≤80 行を満たすことを確認する (AC-02)。 (`00236ec`)

### S003 — DESIGN.md heavy shrink — 3 分割 (T006, T007, T008)

> T006: keep-zone (L1-73) の rework と `## Canonical Blocks` セクション全削除 (~355 行削除)。rework では Module Structure から Key Types 列削除、Key Design Decisions を ADR 索引リンクに置換、Agent Roles を agent-profiles.json 参照に置換する (IN-01 / CN-02)。
> T007: `## Security Hardening: Rust Migration` セクション全削除 (~443 行削除)。完了済み Python→Rust migration record であり ADR D3 の削除対象 (IN-01)。
> T008: 残余削除対象セクション全削除 (`## Feature Branch Strategy` / `## Open Questions` / `## Changelog` / `## Auto Mode` / `## Domain Types Registry`、~201 行削除)。この削除により DESIGN.md 内の `.claude/docs/` 参照 (D6 #5 (d) 項 4 件、L874 / L1019 / L1023-1025) と `domain-types.json` 単数形 (D6 #5 (c) 項、L1037-1039) がすべて解消される (IN-01 / AC-05 / AC-06)。
> T006→T007→T008 の順序で適用する。各タスクは削除行上部から下部に向かって進むため、後続タスクは前のタスク完了後の行番号ではなくコンテンツで一致する (Edit tool による surgical edit)。T008 完了後に行数 ≤200 行を確認する (AC-01)。

- [~] **T006**: `knowledge/DESIGN.md` の keep-zone (L1-73) を rework し、続けて `## Canonical Blocks` セクション (L74-428、~355 行) を削除する (IN-01、第1分割)。rework 内容: (a) `## Module Structure` 表から `Key Types` 列を削除して層と責務のみの表に変換する。(b) `## Key Design Decisions` 表を `knowledge/adr/README.md` への索引リンク 1 行に置換し、個別 ADR 詳細の再掲を排除する (CN-02)。(c) `## Agent Roles` 表を `.harness/config/agent-profiles.json` への参照リンクに置換する (CN-02)。その後 `## Canonical Blocks` セクション全体 (見出しから次の L2 見出し直前まで) を削除する。本タスクの diff は ~360 行以内で 500 行制約に収まる。
- [ ] **T007**: `knowledge/DESIGN.md` の `## Security Hardening: Rust Migration` セクション (L429-871、~443 行) を削除する (IN-01、第2分割)。このセクションは完了済みの Python→Rust migration record であり ADR D3 の削除対象。見出し `## Security Hardening: Rust Migration` から次の L2 見出し `## Feature Branch Strategy` 直前まで全行を削除する。本タスクの diff は ~443 行で 500 行制約に収まる。
- [ ] **T008**: `knowledge/DESIGN.md` の残余セクション (L872-1073、~201 行) を削除する (IN-01、第3分割)。削除対象: `## Feature Branch Strategy` (L874 の `.claude/docs/` 参照含む、D6 #5 (d) 項 1/4 を包含) / `## Open Questions` / `## Changelog` / `## Auto Mode (MEMO-15 Design Spike)` (L1019-1025 の `.claude/docs/` 参照含む、D6 #5 (d) 項 2-4/4 を包含) / `## Domain Types Registry` (L1037-1039 の `domain-types.json` 単数形含む、D6 #5 (c) 項を包含)。これらのセクションを削除することで AC-05 / AC-06 の DESIGN.md 内残存参照もゼロになる。本タスクの diff は ~201 行で 500 行制約に収まる。T006-T008 完了後に `knowledge/DESIGN.md` の行数が ≤200 行 (目標 ~150 行) を満たすことを確認する (AC-01)。

### S004 — CI gate 確認 (T009)

> T001-T008 完了後に `cargo make ci` を実行して全 gate が pass することを確認する (AC-08 / CN-05)。
> Rust ソースコードへの変更がゼロであるため fmt-check / clippy / nextest / deny / check-layers は変更前後で差異がない想定。
> verify-* (verify-latest-track / verify-view-freshness / verify-plan-artifact-refs) がドキュメント修正により誤検出しないことを確認する。特に DESIGN.md / README.md 等への broken ref 検証が DESIGN.md 縮約後も通過することを確認する。

- [ ] **T009**: `cargo make ci` を実行して全 gate が pass することを確認する (AC-08)。Rust コードへの変更はないため fmt-check / clippy / nextest / deny / check-layers はそのまま通過する想定。verify-latest-track / verify-view-freshness / verify-plan-artifact-refs 等の verify-* がドキュメント修正により誤検出しないことを確認し、T001-T008 の全変更が clean であることを最終確認する。
