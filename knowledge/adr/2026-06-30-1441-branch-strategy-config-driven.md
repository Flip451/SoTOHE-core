---
adr_id: 2026-06-30-1441-branch-strategy-config-driven
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session-01R5wwjh4iWiazyh5JkDrRTD:2026-06-30"
    candidate_selection: "from:[hardcode-as-is, env-var-preset, base-only-config, full-config-base-target-merge, preset-with-override] chose:full-config-base-target-merge"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session-01R5wwjh4iWiazyh5JkDrRTD:2026-06-30"
    candidate_selection: "from:[keep-fixed, configurable-prefix, no-prefix] chose:keep-fixed"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session-01R5wwjh4iWiazyh5JkDrRTD:2026-06-30"
    candidate_selection: "from:[introduce-now, defer-to-future-adr, never-introduce] chose:defer-to-future-adr"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:session-01R5wwjh4iWiazyh5JkDrRTD:2026-06-30"
    candidate_selection: "from:[global-config-only, snapshot-per-track, hybrid-with-override] chose:snapshot-per-track"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:session-01R5wwjh4iWiazyh5JkDrRTD:2026-06-30"
    candidate_selection: "from:[full-backward-compat, migration-script, fresh-tracks-only] chose:fresh-tracks-only"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:session-01R5wwjh4iWiazyh5JkDrRTD:2026-06-30"
    candidate_selection: "from:[edit-branch-strategy-only, edit-all-conventions, defer-other-conventions] chose:edit-all-conventions"
    status: proposed
  - id: D7
    user_decision_ref: "chat_segment:session-01R5wwjh4iWiazyh5JkDrRTD:2026-06-30"
    candidate_selection: "from:[hardcode-git-log-main, merge-target-via-port, ai-prompt-only-rewrite] chose:merge-target-via-port"
    status: proposed
  - id: D8
    user_decision_ref: "chat_segment:session-01R5wwjh4iWiazyh5JkDrRTD:2026-06-30"
    candidate_selection: "from:[stay-on-main, switch-base-only-keep-main-merge, switch-base-and-merge-target-to-develop] chose:switch-base-and-merge-target-to-develop"
    status: proposed
---
# Git ブランチ戦略を `.harness/config/branch-strategy.json` で設定駆動にする

## Context

SoTOHE は現状、Git ブランチ戦略が複数の層にハードコードされており、利用者が GitHub Flow 以外の戦略を選べない。

### 固定されている内容

2026-06-30 時点で `main` を単語マッチ (`grep -w`) で `.claude/`, `.harness/`, `.agents/`, `apps/`, `libs/`, `knowledge/`, `Makefile.toml`, `.github/`, `track/` 横断に検索した結果に基づく分類:

