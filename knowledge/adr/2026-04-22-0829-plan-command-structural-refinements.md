# Phase command 共通構造 + subagent 内部 pipeline 決定

## Context

本 ADR は ADR `2026-04-19-1242-plan-artifact-workflow-restructure.md` §展開フェーズ 4-6.5 (T5 + T6 + T7 + T7.5) の実装 track である `track-plan-decomposition-2026-04-22` の bootstrap 中に明確化された実装レベルの詳細を formalize する子 ADR である。親 ADR は「基本方針 + roadmap」の性格を持ち、独立 phase command (`/track:init` / `/track:spec` / `/track:design` / `/track:impl-plan`) と薄い orchestrator (`/track:plan`) への分解の方向性を示したが、subagent の内部責務 / command 定義のスリム化 / subagent への Write + Bash 権限付与の具体度は実装段階に委ねられていた。

bootstrap 中に spec.json を author したところ、31 件の yellow signals が残った。これらは親 ADR §D3.1 の signal 評価ルール (`informal_grounds` 非空 → 🟡) から発火しており、内訳は以下の 6 軸に集約される:

1. **command 命名の具体化**: 親 ADR §D0.0 / §コマンド境界 は `/track:spec` / `/track:design` と記述。本 track では subagent 名 (`spec-designer` / `type-designer`) との 1:1 対応を明確化するため `/track:spec-design` / `/track:type-design` に命名変更する判断を下した
2. **`/track:init` の branch create 責務**: 親 ADR §D0.0 は「`/track:init` (新設)」と書いたが branch create の責務は明示されておらず、「init 段階で branch create する」というユーザー指示に基づいて責務を明確化した
3. **subagent への Write + Bash 権限付与**: 親 ADR §D4 は type-designer の write 権限を §展開フェーズ 5 で明示、spec-designer / impl-planner への write 権限は未言及。さらに subagent が内部で sotp CLI を呼び信号機評価まで一括担当する設計のため Bash 権限も必要
4. **4 phase command の共通構造パターン**: writer invocation + 結果受け取り + subagent 内部で pipeline 実行、という構造を命令文書レベルで統一
5. **`activate.rs` preflight 緩和**: `activation_requires_clean_worktree` が新規 track materialization / branch create でも clean worktree を要求していたため、「init で branch create する設計」が機能しない問題が bootstrap 中に露呈。resume flow のみに緩和する実装を先行実施
6. **D6.4 実装補完**: 親 ADR §D6.4 は「空カタログ許容」を宣言したが、`libs/domain/src/tddd/consistency.rs::check_type_signals` と `libs/usecase/src/merge_gate.rs` の test U5 が空カタログ拒否を維持しており、bootstrap 中に `cargo make ci` が fail する形で gap を発見した

これらは本来、親 ADR で定式化されるべき内容だが、roadmap の粒度を超える詳細であるため、独立した子 ADR として記録することで:

- `spec.json` の 31 yellow informal_grounds を本 ADR の `adr_refs[]` へ formal 昇格させ、merge gate の 🟡 状態を解消する
- 親 ADR 本体を肥大化させず、命名 / 責務 / 権限 / 構造 / 実装詳細の判断を集約して追跡可能にする
- 後続の `adr-editor` back-and-forth が影響する blast radius を小さく保つ

## Decision

### D1: Phase command 共通構造パターン

4 phase command (`/track:init` / `/track:spec-design` / `/track:type-design` / `/track:impl-plan`) は 2 つの視点から記述できる。

**command 定義本文に書かれるのは以下 2 要素のみ** (外部視点):

- (a) 対応する writer (subagent、または `/track:init` のみ main) を Agent tool / 直接処理経由で invoke
- (b) writer が返した結果 (信号機評価結果 / binary gate 結果) を受け取り、`/track:plan` state machine に surface

**writer が内部で実行する pipeline** (内部視点 — command 定義本文からは不可視):

1. **writer invocation 受理**: subagent の場合は Agent briefing の受理、main の場合は `/track:init` 本文の step 列挙
2. **対応 SSoT ファイルへの書き込み**: writer が内部で実行
3. **post-write 信号機評価 / binary 検証 CLI 呼び出し**: writer が内部で実行 (`sotp verify *` / `sotp track *` 等)

以下は command 定義の外 (subagent 内部または main 内部) に閉じ込める:

- schema 検証 / codec round-trip
- signal 合成アルゴリズム
- reference resolution
- coverage 判定
- canonical block 疑惑検出
- state machine 遷移判断 / back-and-forth loop 制御 / `max_retry` guard / 終端 ADR diff 判断 (これらは `/track:plan` state machine にのみ存在)

`/track:init` のみ writer = main のため command 定義本文が直接全 step を実行する (D1 の例外、D3 参照)。

### D2: Command naming 変更

subagent 名との 1:1 対応を明確化するため、以下の命名変更を確定:

