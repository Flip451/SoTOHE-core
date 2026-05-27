<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 50, yellow: 0, red: 0 }
---

# track-id 引数を省略可能にし、省略時は現在ブランチに紐づくアクティブトラックを既定値とする

## Goal

- [GO-01] track を対象とする `sotp` サブコマンドの track-id 引数を必須から省略可能へ変更し、省略時は現在の git ブランチ（`track/<id>`）からアクティブトラックの id を自動的に解決して既定値とする。これにより、pre-commit フックや pre-review コマンドなど「現在ブランチ = 当該トラックブランチ」が自明な文脈で、呼び出し側が track-id を用意する必要をなくす [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D1]
- [GO-02] 省略時解決を単一の共有経路（usecase 操作）に集約し、git ブランチ読み取りをすべて infrastructure の `GitRepository` ポート経由に統一する。これにより、各コマンドの個別実装・直接シェルアウト・Makefile シェルボイラープレートを除去し、解決規則を Rust 側（`usecase::track_resolution`）に一本化する [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D2]
- [GO-03] 既定解決の適用範囲を「現在のトラックを操作するコマンド」に限定し、ブランチ移動系コマンド（`branch create` / `branch switch`）は明示 track-id 必須のままとする [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D3]

## Scope

### In Scope
- [IN-01] 位置引数 `track_id: String`（必須）として定義されている track 系サブコマンド（`transition` / `signals` / `type-signals` / `type-graph` / `baseline-graph` / `contract-map` / `catalogue-spec-signals` / `catalogue-impl-signals` / `spec-element-hash` / `baseline-capture` / `add-task` / `set-override` / `clear-override` / `next-task` / `task-counts` など）の引数を `Option<String>` へ変更する [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D1] [tasks: T004]
- [IN-02] フラグ `--track-id: String`（必須）として定義されている track 系サブコマンド（`lint` / `review codex-local` / `review claude-local` / `review check-approved` / `review results` など）の引数を `Option<String>` へ変更する [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D1] [tasks: T005]
- [IN-03] フラグ `--track: String`（必須）として定義されている `verify catalogue-spec-refs` の引数を `Option<String>` へ変更する [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D1] [tasks: T005]
- [IN-04] 「現在ブランチからアクティブトラックを解決する」操作を usecase 層の操作として公開する。usecase はブランチ読み取りをポート（branch reader port）として受け取り、cli は composition root として `SystemGitRepo` アダプタを wiring して渡す。これにより解決ロジックが git 実体なしで単体テスト可能になる [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D2] [tasks: T001, T002]
- [IN-05] 既存の `usecase::track_resolution::resolve_track_id_from_branch` を省略時解決の中核ロジックとして再利用する。解決処理を新規実装せず重複させない [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D2] [tasks: T001]
- [IN-06] git ブランチ読み取りを infrastructure の `GitRepository` ポート経由に統一する。`track views sync`・`make` ヘルパー・write-guard 注入リーダーに残る `git rev-parse` の直接シェルアウトを取り除き、ポート経由の読み取りに置き換える [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D2] [tasks: T002, T003, T005, T006]
- [IN-07] active-track-write-guard（`2026-05-26-0518-active-track-write-guard.md`）が usecase interactor へのクロージャ注入で行うブランチ読み取りを、本 ADR の単一解決経路（ポート経由）に統合する。バリデーション（明示 id の照合）と既定値解決（省略時の導出）が同じ解決を共有し、git 読み取りは `GitRepository` ポート 1 本に集約する [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D2, knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T003]
- [IN-08] `Makefile.toml` の `*-local` タスク（`verify-plan-artifact-refs-local` / `verify-catalogue-spec-refs-local` / `check-catalogue-spec-signals-local` / `verify-spec-states-current-local` ほか）に散在する shell でのブランチ解析ボイラープレート（`BRANCH=$(git ...); TRACK_ID="${BRANCH#track/}"; TRACK_DIR="track/items/$TRACK_ID"` / `SPEC_PATH="track/items/$TRACK_ID/spec.md"` 相当）を除去する。track-id や pre-resolved spec path を渡さずに `cargo run -p cli -- verify catalogue-spec-refs` / `cargo run -p cli -- verify spec-states` のように呼べるようにする [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D2] [tasks: T006]
- [IN-09] 既に省略時自動解決に対応している `track resolve`（位置引数 `Option<String>`）・`track views sync`（`--track-id: Option<String>`）・`verify plan-artifact-refs`（`--track-dir: Option<PathBuf>`）の個別実装を、本 ADR の単一共有経路に統合し、重複を除去する [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D2] [tasks: T004, T005]
- [IN-10] 省略時解決のテストを追加する: `track/<id>` ブランチ上で省略した場合に正しい track-id が解決されるケース、`main` ブランチなどトラックブランチ以外で省略した場合に明示エラーで停止するケース、明示 track-id を渡した場合にそれが優先されるケース [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D1] [tasks: T001, T004, T007]
- [IN-11] `cargo make track-local-review` ラッパーを bare chain として実装する: `sotp track type-signals → sotp track catalogue-spec-signals → sotp track views sync → sotp review local` の順に各コマンドを呼び出し、ラッパー自身は track-id を保持しない。`BRANCH=$(git ...); TRACK_ID="${BRANCH#track/}"` のような shell でのブランチ解析をラッパー・Makefile 側に一切置かない [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D4, knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D1] [tasks: T008]
- [IN-12] `cargo make track-commit-message` の pre-commit ゲートを `cargo make track-local-review` と同じ bare chain 形式にする: `sotp track type-signals → sotp track catalogue-spec-signals → sotp track views sync` を chain した後にコミット処理を続け、review と commit の両ゲートで同じ手順（同じコマンド列）が使われるようにする [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D4, knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D1] [tasks: T008]
- [IN-13] 運用ドキュメント（`.claude/commands/track/*.md`、`track/workflow.md`、`DEVELOPER_AI_WORKFLOW.md`、`Makefile.toml` のタスク説明）から、コマンドが現在ブランチから自己解決するようになったことで不要になった明示的な `--track-id` 指定の記述を除去する。別トラックを明示的に対象にする意味を持つ箇所（例: 別トラックへの切り替えを説明する文脈）はオプションとして残す [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D1] [tasks: T009]