| カテゴリ | 概要 | 件数感 |
|---|---|---|
| 本番コード (`track/` プレフィックス) | `libs/domain/src/ids.rs` (`TrackBranch::try_new` 型不変条件) + `libs/domain/src/track.rs`, `libs/usecase/src/{track_resolution,git_workflow}.rs`, `apps/cli-composition/src/git.rs:125,278`, `apps/cli-composition/src/pr.rs`, `apps/cli-driver/src/track.rs:48` | ~10 箇所 |
| 本番コード (ベース `main`) | `apps/cli/src/commands/pr.rs:45` (`#[arg(long, default_value = "main")]`), `apps/cli/src/commands/track/branch_ops.rs:107,190-192,336,369`, `apps/cli-composition/src/track/mod.rs:294,311-313,326` | ~12 箇所 |
| 本番コード (baseline-capture の doc / help text) | `apps/cli/src/commands/track/mod.rs:281,299` (CLI `--source-workspace` の clap help, ユーザー `--help` 直視), `apps/cli/src/commands/track/mod.rs:368` (`CatalogueImplSignals` B「captured at pre-implementation main HEAD」), `libs/infrastructure/src/tddd/baseline_capture.rs:40,193` (関数 doc + テストコメント) | 5 箇所 |
| 本番コード (**`git rev-parse main` 差分基底 fallback**) | `apps/cli-composition/src/dry/shared.rs:91-115` (fallback 関数の正本), `apps/cli-composition/src/review_v2/shared.rs:301-303,332,444`, `apps/cli-composition/src/track/fixpoint_resolve.rs:57,452`, `libs/infrastructure/src/track/gate_state.rs:293-306,849`, `libs/infrastructure/src/dry_check/commit_hash_store.rs:55`, `apps/cli-composition/src/dry.rs:845-872` (テスト) | **5+ ファイル、ランタイム挙動。`.commit_hash` 不在時の base branch HEAD 解決** |
| Makefile 内部 gate task description | `Makefile.toml:328,343,355,371,383` (`signal calc-{spec-adr,impl-catalog,catalog-spec}-local`, `ci-track-local`, `gate-track` の description で「main / non-track branches」「PR CI, never push/main」 等) | 5 箇所、base branch context を `main` で代表表現 |
| README (利用者直視) | `README.md:109` (`/track:done                   # main に戻り完了サマリー`) | 1 箇所 |
| Cargo make + Claude settings | `Makefile.toml:575,616-619` (`track-switch-main`, `track-branch-create` 説明), `.claude/settings.json:159` (allowlist) | 5 箇所 |
| Workflow SSoT (`.harness/workflows/track/`) | `init.md` (6), `adr2pr.md:84,117`, `plan.md:33` | 9 箇所 |
| Claude commands / Codex skill | `.claude/commands/track/done.md` (全文の主旨), `.claude/commands/track/merge.md:45`, `.agents/skills/track-init/SKILL.md:3`, `.claude/rules/07-dev-environment.md:54,56` | 10 箇所 |
| **規約文書** | `knowledge/conventions/branch-strategy.md` (5、全文 main 前提), `task-completion-flow.md` (4), `adr.md:132-136` (`git log main` pre-merge 判定), `dry-check-workflow.md:143` (`git rev-parse main` フォールバック), `review-protocol.md:47` | **13 箇所** |
| **AI capability プロンプト** | `.harness/capabilities/adr-editor.md:58-60,120` (`git log main -- <adr-file>` で pre-merge/post-merge 判定) | 4 箇所 |
| GitHub Actions CI | `.github/workflows/ci.yml:5,7,26,106` (`branches: [main]` 等) | 4 箇所。SoTOHE-core 自身の trigger は D8 で `main` / `develop` 対応に更新し、利用者リポジトリ CI YAML の動的書き換えは Out of Scope |
| テスト fixture (`init -b main` 慣習) | `libs/infrastructure/src/{track,git_cli,verify,adr_decision,signal_layer_reader}` 系, `apps/cli/src/commands/{track,git,review,verify,dry}` 系, `apps/cli-composition/src/{dry,review_v2}` 系, `libs/domain/src/{git_ref,lib,guard/policy}.rs` | 40+ ファイル、100+ 箇所 |
| マージメソッド | `apps/cli/src/commands/pr.rs` 周辺で散在 | 既定値の根拠が複数箇所 |

特に前回見落としていた重要 surfaces: (a) **`branch-strategy.md` が全文 main 前提**で書かれている、(b) **`adr.md` の `git log main` pre-merge 判定**と **`dry-check-workflow.md` の `git rev-parse main` フォールバック**はランタイム挙動指示で config 駆動化が必要、(c) **`.harness/capabilities/adr-editor.md`** は AI プロンプトに `git log main` が埋め込まれている、(d) **`.github/workflows/ci.yml`** の `branches: [main]` は SoTOHE-core 自身の develop 採用に合わせて D8 で更新するが、利用者リポジトリ CI YAML の動的書き換えは利用者責務、(e) **テスト fixture 100+ 箇所**は test-local の "default branch" 慣習として `"main"` 維持可能。

### 業界の選択肢と SoTOHE 適合度