| 旧名 (親 ADR §D0.0) | 新名 (本 ADR) | 対応 writer |
|---|---|---|
| `/track:spec` | `/track:spec-design` | spec-designer subagent |
| `/track:design` | `/track:type-design` | type-designer subagent |
| `/track:impl-plan` | `/track:impl-plan` (変更なし) | impl-planner subagent |
| `/track:init` | `/track:init` (変更なし) | main (subagent なし) |
| `/track:plan` | `/track:plan` (変更なし、orchestrator) | main (state machine) |

### D3: `/track:init` の branch create 責務

`/track:init` は writer = main であり、command 実行時に以下の step を順次実行する:

1. track directory 作成 + metadata.json (branch=null 暫定) write
2. `sotp track branch create '<track-id>'` で main から `track/<id>` branch を作成して switch
3. metadata.json update (branch=`track/<id>`)
4. ADR 存在 pre-check (`knowledge/adr/` に参照予定 ADR が存在するか確認、未整備なら停止してユーザーに整備を促す)
5. identity schema binary 検証 (`sotp verify track-metadata`)

親 ADR §D0.0 では `/track:init` の責務として「metadata.json (identity-only) 生成」が明示されていたが、branch create は未言及。本 ADR で責務を明確化する。

### D4: subagent 3 つへの Write + Bash (sotp CLI 専用) 権限付与

`spec-designer` / `type-designer` / `impl-planner` の 3 subagent の frontmatter `tools` に以下を追加する:

- **Write** — 対応 SSoT ファイルおよびその render 派生ファイルへの直接書き込み
  - spec-designer → `spec.json` + `spec.md` (render 結果)
  - type-designer → `<layer>-types.json` + `<layer>`-graph md / `contract-map.md` / `<layer>-type-signals.md` (render 結果)
  - impl-planner → `impl-plan.json` + `task-coverage.json` + `plan.md` (render 結果)
- **Bash (sotp CLI 専用)** — `sotp verify *` / `sotp track *` 系の信号機評価 / render CLI 呼び出し
  - `cat` / `grep` / `head` 等の禁止 Bash 操作は引き続き禁止

各 subagent は対応 SSoT ファイルおよびその派生 render ファイルのみを Write し、他 SSoT ファイルには触れない。

親 ADR §D4 / §展開フェーズ 5 は type-designer の write 権限のみを明示、spec-designer / impl-planner への write 権限はユーザー指示による拡大適用として本 ADR で記録する。

### D5: `activation_requires_clean_worktree` を resume flow のみに限定

`apps/cli/src/commands/track/activate.rs` の `activation_requires_clean_worktree` 関数を以下に緩和する:

- **変更前**: 新規 track (not `already_materialized`) でも clean worktree を要求
- **変更後**: `matches!(mode, BranchMode::Auto) && already_materialized && resume_allowed` のときのみ `true`

新規 track materialization および一般 branch create は dirty worktree を許容する。git の通常動作 (branch switch で untracked files を carry over) により安全。

この緩和により D3 の `/track:init` が metadata.json を dirty worktree に書いてから `sotp track branch create` を呼べるようになる。

### D6: ADR §D6.4 実装補完 — 空カタログ許容の `check_type_signals` / merge_gate test 追従

親 ADR §D6.4 は「空カタログ許容」を宣言したが、以下の実装箇所で拒否ロジックが残っていた:

- `libs/domain/src/tddd/consistency.rs::check_type_signals` — entries が空のとき `VerifyOutcome::from_findings(error("has no entries"))` を返していた
- `libs/usecase/src/merge_gate.rs::test_u5_spec_blue_dt_empty_entries_blocks` — 空カタログを BLOCKED として期待していた test

本 ADR で以下の実装補完を正式化する:

- `check_type_signals` は empty entries で `VerifyOutcome::pass()` を返す (drift 検出は reverse SoT Chain ③ rustdoc ↔ catalogue で継続)
- test U5 は `test_u5_spec_blue_dt_empty_entries_passes_per_adr_d64` に改名し、assertion を pass に反転
- TDDD-BUG-02 regression guard `test_check_type_signals_empty_entries_error_mentions_catalogue_file` を廃止 (catalogue_file parametrization は sibling `test_check_type_signals_yellow_error_mentions_catalogue_file` が継続カバー)

## Rejected Alternatives

### A. 既存 ADR 2026-04-19-1242 に Follow-up 節として追記する

親 ADR は既に 1127 行。6 軸の決定を追記すると 1400-1500 行規模に肥大化し、基本方針 + roadmap の性格が薄れ、読み順が崩れる。命名変更 / preflight 緩和 / D6.4 実装補完は roadmap の抽象度を超える詳細で、親 ADR に混在させると将来の adr-editor back-and-forth が影響する blast radius が大きくなる。

### B. 6 決定を複数小 ADR に分割する (命名 / init 責務 / subagent 権限 / 共通構造 / preflight / D6.4 ごとに 1 ADR)