### Out of Scope
- [OS-01] ブランチ移動系コマンド（`track branch create` / `track branch switch`）への既定解決の適用: これらは「現在いないトラック」を対象に動くコマンドであり、現在ブランチからの導出は意味をなさないか誤りになるため、track-id は明示必須のままとする [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D3]
- [OS-02] CI 環境での detached HEAD やブランチ外バッチ処理への対応: ブランチに紐づかない文脈での fail-closed 以外の挙動（サイレントスキップや警告止まり）は本 track のスコープ外とする（ADR Reassess When に記載） [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D1]
- [OS-03] 複数トラックを同時に操作するユースケースへの対応: 「現在ブランチ = 単一トラック」という前提が崩れる複数トラック並行操作への対応は本 track のスコープ外とする（ADR Reassess When に記載） [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D1]
- [OS-04] rejected alternative B（shell 側にブランチ解決を置き続ける）の実装: Makefile タスクに shell ボイラープレートを追加・維持するアプローチは採用しない [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D2]
- [OS-05] rejected alternative C（環境変数 `SOTP_TRACK_ID` 等で track-id を引き回す）の実装: 環境変数ベースの設定管理は採用しない [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D1]
- [OS-06] rejected alternative D（各コマンドへの個別 auto-detect 実装追加）の踏襲: 既存の `resolve.rs` / `views.rs` 方式のように個別関数を都度追加するアプローチは採用しない [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D2]
- [OS-07] cli ヘルパーによる糊付け案（D2 内 rejected alternative）の採用: 解決を usecase 操作にせず cli 層の共有ヘルパー関数に集約するアプローチは原則採用しない。型設計フェーズで write-guard 側の既存抽象との整合確認後に最終判断する余地は ADR が残しているが、usecase 操作として公開する案が優先 [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D2]
- [OS-08] rejected alternative B（shell 側でブランチ解決しコマンドに渡す）をラッパーに採用すること: `cargo make track-local-review` や `cargo make track-commit-message` のラッパー内で `git rev-parse` + プレフィックス除去を行い、解決済み track-id をコマンドへ渡す形は採用しない（D4 の bare chain 方針と矛盾し、解決規則の二重化を再導入する） [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D4]