future-architect の Git ブランチ運用標準 (https://future-architect.github.io/arch-guidelines/documents/forGitBranch/git_branch_standards.html) を踏まえて、代表的な 5 戦略との適合度を整理する:

| 戦略 | base | PR target | 追加ブランチ種別 | 現状の SoTOHE で可能か |
|---|---|---|---|---|
| Trunk-Based | main | main | なし | ◎ |
| GitHub Flow | main | main | なし | ◎ |
| Lite GitLab Flow | develop | develop | hotfix | × (base 固定) |
| GitLab Flow | develop | develop | release, hotfix | × |
| Git Flow | develop | develop | release, hotfix | × |

### 採用判断の経緯

利用者が SoTOHE を社内プロジェクトに採用する際、既存の運用 (例: develop ブランチ運用) に合わせる必要がある場面が想定される。プレフィックス `track/` 自体は SoTOHE の "track" ドメイン語彙そのものであり戦略軸ではない (どの戦略を選んでも `track/` は SoTOHE が管理する空間として残せる) ため、戦略可変化の対象は base/target ブランチおよびマージメソッドに絞る。

track kind (feature / hotfix / release) 軸は、SoTOHE の中核前提である「1 track = 1 spec.json = 1 PR」を再設計する必要があるため、戦略可変化とは独立した将来の判断として切り出す。

### Related conventions

- `knowledge/conventions/no-backward-compat.md` — 後方互換性なしの判断と整合する
- `knowledge/conventions/hexagonal-architecture.md` — Port abstraction (BranchStrategyPort) の配置根拠

## Decision

### D1: base / target / merge_method を `.harness/config/branch-strategy.json` で外出しする

新規 config ファイル `.harness/config/branch-strategy.json` を導入し、以下 3 フィールドを定義する:

```json
{
  "base_branch": "main",
  "merge_target": "main",
  "merge_method": "squash"
}
```

- `base_branch`: track ブランチを切る分岐元
- `merge_target`: PR の base (通常 `base_branch` と同じだが GitLab Flow 系で develop→main を経由する場合に分離可能)
- `merge_method`: PR マージ時の既定メソッド (`squash` / `merge` / `rebase`)

usecase 層は `BranchStrategyPort` (新規 port) 経由で branch strategy を参照する。ただしこの port は「常にグローバル config を読む口」ではなく、操作フェーズごとの **effective strategy** を返す境界である。

解決規則:

- track 初期化時: `.harness/config/branch-strategy.json` を読み、`metadata.json#branch_strategy_snapshot` に焼き込むための default strategy として使う
- track 解決後の PR 作成・コミットゲート・ブランチ操作: `metadata.json#branch_strategy_snapshot` を effective strategy として使い、グローバル config は再参照しない
- track 外の bootstrap / diagnostics: track context が存在しない場合のみ `.harness/config/branch-strategy.json` の default strategy を使う

infrastructure 層には JSON config ローダ adapter と metadata snapshot reader を実装し、usecase 層には `BranchStrategy` value として渡す。

`BranchStrategy` value は以下を expose する:

- `base_branch() -> &str`
- `merge_target() -> &str`
- `merge_method() -> MergeMethod`
- `track_prefix() -> &str` (D2 で固定値 `"track/"` を返す実装、API として残すのは将来の柔軟性確保のため)

D6 で扱うランタイム git コマンドは用途ごとに参照先を分ける。`dry-check-workflow.md` の差分基底 fallback (`git rev-parse main`) は effective strategy の `base_branch()` を使う。一方、`adr.md` の pre-merge / post-merge 判定 (`git log main -- <adr-file>`) と D7 の AI capability プロンプトは、ADR が PR base に取り込まれたかを見る判定なので effective strategy の `merge_target()` を使う。

ハードコードを置換する範囲 (本番コード + delivery 層、テスト fixture と規約文書と AI プロンプトは D6/D7 で別途):

| 置換対象 | 置換方法 |
|---|---|
| `apps/cli/src/commands/pr.rs:45` | `default_value = "main"` を撤去し effective strategy から base 取得 |
| `apps/cli/src/commands/track/branch_ops.rs:107,190-192,336,369` | effective strategy から base 取得、起動条件 `current != "main"` を strategy 値と比較 |
| `apps/cli-composition/src/track/mod.rs:294,311-313,326` | 同上 (composition 側、`branch_ops.rs` と二重実装になっている部分を統合検討) |
| `apps/cli-composition/src/git.rs:125,278` | `"track/"` リテラルを `track_prefix()` 経由に (D2 で固定値返却) |
| `apps/cli-composition/src/pr.rs` | `merge_target` 既定値を effective strategy 経由 |
| `apps/cli-driver/src/track.rs:48` | doc comment 更新 (`from main` → `from configured base branch`) |
| `apps/cli/src/commands/track/mod.rs:281,299` (`bin/sotp track baseline-capture` の `--source-workspace` help text) | 「e.g. a git worktree at `main`」を「e.g. a git worktree at the configured base branch」に書き換え。**clap の `--help` 出力に現れるユーザー直視テキスト**なので config 駆動化必須 |
| `apps/cli/src/commands/track/mod.rs:368` (`CatalogueImplSignals` B docstring) | 「captured at pre-implementation main HEAD」→「captured at pre-implementation HEAD of the configured base branch」 |
| `libs/infrastructure/src/tddd/baseline_capture.rs:40,193` (関数 doc + テストコメント) | 同様の文言修正。runtime 挙動は config 非依存 (デフォルトで現 workspace から rustdoc 取得、`--source-workspace` 明示時のみ別 workspace)、書き換えは doc string のみ |
| **`apps/cli-composition/src/dry/shared.rs:91-115`** (`git rev-parse main` fallback 関数) | effective strategy から `base_branch()` を受け取り `format!("git rev-parse {}", strategy.base_branch())` 化。**正本実装なので最優先**。エラーメッセージも同様に更新 |
| `apps/cli-composition/src/review_v2/shared.rs:301-303,332,444` | review v2 の fallback も dry/shared.rs と同じ helper を共有するよう統合、または port 経由化 |
| `apps/cli-composition/src/track/fixpoint_resolve.rs:57,452` | 同上 (差分基底計算が gate_state を経由する場合は呼び出し元の更新で吸収される可能性) |
| `libs/infrastructure/src/track/gate_state.rs:293-306,849` | gate_state も同じ port を注入して fallback の base を引く |
| `libs/infrastructure/src/dry_check/commit_hash_store.rs:55` | doc 更新のみ (実体は cli_composition 層で fallback) |
| `apps/cli-composition/src/dry.rs:845-872` (テスト) | parametrize、fallback テストは port stub 経由で base を渡す |
| `Makefile.toml:328,343,355,371,383` (内部 gate task descriptions) | 「main / non-track branches」「push/main」を「base branch / non-track branches」「PR CI, never push to base branch」等に書き換え (runtime 挙動は branch shape チェックなので config 非依存、文言修正のみ) |
| `README.md:109` (利用者向けトップレベル README) | `# main に戻り完了サマリー` を `# 設定された base branch に戻り完了サマリー` に書き換え |
| `Makefile.toml:575,616-619` | `track-switch-main` → `track-switch-base` rename、本体は `bin/sotp track switch-base` (新規 native subcommand) に委譲 (cargo make 側にロジックを置かず Rust 側に集約する方針) |
| `.claude/settings.json:159` | `Bash(cargo make track-switch-main)` → `Bash(cargo make track-switch-base)` |
| `.harness/workflows/track/init.md` (6 箇所), `adr2pr.md:84,117`, `plan.md:33` | `main` 言及を「configured base branch (per `.harness/config/branch-strategy.json`)」に書き換え |
| `.claude/commands/track/done.md` (全文の主旨) | `track-switch-base` (新タスク名) を呼ぶよう書き換え、説明文も "switch to configured base" に |
| `.claude/commands/track/merge.md:45` | next action 推奨テキスト更新 |
| `.agents/skills/track-init/SKILL.md:3` | description テキスト更新 |
| `.claude/rules/07-dev-environment.md:54,56` | task 名 + 説明更新 |
| `libs/infrastructure/src/gh_cli.rs` PR test fixtures (`552-682` 周辺) | parametrize、base 引数化 (PR 作成テストは戦略依存なので必須) |
| その他テスト fixture (40+ ファイルの `init -b main` 系) | test-local 慣習として `"main"` 維持可。必要なら `init_repo_on_default_branch(strategy)` helper で隠蔽。一括変更は別 PR |

### D2: `track/` プレフィックスは固定したまま設定可能化しない

`TrackBranch::try_new` (`libs/domain/src/ids.rs:138-144`) の `"track/"` プレフィックス不変条件は維持する。`.harness/config/branch-strategy.json` にプレフィックスフィールドは追加しない。

理由:

- `track/` プレフィックスは SoTOHE の "track" ドメイン語彙そのものであり、`feature/` 等への変更要求は SoTOHE が「track = feature」だと誤解しているケースが多い
- 他戦略の `feature/` / `topic/` と SoTOHE の `track/` が同じリポジトリで共存する場合、プレフィックスが分離されている方が干渉が起きない
- 設定可能化すると `metadata.json` 経由の逆引き (branch → track id) のあいまい性が増す (空プレフィックスを許すと全ブランチが track 候補になる)

`TrackBranch` 型が domain 層に置かれている (delivery 概念が domain に染み出している) layer 問題は別軸の整理対象として切り出す (Out of Scope)。

### D3: track kind (feature / hotfix / release) は導入しない

`metadata.json` に `kind` フィールドを追加しない。hotfix / release ブランチを SoTOHE の first-class オブジェクトとして扱わない。

理由:

- kind 軸を入れると spec.json / 型カタログ / impl-plan の作成 ceremony が kind ごとに変わり、SoT Chain の signal 評価器も kind 軸を持つことになる
- 「1 track = 1 spec = 1 PR」前提が崩れる (Git Flow では release/hotfix は別 PR で main にも入る)
- 現時点で具体的な hotfix / release 運用の利用者要求が出ていない
- D1 の base/target 切り替えで Trunk-Based / GitHub Flow / Lite GitLab Flow / GitLab Flow の feature 開発フェーズはカバーできる

将来、具体的なユースケース (例: 緊急 hotfix を SoTOHE 内で扱いたい) が出てから別 ADR で再検討する。

### D4: `metadata.json` に `branch_strategy_snapshot` を持たせ、track 初期化時にその時点の strategy を凍結する

新規 track の `metadata.json` に以下のフィールドを追加する:

```json
{
  "branch_strategy_snapshot": {
    "base_branch": "develop",
    "merge_target": "develop",
    "merge_method": "squash"
  }
}
```

`/track:init` 実行時に `.harness/config/branch-strategy.json` の現在値を読み、snapshot として `metadata.json` に書き込む。以降の PR 作成・コミットゲート・ブランチ操作は snapshot を参照し、グローバル config を直接参照しない。

D1 の `BranchStrategyPort` はこの不変条件を守るための境界であり、track 解決後は snapshot を effective strategy として返す。実装者は PR 作成・コミットゲート・ブランチ操作で `.harness/config/branch-strategy.json` を直接読み直してはならない。

理由:

- グローバル config を後から変更しても in-flight track の PR base がずれない
- 過去 track の archaeology で「この track がどの戦略で作られたか」が `metadata.json` から自明
- snapshot は track 初期化時の 1 回書き込みのみで、以降不変

### D5: 後方互換性を持たない

既存 (本 ADR 実装前に作成された) track の `metadata.json` に対する migration 機構を提供しない。`branch_strategy_snapshot` 未持参の `metadata.json` は parse 失敗 (fail-closed) として扱う。既存 track の remediation 手順は SoTOHE 側で sanction しない (手動編集を推奨手順として文書化しない)。

Bootstrap exception:

- 本 ADR 実装 track 自身は D8 の transition track であり、fail-closed parser を有効化する前に、この track の `metadata.json` へ現在の戦略 (`main` / `main` / `squash`) を snapshot として焼き込む
- この 1 件は実装 PR 内の自己ブートストラップであり、既存 track 全般への migration script / remediation 手順は提供しない

理由:

- migration script のメンテナンス burden が新たに発生する
- 後方互換性なし原則 (`knowledge/conventions/no-backward-compat.md`) と整合する
- 「手動編集を運用で対応」と書いた瞬間にそれが事実上の移行手順となり、後方互換性なしの建前と矛盾する
- 既存 track の扱いは利用者判断に委ね、SoTOHE 側の責務範囲を明確にする

### D6: 規約文書 5 ファイルを config 駆動の記述に書き換える

`knowledge/conventions/` 配下の以下 5 ファイルを config 駆動の記述に書き換える:

- `branch-strategy.md`: 全文書き換え。base/target の具体値を出さず、`.harness/config/branch-strategy.json` 経由で利用者が選べることを記述する。先行 ADR (workflow.md 解体) で新設された convention で、本 ADR の主要なランディングゾーン
- `task-completion-flow.md`: 「マージ後の main 上で〜」を「マージ後の merge_target 上で〜」 (4 箇所)
- `adr.md:132-136`: pre-merge / post-merge 判定の `git log main -- <adr-file>` を `git log <merge_target> -- <adr-file>` に。`<merge_target>` は effective strategy の `merge_target()` 経由で解決
- `dry-check-workflow.md:143`: `git rev-parse main` フォールバックを `git rev-parse <base_branch>` に
- `review-protocol.md:47`: 「`git stash` + main でベースライン確認」を「`git stash` + base branch でベースライン確認」に

理由:

- `adr.md` と `dry-check-workflow.md` の git コマンド指示は **ランタイム挙動を規定**しているため、ここを直さないと default 戦略以外で破綻する。ADR lifecycle 判定は PR base への取り込みを見るため `merge_target()` を使い、差分基底 fallback は track 分岐元を見るため `base_branch()` を使う
- `branch-strategy.md` は ADR-A の延長で新設された convention で、現状 main 前提のまま書かれている
- 他 3 ファイルは説明文のテキスト更新で、規約の意図は変わらない

### D7: AI capability プロンプト内の `git log main` pre-merge 判定を `merge_target()` 経由にする

`.harness/capabilities/adr-editor.md:58-60,120` の `git log main -- <adr-file>` 4 箇所を、effective strategy の `merge_target()` 経由で解決した値を使うよう書き換える。

書き換え方針:

- 静的な `main` を埋め込む代わりに、プロンプト内で「the configured merge target (`branch_strategy_snapshot.merge_target` / `.harness/config/branch-strategy.json#merge_target`)」を参照させる
- adr-editor 起動時に effective strategy から merge target を解決する手順を briefing 側で明示し、adr-editor 本体は変数として受け取る
- pre-merge / post-merge 判定の意味は変えない (空かどうかでの判定のまま)

理由:

- AI capability プロンプトは Code ではないがランタイム挙動を規定するため、本番コードと同様に config 駆動化が必要
- pre-merge / post-merge 判定は ADR ライフサイクル (immutability) に直結する正当性条件で、戦略依存
- adr-editor は SoT 編集者なので、判定が誤れば「merge_target にマージ済みの ADR を pre-merge と誤認して直接書き換える」事故が起きうる

### D8: SoTOHE 本体は `develop` ブランチを採用し、`.harness/config/branch-strategy.json` を develop ベースでシップする

`main` から派生して `develop` ブランチを新規作成する。`.harness/config/branch-strategy.json` を以下の内容でシップする:

```json
{
  "base_branch": "develop",
  "merge_target": "develop",
  "merge_method": "squash"
}
```

以降、新規 track は develop からブランチを切り develop へ PR する。`main` には ADR / convention / architecture-rules.json などの「凍結された設計記録」が蓄積され、develop が安定したタイミングで定期的に develop → main マージで SoTOHE の "stable point" を更新する (頻度・タイミングは本 ADR スコープ外、運用判断)。

本 ADR 実装の副次更新として `.github/workflows/ci.yml` の `branches: [main]` トリガを `branches: [main, develop]` (push + PR target) に更新する。SoTOHE-core 自身の CI 設定であり、Out of Scope の責任分界 (利用者リポジトリの CI YAML 書き換え) とは独立で本 ADR が扱う。

理由:

- ADR / convention / architecture-rules.json は時間とともに増えていくが、それぞれは「凍結された設計記録」として permanent record の性格を持つ。これらが main に積み上がる構造の方が SoT の安定性を読み取りやすい
- track 実装作業は試行錯誤を含むため、develop 上で蓄積して安定したタイミングで main に反映する方が main の "stable / released" 性を保てる
- 本 ADR 実装 track は現行 `main` ベースで config 駆動化を導入する transition track であり、develop 作成後の最初の新規 track から **develop ベースの dogfooding** を開始する。本 track はそのための config / snapshot / CI 更新を同時に ship する bootstrap dogfooding を担う
- 利用者リポジトリは `.harness/config/branch-strategy.json` を上書きすれば main ベース等に戻せる (D1 で可変化済みなので、develop を default にしても利用者の選択肢を狭めない)

Implication (ブートストラップ順序):

- 本 ADR 実装 track は **現状の main ベース** で開始 (実装中はまだ config 駆動化が完了していない)
- fail-closed parser を有効化する前に、本 ADR 実装 track の `metadata.json` に `branch_strategy_snapshot: {base_branch: "main", merge_target: "main", merge_method: "squash"}` を追加する
- 実装 PR を main へマージ
- マージ後、main HEAD から `git switch -c develop` で develop ブランチを派生し push
- 以降の新規 track はすべて develop ベース (`.harness/config/branch-strategy.json` の develop 値が `branch_strategy_snapshot` に焼かれる)
- develop 作成後の最初の follow-up track で branch-base dogfooding を開始し、D4 の snapshot 不変条件を実運用で検証する
- 既存 (本 ADR 実装前に作成された) track は D5 により parse 失敗、SoTOHE は remediation しない

## Rejected Alternatives

### A. ハードコードのまま維持する

戦略選択肢を奪う。社内採用時の運用衝突を SoTOHE 側で吸収できない。却下。

### B. 環境変数のプリセットで切り替える (`BRANCH_STRATEGY=lite-gitlab` 等)

環境変数の名前空間を汚す。プリセット名と実フィールド値の対応関係を SoTOHE 側で保守することになり、業界の戦略呼称が変わるたびに追従が必要。却下。

### C. ベースブランチのみ外出しし、マージメソッドは固定 (`squash` 固定)

future-architect の標準が指摘する「feature → develop は squash、develop → feature は merge commit」のトレードオフを表現できない。マージメソッドも合わせて外出ししないと意味がない。却下。

### D. プリセット (named strategies) + フィールド override の hybrid

`strategy_name: "lite-gitlab-flow"` + 個別フィールド上書きを許す案。SoTOHE が複数戦略の正本を持つことになり、プリセット定義と実フィールド値の二重保守が発生する。プリセットなしで base/target/merge_method の 3 フィールドだけ持つ方が SoTOHE 側の知識が減り、戦略名は利用者が運用上で命名すればよい。却下。

### E. track kind (feature / hotfix / release) もセットで導入

D3 の理由により scope creep。SoT Chain と「1 track = 1 PR」前提の再設計を要し、現需要を超える複雑度。defer。

### F. 後方互換性 + migration script を提供する

D5 の理由により却下。

## Consequences

### Positive

- 利用者が Trunk-Based / GitHub Flow / Lite GitLab Flow / GitLab Flow の feature 開発フェーズを選択可能になる
- `BranchStrategyPort` という 1 つの port から戦略を読むため、SoTOHE 内部で「戦略がどこから来るか」が明確になる
- `metadata.json` snapshot により track 単位で戦略が固定され、グローバル config 変更が in-flight track を破壊しない
- 後続 ADR (Branch Strategy 関連の追加判断) は config schema 拡張で済み、再びハードコードを掘り起こす作業は不要
- SoTOHE-core 自身が D8 で develop ベースを採用することで、configurability を **dogfooding** で実証する (default config が動作することを開発の中で常時検証)
- main が「凍結された設計記録 (ADR / convention / architecture-rules.json) の蓄積層」、develop が「track 実装の積み上げ層」と役割が明確に分離する

### Negative

- 新規 port 追加に伴うレイヤー間配線 (port 定義 + adapter 実装 + DI) のコストが発生する
- **影響範囲が広い**: 本番コード 6 ファイル (branch create + PR base) + **`git rev-parse main` 差分基底 fallback 5+ ファイル (dry/shared, review_v2/shared, fixpoint_resolve, gate_state, commit_hash_store, dry テスト)** + baseline-capture doc 2 ファイル + Cargo make + Claude settings + workflow SSoT 3 ファイル + Claude commands 2 ファイル + README.md + Codex skill 1 ファイル + Claude rules 1 ファイル + **規約文書 5 ファイル** + **AI capability プロンプト 1 ファイル** = 合計 **約 28 ファイル**
- 規約文書側は `branch-strategy.md` を全文書き換え、他 4 ファイルもテキスト書き換え。レビュー観点では言葉狩りに見える PR になりやすく、レビュー粒度の設計が必要
- `metadata.json` の schema バージョンが上がる (`schema_version` 更新)
- `Makefile.toml` の `track-switch-main` → `track-switch-base` リネームは下位 surface (`.claude/commands/track/done.md`, `.claude/settings.json:159`, `.claude/rules/07-dev-environment.md:54,56`) との連動更新が必要
- PR test fixtures (`libs/infrastructure/src/gh_cli.rs`) の parametrize は必須。それ以外のテスト fixture (40+ ファイル、100+ 箇所) は test-local 慣習として `"main"` 維持可能だが、一括 parametrize するなら別 PR で扱う
- AI capability プロンプト (`adr-editor.md`) の書き換えは、AI が config 値を briefing 経由で受け取る経路の整備が必要 (adr-editor 自体が config を読まないため)

### Neutral

- Git Flow / hotfix branch first-class サポートは依然として未対応 (D3 により defer)
- `track/` プレフィックスの設定可能化は別軸 (D2 により本 ADR スコープ外)
- D8 により利用者リポジトリは default で develop ベースを受け取るが、`.harness/config/branch-strategy.json` を上書きすれば main ベース等に戻せる (D1 で可変化済み)。default 選択を変えただけで、選択肢を狭めるわけではない

## Out of Scope

- `TrackBranch` 型の domain → infrastructure 層への移動 (layer cleanup): 戦略可変化とは独立した技術的負債整理として別 ADR で扱う
- track kind 軸 (feature / hotfix / release) の導入: D3 で defer
- プレフィックスの設定可能化: D2 で固定
- 並行リリース運用 (`develop1` / `develop2` 等): future-architect の標準で言及されるが本 ADR スコープ外。`branch_strategy_snapshot` を track ごとに切り替えれば部分的に表現可能だが、SoTOHE 側で意図的に支援はしない
- 既存 track の自動マイグレーション: D5 で却下
- **利用者リポジトリの GitHub Actions CI 設定の動的書き換えサポート**: SoTOHE-core 自身の `.github/workflows/ci.yml` は D8 により develop に対応するよう本 ADR で更新するが (`branches: [main, develop]`)、利用者リポジトリの CI YAML を SoTOHE が config 値で動的に書き換える仕組みは提供しない。利用者は自分の base branch に合わせて `branches: [...]` を手動更新する責務を負う (`knowledge/conventions/responsibility-boundary.md` の精神に沿う)
- **develop ↔ main の同期運用** (D8 派生): main → develop の取り込み (main のセキュリティ修正等を develop が受け取るタイミング)、develop → main のリリースカット (頻度 / トリガ / タグ運用) は本 ADR スコープ外、SoTOHE-core の運用判断として別途定める
- **利用者リポジトリの develop ブランチ自動セットアップ**: SoTOHE をテンプレートとして採用した利用者リポジトリでは、develop ブランチを利用者が自分で作成する。SoTOHE がフォーク時に develop を自動派生する機構は提供しない
- テスト fixture 全件の `"main"` parametrize: PR test fixtures (`gh_cli.rs`) は branch 戦略依存なので置換対象だが、`init -b main` 慣習で書かれた 100+ 箇所は test-local の "default branch" convention として維持可能。一括 parametrize は別 PR で扱う
- 「main orchestrator」「main session」「main context」「main briefing」など `main` を AI セッション / brief 文書の意味で使っている false-positive surface (`pre-track-adr-authoring.md`, `.claude/agents/README.md`, `.claude/rules/10-guardrails.md`, `.harness/capabilities/review-fix-lead.md` 等): branch 概念と無関係なので置換対象外

## Reassess When

- 利用者から hotfix / release ブランチを SoTOHE 内で扱いたいという具体的なユースケースが出たとき (D3 の再評価)
- `.harness/config/branch-strategy.json` の 3 フィールドでは表現できない戦略要求が出たとき (例: 環境別マージメソッド、複数 develop の並行サポート)
- `track/` プレフィックスが他フィーチャーブランチ命名と衝突する利用者要求が出たとき (D2 の再評価)
- `TrackBranch` 型の layer cleanup が完了し、プレフィックス決定が delivery 層に移ったとき (D2 の論点構造が変わる)

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/conventions/no-backward-compat.md` — D5 の根拠 convention
- `knowledge/conventions/hexagonal-architecture.md` — `BranchStrategyPort` の配置原則
- `knowledge/adr/2026-06-15-0025-operational-docs-restructure-unified.md` — 先行 ADR (workflow.md 解体 + `branch-strategy.md` convention 新設、本 ADR の主要ランディングゾーン)
- `knowledge/conventions/branch-strategy.md` — D6 で全文書き換え対象 (本 ADR の主要 surface)
- `knowledge/conventions/{task-completion-flow,adr,dry-check-workflow,review-protocol}.md` — D6 で部分書き換え対象
- `.harness/capabilities/adr-editor.md` — D7 で書き換え対象
- `.github/workflows/ci.yml` — D8 で SoTOHE-core 自身の `main` / `develop` 対応として更新対象。利用者リポジトリ CI YAML の動的書き換えは Out of Scope
- `.harness/config/agent-profiles.json` — `.harness/config/` 配下の既存 config 配置例
- `.harness/config/signal-gates.json` (`2026-06-16-1030`) — config 駆動の chain × gate × strictness SSoT の参考実装
- `architecture-rules.json` — 既存の workspace 構造定義 (本 ADR の config はこれを直接参照しない)