6 決定は互いに関連する (命名 → subagent 責務 → 共通構造 → preflight 緩和 → D6.4 は一連のフロー)。分割すると ADR 間の cross-ref が増え、逆に追跡性が低下する。1 つの子 ADR にまとめることで spec.json の 31 informal_grounds を単一 `adr_refs[]` で formal ref 化できる。

### C. spec.json の informal_grounds をそのまま残し ADR 化しない

親 ADR §D3.1 は informal_grounds 非空を 🟡 signal として発火させ、merge gate でブロックする運用。yellow のまま残すと merge できず実装タスク着手が滞る。formal ref 化が必須。

### D. command naming を親 ADR のまま (`/track:spec` / `/track:design`) 維持する

親 ADR の命名は「spec」「design」という単語単独だが、subagent 名 (`spec-designer` / `type-designer`) との対応が不明瞭。phase / writer / command の 3 つが別々の命名で存在すると、レビュー時の mental mapping コストが増える。1:1 対応を確立する `-design` suffix 追加が自然。

### E. subagent を advisory (read-only) のまま維持し orchestrator が transcribe する

本 bootstrap の spec.json 転記で実際に発生した「orchestrator が大量のテキストを手で書き写す」作業は error-prone かつ context 消費が大きい。subagent に Write 権限を付与することで 1 file = 1 writer が subagent 層で完結し、transcribe 作業と drift risk を同時に消せる。親 ADR §D4 は type-designer のみ write 付与を明示しているが、ユーザー指示により spec-designer / impl-planner にも拡大。

### F. `activate.rs` preflight 緩和をせず、bootstrap を workaround で進める

bootstrap 時の回避策 (mv で artifacts 退避 → branch create → 戻す、または main 直接 commit) は「init で branch create する設計」が現実的に機能しないままの状態を残す。将来の `/track:init` 実装で同じ問題が再発するので、本 ADR 時点で preflight を修正する。

## Consequences

### Positive

- spec.json の 31 yellow informal_grounds が本 ADR を cite する formal `adr_refs[]` に昇格 → merge gate 🟡 解消
- 親 ADR (2026-04-19-1242) は基本方針 + roadmap の性格を維持、肥大化を回避
- subagent が内部で file write + render + 信号機評価を完結 → orchestrator context 圧迫を回避、再利用性向上
- command 定義が subagent invocation + 結果受け取りのみに縮小 (`/track:init` は main 直接実行) → レビュー性向上、blast radius 小
- `/track:init` で branch create することで planning-only lane (`plan/<id>`) と実装 lane (`track/<id>`) の設計差が縮み、workflow が simple に
- `activate.rs` preflight 緩和で新規 track init 時の dirty worktree が許容される → 「init で branch create」が現実的に機能する
- D6.4 実装 gap 解消で「Rust コード変更のない track」の空カタログが `cargo make ci` を通る

### Negative

- 親 ADR 2026-04-19-1242 との関係を常に意識する必要あり (cross-ref の運用負担)
- OS-08 (`execute_branch` / `execute_activate` 共有実装の責務分離) は本 ADR では解決せず、別 track に委ねる
- D6.4 実装補完 (D6) は本来親 ADR の scope だが本 ADR で代替的に記録するため、将来の ADR 読み解き時に混乱する可能性
- subagent tools に Bash を追加することで、hook / guardrail が意図せず subagent の sotp CLI 呼び出しを block する risk が新たに発生する

## Reassess When

- 親 ADR (2026-04-19-1242) が superseded / deprecated になった場合 — 本 ADR の前提が崩れるので再評価
- subagent が Write + Bash (sotp CLI) 以外のツール権限を必要とする機能拡張が発生した場合 — D4 を revisit
- OS-08 (`execute_branch` / `execute_activate` 責務分離) が別 track で完了した場合 — D5 の preflight 緩和の条件式を再整理する必要がある
- phase 数が 4 を超える / 減る形の workflow 再編が提案された場合 — D1 の共通構造パターンを revisit
- TDDD の空カタログ許容 (親 ADR §D6.4) の判定ロジックが変わった場合 — D6 の実装補完を revisit
- `/track:plan` state machine の back-and-forth loop を別形式 (例: 事後 adr-editor 編集を不要にする structural solution) に置き換える判断が出た場合 — D1 の state machine 境界を revisit

## Related

- `knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md` — 親 ADR (展開フェーズ 4-6.5 / §D0.0 / §D1.4 / §D3.1 / §D4 / §D6.4 を本 ADR が具体化)
- `knowledge/conventions/pre-track-adr-authoring.md` — ADR 作成 lifecycle
- `knowledge/conventions/workflow-ceremony-minimization.md` — post-hoc review 方式 / 人工状態の撤廃
- `.claude/rules/04-coding-principles.md` — enum-first / typestate / newtype
- `.claude/rules/08-orchestration.md` — capability routing
- `.claude/rules/11-subagent-model.md` — subagent model tier rule
- `track/items/track-plan-decomposition-2026-04-22/spec.json` — 本 ADR が formal ref 化対象とする spec (informal_grounds → adr_refs)