## Constraints
- [CN-01] track-id を省略しかつ現在ブランチがトラックブランチ（`track/<id>`）でない場合（`main` / detached HEAD など）は fail-closed とする: サイレントスキップや警告に留めず明示エラーで停止し、明示 track-id の指定を促す。ただし、トラック文脈を必須としないコマンド固有の no-track モードは文書化された例外として許容する。具体例として、`track views sync` は明示 `--track-id` がなく active track も解決できない場合に registry-only モードを維持できる [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D1] [tasks: T001, T004, T005, T006, T007]
- [CN-02] 明示 track-id は常に優先される: 引数で track-id を渡した場合はそれを使う。ブランチに紐づかない文脈や別トラックを明示的に対象にしたい場合のために、明示経路は維持する [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D1] [tasks: T004, T005]
- [CN-03] 省略時に導出した id は定義上現在ブランチと一致するため、`2026-05-26-0518-active-track-write-guard.md` のブランチ紐付きバリデーションを自動的に満たす。明示 id がブランチと一致しない書き込み系コマンドは write-guard が拒否し、その escape hatch（`--skip-branch-check`）は従来どおり機能する [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D1, knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T001, T003]
- [CN-04] 「現在ブランチからアクティブトラックを解決する」操作は usecase 層の操作として公開する。配置は hexagonal architecture と `2026-04-30-0848-cli-via-usecase-only.md` の cli-via-usecase 方針に従い、cli は composition root として wiring を行う薄い層に留まる [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D2, knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D1] [tasks: T001, T002]
- [CN-05] git の現在ブランチ読み取りはすべて infrastructure の `GitRepository` ポート（`current_branch()`）経由に統一する。usecase 操作内で git を直接呼び出さない（usecase 層の純粋性を維持する） [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D2] [tasks: T001, T002, T003]
- [CN-06] 解決規則は Rust 側（`usecase::track_resolution`）に一本化する。shell 文字列処理（`${BRANCH#track/}` 相当）を Rust 引数処理へ置き換えるという `sotp` 導入方針（`knowledge/conventions/shell-parsing.md`）に従い、shell 側に解決規則を重複させない [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D2] [conv: knowledge/conventions/shell-parsing.md#Single Parser Rule] [tasks: T006]
- [CN-07] 既定解決はブランチ移動系コマンド（`branch create` / `branch switch`）に適用しない。これらは track-id を明示必須のままとする [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D3] [tasks: T004, T005]
- [CN-08] 解決対象のブランチプレフィックスは `track/<id>` のみ。`plan/<id>` ブランチと lenient 解決関数は `2026-05-26-1123-remove-plan-only-activate-lane.md` の決定によりすでに削除されているため、strict / lenient の方針分岐を新たに設計しない [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D1, knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md#D1] [tasks: T001]
- [CN-09] bare chain の各ステップは fail-closed で連結する: chain の途中ステップが失敗した場合は後続ステップを実行しない。部分的に更新された状態でレビューやコミットを進め、後でハッシュずれが発覚する事態を防ぐ [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D4] [tasks: T008]
- [CN-10] ラッパー（`cargo make track-local-review` / `cargo make track-commit-message`）は track-id に関する知識を持たない。各 `sotp` サブコマンドが D1 の省略時解決を自分で行うため、ラッパーは track-id を shell で解析・保持・渡しをしない。各コマンドにオーケストレーション責務を追加することも行わない [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D4, knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D1] [tasks: T008]
- [CN-11] regen ゲート列（`sotp track type-signals` → `sotp track catalogue-spec-signals` → `sotp track views sync`）は単一の共有 `cargo make` タスクに一元定義し、`track-local-review` と `track-commit-message` の両ラッパーはその共有タスクを依存として呼び出す。同じ列を両ラッパーに重複定義しない。片方だけ変更されて pre-review と pre-commit が非対称になる回帰を構造的に防ぐ [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D4] [tasks: T008]
- [CN-12] `cargo make track-commit-message` の bare chain 化は、regen 系コマンドの呼び出し形（shell 解決から裸の自己解決コマンドへの変更）のみを変える。commit ゲートが持つ厳格な検査 — フル `cargo make ci`、および `check-approved`（レビュー未承認なら fail-closed で BLOCK）— は一切削除・弱体化しない。D4 は pre-review に regen ゲートを追加するものであり、commit ゲートを緩めるものではない [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D4] [tasks: T008]

## Acceptance Criteria
- [ ] [AC-01] `track/<id>` ブランチ上で track-id を省略した場合、`sotp track signals`（および IN-01〜IN-03 の対象コマンド群）が正常に動作し、現在ブランチの `<id>` が既定値として使われる [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D1] [tasks: T004, T007]
- [ ] [AC-02] `main` ブランチまたは detached HEAD の状態で track-targeted コマンドの track-id を省略した場合、明示的なエラー（`TrackResolutionError::NotTrackBranch` または `DetachedHead` 相当）でコマンドが停止し、明示 track-id の指定を促すメッセージが表示される。CN-01 で文書化した no-track モード例外（例: `track views sync` の registry-only モード）はこの fail-closed 対象から除外する [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D1] [tasks: T001, T004, T007]
- [ ] [AC-03] 明示 track-id を渡した場合は、現在ブランチに関わらずその id が使われる。例: `main` ブランチ上で `sotp track signals my-track` を実行した場合に `my-track` を対象とする [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D1] [tasks: T004, T005, T007]
- [ ] [AC-04] `cargo run -p cli -- verify catalogue-spec-refs` を track-id 引数なしで `track/<id>` ブランチ上から呼べる。従来 Makefile タスクで必要だった `TRACK_ID=${BRANCH#track/}` 相当の shell ボイラープレートなしで動作する [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D2] [tasks: T005, T006, T007]
- [ ] [AC-05] 省略時解決の git ブランチ読み取りは `GitRepository::current_branch()` 経由で行われる。`git rev-parse --abbrev-ref HEAD` の直接シェルアウトが usecase 操作内に残っていない [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D2] [tasks: T002, T003, T007]
- [ ] [AC-06] active-track-write-guard のブランチ読み取り（usecase interactor へのクロージャ注入）が `GitRepository` ポート経由に置き換えられており、write-guard の動作（現在ブランチに紐づかないトラックへの書き込みを拒否する）にリグレッションがない [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D2, knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T003, T007]
- [ ] [AC-07] `track branch create` と `track branch switch` は track-id を必須引数のままとする。引数なしで呼び出した場合、省略時解決ではなく clap のエラー（引数不足）として報告される [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D3] [tasks: T004, T005, T007]
- [ ] [AC-08] 省略時解決のユニットテストが存在する: (a) `track/<id>` ブランチで省略した場合に正しい id を返す、(b) `main` ブランチで省略した場合に `NotTrackBranch` エラーを返す、(c) detached HEAD の場合に `DetachedHead` エラーを返す [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D1] [tasks: T001, T007]
- [ ] [AC-09] `cargo make ci`（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D2] [tasks: T007]
- [ ] [AC-10] Makefile の `*-local` タスクから `BRANCH=$(git ...); TRACK_ID="${BRANCH#track/}"` 相当の shell ボイラープレートが除去されている。`plan/*` の dead な case アームが残っていない [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D2] [tasks: T006, T007]
- [ ] [AC-11] `cargo make track-local-review` の実装において、ラッパー自身が track-id を shell で解析・保持する処理が存在しない。`BRANCH=$(git ...); TRACK_ID="${BRANCH#track/}"` のような式が含まれていない [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D4] [tasks: T008, T007]
- [ ] [AC-12] `cargo make track-local-review` と `cargo make track-commit-message` のシグナル再生成ステップが対称である: 両者ともに `sotp track type-signals` → `sotp track catalogue-spec-signals` → `sotp track views sync` の同じコマンド列を実行する。review が承認した時点のシグナル状態と commit ゲートが観測するシグナル状態が一致し、`*-types.md` シグナル列のハッシュずれによる `check-approved` ブロックが構造的に発生しない [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D4] [tasks: T008, T007]
- [ ] [AC-13] `cargo make track-local-review` の bare chain 中、いずれかのコマンドが失敗した場合に後続コマンドが実行されず、chain 全体がエラーで終了する [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D4] [tasks: T008, T007]
- [ ] [AC-14] `.claude/commands/track/*.md`、`track/workflow.md`、`DEVELOPER_AI_WORKFLOW.md`、および `Makefile.toml` のタスク説明において、D1 により自己解決が可能になったコマンドに対する明示的な `--track-id` 指定の例示・記述が除去されている。別トラックを対象にする場面など、明示指定が意味を持つ文脈にはオプション表記として残っている [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D1] [tasks: T009, T007]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/shell-parsing.md#Single Parser Rule
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/source-attribution.md#Source Tag Types
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/05-testing.md#Test Structure

## Signal Summary

### Stage 1: Spec Signals
🔵 50  🟡 0  🔴 0

