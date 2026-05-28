---
adr_id: 2026-05-26-1813-track-id-default-active-track
decisions:
  - id: D1
    user_decision_ref: "chat_segment:optional-track-id-active-track-default:2026-05-26"
    candidate_selection: "from:[A-keep-required,B-shell-side-resolution,C-env-var,D-per-command-bespoke] chose:active-track-default"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:optional-track-id-active-track-default:2026-05-26"
    candidate_selection: "from:[cli-helper-glue,usecase-op-with-port] chose:usecase-op-with-port"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:optional-track-id-active-track-default:2026-05-26"
    candidate_selection: "from:[all-track-id-commands,current-track-operating-only] chose:current-track-operating-only"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:pre-review-gate-via-cargo-make-wrapper:2026-05-27"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:optional-track-id-uniform-flag:2026-05-27"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:make-wrapper-uniform-flag-passthrough:2026-05-28"
    review_finding_ref: "pr:142:codex-review:make-wrapper-track-id-self-resolve"
    status: proposed
  - id: D7
    user_decision_ref: "chat_segment:track-id-read-write-split:2026-05-28"
    status: proposed
---
# track-id 引数を省略可能にし、省略時は現在ブランチに紐づくアクティブトラックを既定値とする

## Context

`sotp` CLI には track を対象とする多数のサブコマンドがあり、その大半が track-id を明示引数として要求している。引数の形は統一されておらず、次のように分かれている:

- 位置引数 `track_id: String`（必須）— `track transition` / `signals` / `type-signals` / `type-graph` / `baseline-graph` / `contract-map` / `catalogue-spec-signals` / `catalogue-impl-signals` / `spec-element-hash` / `baseline-capture` / `add-task` / `set-override` / `clear-override` / `next-task` / `task-counts` ほか。
- フラグ `--track-id: String`（必須）— `track lint` / `review codex-local` / `review claude-local` / `review check-approved` / `review results`。
- フラグ `--track: String`（必須・名前だけ別）— `verify catalogue-spec-refs`。
- フラグ `--track-dir: PathBuf`（パス指定）— `git commit-from-file` / `verify plan-artifact-refs`。

一方で、対象トラックの id は通常 **現在の git ブランチ** から導出できる。トラックブランチは `track/<id>` 命名であり、`<id>` 部分がそのままトラック id になる。ブランチ名 → track-id の解決ロジックは既に usecase 層 (`libs/usecase/src/track_resolution.rs`) に `resolve_track_id_from_branch`（`track/<id>` のみ受理）として存在する。git I/O はインフラ層の `GitRepository` ポート (`libs/infrastructure/src/git_cli/mod.rs` の `current_branch()`、内部で `git rev-parse --abbrev-ref HEAD`) に隔離されている。

かつて存在した `plan/<id>` ブランチと、`track/`・`plan/` 双方を受理する lenient な解決関数は、plan-only / activate レーンの削除（`2026-05-26-1123-remove-plan-only-activate-lane.md`）で取り除かれた。これにより既定解決の対象は `track/<id>` の 1 種類に一本化されており、解決方針の分岐（strict / lenient）を新たに設計する必要はない。

この既存資産の使われ方が一貫しておらず、次の重複と不整合がある:

1. **一部コマンドだけが既に省略時の自動解決に対応している**。`track resolve`（位置引数 `Option<String>`）・`track views sync`（`--track-id: Option<String>`）・`verify plan-artifact-refs`（`--track-dir: Option<PathBuf>`）は省略時にブランチから解決する。だがそれぞれが **別々の自前実装** を持つ。`plan/` 廃止前は strict（`track/` のみ）と lenient（`track/`・`plan/`）の挙動差もあったが、廃止後はいずれも単一の strict 解決へ寄っている。残る不一致は「同じ解決処理を各コマンドが個別に再実装している」点と、後述するポート迂回である。
2. **解決・ブランチ読み取りの一部が `GitRepository` ポートを迂回している**。`track resolve` と `verify plan-artifact-refs` は `SystemGitRepo`（ポート経由）を使うが、`track views sync`・`make track-commit-message` の一部ヘルパー・および active-track-write-guard が usecase interactor へ注入するブランチリーダー（`transition` ほか）は `git rev-parse` を直接シェルアウトしており、ヘキサゴナルの I/O 隔離を破っている。
3. **残りの大多数のコマンドは track-id 必須のまま**で、呼び出し側が id を用意する責任を負う。
4. **その「id を用意する」処理が shell 側に散在している**。`Makefile.toml` の複数の `*-local` タスク（`verify-plan-artifact-refs-local` / `verify-catalogue-spec-refs-local` / `check-catalogue-spec-signals-local` / `verify-spec-states-current-local` ほか）に、`BRANCH=$(git ...); TRACK_ID="${BRANCH#track/}"; TRACK_DIR="track/items/$TRACK_ID"` という同型の shell ボイラープレートが重複している。実際、plan-only / activate レーン削除後も一部タスクには既に dead な `plan/*` の case アームが残っており、解決規則が shell 側と Rust 側で二重化している弊害が表れている。これは shell 文字列処理を安全な Rust 引数処理へ置き換えるという `sotp` 導入の方向性（`knowledge/conventions/shell-parsing.md`）に逆行している。

この状態が、track 系コマンドを **pre-commit フックや pre-review コマンドへ素直に組み込めない** 原因になっている。フックやレビュー前処理が走る文脈では現在ブランチが当該トラックブランチであるにもかかわらず、コマンドは track-id を要求するため、呼び出し側に shell でのブランチ解析を強いることになる。

なお、現在ブランチとトラックの紐付けという同じ概念は、書き込み保護の文脈で `2026-05-26-0518-active-track-write-guard.md` が「現在ブランチに紐づくトラックのみ書き込み許可」として決定し、既に実装されている。この実装はブランチ紐付きバリデーションを usecase 層（`TaskOperationInteractor` ほか）に置き、ブランチ読み取りを cli から注入する形を採るが、その読み取りは前述のとおり `git rev-parse` の直接シェルアウトである。本 ADR はこの紐付けを **引数の既定値解決** へ拡張し、バリデーションと既定値解決の双方を同一の解決経路（ポート経由）に集約する位置付けとなる。

D1 によって track-id が省略可能になる直接の恩恵として、`cargo make` ラッパーが信号再生成コマンドを bare chain として組み込めるようになる。`sotp track type-signals` / `sotp track catalogue-spec-signals` / `sotp track views sync` / `sotp review local` はそれぞれが track-id を省略時に現在ブランチから自己解決するため、`cargo make track-local-review` はこれら自己解決コマンドを順に呼ぶだけでよく、ラッパー/Makefile 側に `BRANCH=$(git ...); TRACK_ID="${BRANCH#track/}"` のような shell でのブランチ解析を置かなくて済む。同じ bare chain を `cargo make track-commit-message` の pre-commit ゲートにも適用することで、レビューとコミットが同じ手順で更新された状態を観測するようになる。この対称性がなければ、レビュー承認時の `*-types.md` シグナル列がコミットゲートで生成される状態と一致せず、ハッシュのずれにより `check-approved` が通らない問題が構造的に発生する。

## Decision

### D1: track-id 引数を省略可能にし、省略時は現在ブランチに紐づくアクティブトラックへ既定解決する

track を対象とするサブコマンドの track-id 引数を、必須から省略可能へ変更する。省略された場合は現在の git ブランチからアクティブトラックの id を解決し、それを既定値として用いる。

- **明示指定は上書きとして残す**: 引数で track-id を渡した場合はそれを使う。ブランチに紐づかない文脈や別トラックを明示的に対象にしたい場合のために、明示経路は維持する。
- **fail-closed**: track-id を省略し、かつ現在ブランチがトラックブランチ（`track/<id>`）でない（`main` / detached HEAD など）場合は、サイレントスキップや警告に留めず明示エラーで停止し、明示 track-id の指定を促す。トラック文脈を必須としない一部コマンド（例: registry のみを対象にできる `track views sync`）で「トラックなしモード」が意味を持つ場合に限り、そのコマンド固有の挙動として例外を文書化する。
- **書き込み保護との関係**: 省略時に導出した id は定義上現在ブランチと一致するため、`2026-05-26-0518-active-track-write-guard.md` のブランチ紐付きバリデーションを自動的に満たす。明示 id がブランチと一致しない書き込みを伴う操作への explicit override の可否、および escape hatch の扱いは D7 で決定する。

### D2: アクティブトラック解決を単一の共有経路に集約し、git 読み取りは `GitRepository` ポート経由に統一する

省略時の解決を各コマンドで個別実装せず、**単一の共有経路** に集約する。

- 既存の `usecase::track_resolution`（ブランチ名 → track-id のパース規則、現在は `resolve_track_id_from_branch` の 1 本）を再利用し、解決処理を重複させない。
- git の現在ブランチ読み取りはすべて infrastructure の `GitRepository` ポート経由に統一し、`track views sync`・`make` ヘルパー・write-guard 注入リーダーに残る `git rev-parse` の直接シェルアウトを取り除く。
- 配置はヘキサゴナルアーキテクチャと `2026-04-30-0848-cli-via-usecase-only.md` に従い、「現在ブランチからアクティブトラックを解決する」操作を **usecase 層の操作** として公開する。usecase はブランチ読み取りをポート（branch reader port）として受け取り、cli は composition root として `SystemGitRepo` アダプタを wiring して渡す。これにより解決ロジックは git 実体なしで単体テスト可能になり、cli 側は薄い wiring に留まる。
- active-track-write-guard が既に導入したブランチ紐付きバリデーション（usecase 内）と、それが用いるブランチ読み取りも、本 ADR の単一解決経路へ統合する。バリデーション（明示 id の照合）と既定値解決（省略時の導出）が同じ解決を共有し、git 読み取りは `GitRepository` ポート 1 本に集約する。

`Makefile.toml` の `*-local` タスクに散在する shell でのブランチ解析は、コマンド自身が解決を担うことで不要になり、`cargo run -p cli -- verify catalogue-spec-refs` のように track 引数なしで呼べるようになる。

### D3: 既定解決の適用範囲は「現在のトラックを操作するコマンド」に限り、ブランチ移動系は明示指定のままとする

既定解決は、実行時に現在のトラックを対象とするコマンド（`transition` / `signals` / `type-signals` / `type-graph` / `baseline-graph` / `contract-map` / `catalogue-spec-signals` / `catalogue-impl-signals` / `spec-element-hash` / `baseline-capture` / `add-task` / `set-override` / `clear-override` / `next-task` / `task-counts` / `lint` / `review *` / `verify catalogue-spec-refs` など）に適用する。

ブランチ移動に関わるコマンド（`branch create` / `branch switch`）は適用対象から除外し、track-id を明示必須のままにする。これらは「現在いないトラック」を対象に動くコマンド（`branch create` は通常 `main` 上で実行、`switch` は移動先トラックを指定）であり、現在ブランチからの導出は意味をなさないか誤りになるため。

### D4: pre-review ゲートを pre-commit ゲートと対称にする（`cargo make` ラッパーの bare chain）

各ゲートコマンド（`sotp track type-signals` / `sotp track catalogue-spec-signals` / `sotp track views sync` / `sotp review local`）は D1 により track-id が省略可能であり、track-id が省略された場合は**コマンド自身が**現在ブランチからアクティブトラックを内部解決する。ブランチ解決ロジックは各コマンドに内在し、ラッパーや Makefile は解決に関与しない。

regen ゲート列（`sotp track type-signals` → `sotp track catalogue-spec-signals` → `sotp track views sync`）は**単一の共有 `cargo make` タスク**（例: `track-active-gate` のような private task。名前は実装時に決める）に**一元定義**する。`cargo make track-local-review`（pre-review）と `cargo make track-commit-message`（pre-commit）はともにこの共有ゲートタスクを**依存として呼び出した後**、各自固有の tail を実行する。

```
共有ゲートタスク (track-active-gate):
  sotp track type-signals → sotp track catalogue-spec-signals → sotp track views sync

track-local-review:   track-active-gate → sotp review local
track-commit-message: track-active-gate → cargo make ci → check-approved → commit
```

各ラッパー自身は track-id を保持せず、`BRANCH=$(git ...); TRACK_ID="${BRANCH#track/}"` のような shell でのブランチ解析を一切行わない。regen 列を両ラッパーに重複定義しないことで、片方だけ変更されて pre-review/pre-commit が非対称になる回帰を構造的に防ぐ。

- **解決はコマンドに内在**: `sotp` の各コマンドが D1 の省略時解決を自分で行うため、ラッパーは track-id に関する知識を持たなくてよい。Makefile/shell へのブランチ解析残留（`Makefile.toml` の `*-local` タスクに `BRANCH=$(git ...); TRACK_ID="${BRANCH#track/}"` のような shell ブランチ解析が散在する現状の重複と同種のもの）を新たに生み出さない。
- **`sotp` コマンドの責務は変えない**: 各 `sotp` サブコマンドは単一の操作（type-signals 再生成、catalogue-spec-signals 再生成、ビュー同期、レビュー）のみを担う。chain の組み立てはラッパーが担うが、個々のコマンドにオーケストレーション責務を追加しない。
- **fail-closed の維持**: chain の途中ステップが失敗した場合は後続ステップを実行せずに停止する。部分的に更新された状態でレビューを進めて後でハッシュずれが発覚するよりも、失敗を明示して停止する。
- **regen 列の DRY 定義**: regen ゲート列（type-signals → catalogue-spec-signals → sync-views）は共有タスクに一元定義し、`track-local-review` と `track-commit-message` の両ラッパーがそれを呼び出す。同じ列を両ラッパーに重複定義しない。これにより、片方のゲートだけ変更されて pre-review と pre-commit が非対称になる回帰を構造的に防ぐ。
- **commit ゲートの検査内容は不変**: `cargo make track-commit-message` の bare chain 化は、regen 系コマンド（type-signals / catalogue-spec-signals / sync-views）の呼び出し形（shell 解決 → 裸の自己解決コマンド）のみを変える。commit ゲートが持つ厳格な検査 — フル `cargo make ci`、および `check-approved`（レビュー未承認なら fail-closed で BLOCK）— は一切削除・弱体化しない。D4 は pre-review に regen ゲートを**追加**するものであり、commit ゲートを緩めるものではない。bare chain 化によってこれらの検査が落ちる回帰は fail-open であり禁止する。

### D5: 明示 track-id 上書きの形式は統一フラグ `--track-id` とし、全コマンドで名前を揃える

D1 で「明示指定は上書きとして残す」と決定したが、その *形式*（位置引数かフラグか）と *名前* は D1 の範囲外だった。PR #142 の @codex レビューでコマンド間の不統一が指摘されたため、この次元の設計を明示的に決定する。

- **フラグ形式を採用する**: 対象コマンド（D3 の IN-01 コマンド群: `transition` / `signals` / `type-signals` / `type-graph` / `baseline-graph` / `contract-map` / `catalogue-spec-signals` / `catalogue-impl-signals` / `spec-element-hash` / `baseline-capture` / `add-task` / `set-override` / `clear-override` / `next-task` / `task-counts`）に対して、明示 track-id 上書きを**位置引数ではなくフラグ `--track-id`** として公開する。
  - 複数の位置引数を持つコマンド（例: `transition <track_id> <task_id> <status>`）では、先頭の位置引数を省略可能にすると後続の必須位置引数との境界が曖昧になり、clap がパースを誤る。フラグにすることでこの曖昧さを回避できる。
  - 明示 track-id 上書きは「現在ブランチ外のトラックを対象にしたい」まれなケースであり、フラグの若干の冗長さは許容できる。省略時の自動解決が大多数の用途を担うため、フラグの記述量が問題になる頻度は低い。
- **フラグ名を `--track-id` に統一する**: すべての track 対象コマンドでフラグ名を `--track-id` に揃える。現状で `--track` という別名を持つ `verify catalogue-spec-refs` は、`--track-id` に改名する。この統一により、コマンドをまたいでフラグ名を使い分ける必要がなくなる。
- **`--track-dir` は対象外**: `git commit-from-file` / `verify plan-artifact-refs` の `--track-dir` は track-id スラグではなくファイルシステムパスを受け取る引数であり、別の概念である。この名前統一の対象に含めない。

D5 の uniform `--track-id` フラグを `cargo make` task wrapper 起動層まで延長する方針は D6 で決定する。

### D6: `cargo make` task wrapper は positional track-id を課さず、`--track-id` を素通しする

D5 は `sotp` CLI コマンド定義層（`apps/cli/src/commands/track/` の各サブコマンド）での `--track-id` フラグ統一を決定した。しかし `apps/cli/src/commands/make.rs` の `dispatch_track_*` 関数群（`dispatch_track_add_task` / `dispatch_track_transition` / `dispatch_track_next_task` / `dispatch_track_task_counts` / `dispatch_track_set_override`）は、位置引数の先頭要素を強制的に `track_id` として消費する実装になっている。このため `cargo make track-add-task -- "write docs"` のように track-id なしで呼ぶと `"write docs"` が track_id として誤消費され、D1 の省略時自己解決が wrapper 経由では働かない。

PR #142 の @codex レビューで上記の問題が "Allow make wrappers to use the active track default" として指摘された。

この問題を解決するため、以下のように決定する。

- **wrapper は positional track-id を課さない**: 対象 wrapper（`track-add-task` / `track-transition` / `track-next-task` / `track-task-counts` / `track-set-override`）は先頭位置引数を track_id として読み取ることをやめる。各 wrapper が最低限行うのは `--items-dir track/items` の注入であり、それ以外の引数はすべて underlying `sotp` コマンドへ素通しする。ただし `dispatch_track_set_override` は例外として、最初の非フラグ位置引数をステータス語（`blocked` / `cancelled` / `clear`）として読み取り、`track set-override` と `track clear-override` のいずれを呼ぶかをルーティングする（この読み取りは track-id の消費ではなく、Makefile の単一エントリポイント `track-set-override` を 2 つの underlying コマンドに振り分けるための内部ルーティングである）。
- **`--track-id` を明示するか省略する**: track-id を使いたい呼び出し側は `cargo make track-add-task -- --track-id <id> "write docs"` のようにフラグとして渡す。省略した場合は D1 の自己解決（現在ブランチ → active track）が働く。これにより `track/<id>` ブランチ上では `cargo make track-add-task -- "write docs"` が期待どおり動く。
- **対象 wrapper の列挙**:
  - `dispatch_track_add_task` — `track add-task`
  - `dispatch_track_transition` — `track transition`
  - `dispatch_track_next_task` — `track next-task`
  - `dispatch_track_task_counts` — `track task-counts`
  - `dispatch_track_set_override` — `track set-override` / `track clear-override`
- **wrapper は track-id 知識を持たない**: D4 で定めた「ラッパーは track-id に関する知識を持たなくてよい」という原則（bare chain wrapper の方針）を task wrapper へも適用する。
- **interface 変更を伴う**: 既存の positional 呼び出し（`.claude/commands/track/implement.md` / `full-cycle.md` / `knowledge/conventions/task-completion-flow.md` / `apps/cli/src/commands/make_tests.rs` における `cargo make track-transition <track_dir> ...` など）は、`--track-id <id>` フラグ形式または track-id 省略形式へ更新が必要になる。

### D7: explicit `--track-id` の意味論を、操作が `track/items/<id>/` 配下へのファイル書き込みを伴うか否かで分岐する

D1 は「明示 id がブランチと一致しない書き込みを伴う操作への explicit override の可否は D7 で決定する」としていた。ここでその分岐を確定する。

**分類基準**: 操作が `track/items/<id>/` 配下の任意ファイルを書くか否か。

- **書き込みを伴う操作**（以下 WRITE 分類）: `signals`（spec.json / spec.md 再生成） / `type-signals` / `catalogue-spec-signals` / `baseline-graph` / `contract-map` / `baseline-capture` / `transition` / `add-task` / `set-override` / `clear-override` / `views sync` / `review local`（`codex-local` / `claude-local` を含む、`review.json` を書く）
- **読み取り専用操作**（以下 READ 分類）: `catalogue-impl-signals` / `spec-element-hash` / `next-task` / `task-counts` / `lint` / `resolve` / `review results` / `review check-approved`
- **設計対象外**: `type-graph` は現時点で削除済みスタブであり、この決定の対象外とする。

この分類は現時点で判明している操作の列挙であり、今後コマンドを追加する際は上記基準（`track/items/<id>/` への書き込み有無）に従って分類する。

**WRITE 操作における explicit `--track-id` の扱い**:

explicit `--track-id` が渡された場合、その値を現在ブランチから導出した track-id と照合する。一致しなければ fail-closed エラーとして停止する。非トラックブランチ（`main` など `track/<id>` 形式でないブランチ）では導出 id が存在しないため、同様に fail-closed で停止する。

escape hatch（`--skip-branch-check` 相当のフラグ）は設けない。WRITE 操作は working-tree のコードや現在ブランチ文脈に依存して `track/items/<id>/` を書くため、別トラック id を渡すと誤った文脈で書き込みが生じる。escape hatch を持つと、ユーザーが誤操作した際に壊れた状態を意図せず確定させるリスクがある。

既存の実装例として `apps/cli/src/commands/track/tddd/baseline_graph.rs` および `contract_map.rs` はすでに「explicit id がブランチと一致しなければ拒否」を実装している。D7 はこのパターンを全 WRITE 操作へ一般化する。

D1 の「明示指定は上書きとして残す」という方針は、D7 により WRITE 操作については撤回し、READ 操作のみに適用を限定する。

**READ 操作における explicit `--track-id` の扱い**:

explicit `--track-id` は他トラックを対象にするための override として機能する。READ 操作は `track/items/<id>/` の状態を変えないため、別トラック id を渡しても誤った書き込みは発生しない。任意のブランチから指定トラックのアーティファクトを参照するユースケース（例: main ブランチから過去トラックの結果を確認する）を妨げない。省略時は D1 の通り現在ブランチから自己解決する。

## Rejected Alternatives

### A. track-id を全コマンドで必須のまま維持する（現状維持）

却下理由: 呼び出し側に id を用意する責任が残り、フック／レビュー前処理に組み込むたびに shell でのブランチ解析が必要になる。現状の重複（Makefile の `*-local` タスク群）と不整合（一部だけ自動解決対応）も解消されない。

### B. ブランチ解決を shell 側（Makefile / フック）に置き続ける

各タスク／フックで `git rev-parse` とプレフィックス除去を行い、解決済み id をコマンドへ渡す案。

却下理由: 同型の shell ボイラープレートが多数の `*-local` タスクへ重複し続け、shell 文字列処理を Rust に寄せるという `sotp` 導入方針（`shell-parsing.md`）に反する。解決規則（`track/` 判定・slug 検証・エラー扱い）が Rust 側 (`track_resolution`) と shell 側に二重化し、片方だけ直すと挙動がずれる（plan-only / activate 削除後も Makefile に dead な `plan/*` アームが残っているのが実例）。

### C. 環境変数で track-id を引き回す（`SOTP_TRACK_ID` など）

却下理由: アクティブトラックの真の手がかりは「現在ブランチ」であり、別途環境変数を設定・同期させる運用は現在ブランチと乖離するリスクを生む。フックやレビュー前処理が環境変数の事前設定に依存し、設定漏れがサイレントな誤対象を招く。ブランチから直接解決すれば設定の二重管理が不要になる。

### D. コマンドごとに個別の auto-detect 実装を増やす（現在の `resolve.rs` / `views.rs` 方式の踏襲）

省略対応を必要なコマンドへ個別の関数として都度追加する案。

却下理由: 既にこの方式がポート迂回（直接シェルアウト）の不整合を生んでいる。約 15 以上のコマンドへ同じ解決処理を個別複製すると重複が拡大し、`knowledge/conventions/`（重複実装の抑止）にも反する。D2 の単一共有経路に集約する方が保守コストが低い。

### D2 内の代替: cli ヘルパーで git 読み取りと usecase パースを糊付けする

解決を usecase 操作にせず、cli 層の共有ヘルパー関数（`SystemGitRepo` 呼び出し + `usecase::track_resolution`）に集約する案。

却下理由: composition root での糊付けは `cli-via-usecase-only` の「操作は usecase 経由」という方向性から見て薄い orchestration が cli に残る。また git 実体なしの単体テストがしにくい。ただし新規ポート導入を避けられる軽量案ではあるため、型設計フェーズで write-guard 側の既存抽象（usecase 注入リーダー）との整合を見て最終判断する余地は残す。

## Consequences

### Positive

- pre-commit フック・pre-review コマンドが track-id を渡さずに `sotp` サブコマンドを呼べるようになり、現在ブランチが当該トラックブランチである限り素直に組み込める。
- `Makefile.toml` の `*-local` タスクから shell でのブランチ解析ボイラープレートが除去でき、解決規則が Rust 側 (`track_resolution`) に一本化される（残存する dead な `plan/*` アームも解消できる）。
- 既存の自前 auto-detect 実装（`resolve` / `views sync` / `plan-artifact-refs`）と直接シェルアウト経路（write-guard 注入リーダーを含む）が単一経路に統合され、ポート迂回が解消する。
- 明示指定を残すことで、別トラックや非トラックブランチ文脈の操作経路を失わない。
- plan-only / activate レーン削除により解決対象が `track/<id>` に一本化済みで、strict / lenient の方針分岐を設計せずに済む。
- レビューとコミットが同じ再生成済み状態を観測するようになり、`*-types.md` シグナル列のハッシュずれによる `check-approved` ブロックが構造的に解消する（D4）。commit ゲートの厳格な検査（`cargo make ci` + `check-approved`）は維持され、D4 はその手前に regen ゲートを追加するのみ。regen 列を共有タスクに一元定義することで、片方のゲートだけ変更されて pre-review/pre-commit が非対称になる回帰も構造的に防ぐ（DRY）。

### Negative

- 約 15 以上のコマンドの引数定義（必須 → 省略可能）と解決呼び出しの差し替えが必要で、影響範囲が広い。
- active-track-write-guard が直近で追加したブランチ読み取りは `git rev-parse` の直接シェルアウト（usecase interactor へのクロージャ注入）であり、本 ADR の単一経路化ではその注入箇所もポート経由へ揃える refactor が伴う。
- `cargo make track-local-review` が type-signals / catalogue-spec-signals / sync-views を chain するため、レビュー呼び出し全体の所要時間が増える（D4）。

### Neutral

- 既定解決を usecase 操作として公開するため、branch reader port と DTO/エラー型を usecase 側に整える必要があるが、これは `cli-via-usecase-only` の boundary 方針に沿った追加であり、新たな lint 機構は要らない。

## Reassess When

- ブランチに紐づかない文脈での実行が常態化した場合（CI の detached HEAD、ブランチ外からのバッチ実行など）。fail-closed の既定が正当な操作を妨げるなら、明示指定の運用や解決基準の拡張を再検討する。
- 複数トラックを同時に操作する必要が出た場合（「現在ブランチ = 単一トラック」という前提が崩れる）。
- branch reader port 導入の負担が、cli ヘルパーで糊付けする軽量案（D2 代替）の利点を上回ると判明した場合。

## Related

- `knowledge/adr/2026-05-26-0518-active-track-write-guard.md` — 現在ブランチとトラックの紐付けを書き込み保護として決定（実装済み）。本 ADR はその紐付けを引数の既定値解決へ拡張し、解決ロジックとブランチ読み取りを共有・集約する。
- `knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md` — plan-only / activate レーンを削除。これにより `plan/<id>` ブランチと lenient 解決関数が無くなり、`sotp track activate` コマンドも消滅した。本 ADR の解決対象は `track/<id>` に一本化され、除外対象のブランチ移動系コマンドは `branch create` / `branch switch` のみとなる。
- `knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md` — cli は usecase 経由で操作にアクセスする方針。D2 の配置判断（解決を usecase 操作として公開）の根拠。
- `knowledge/conventions/shell-parsing.md` — shell 文字列処理を安全な Rust 引数処理へ置き換える方針。Makefile の shell ボイラープレート除去の根拠。
- `knowledge/conventions/workflow-ceremony-minimization.md` — 状態フィールドではなく現在ブランチを判定基準にする方向性。
- `libs/usecase/src/track_resolution.rs` — 既存のブランチ名 → track-id 解決（`resolve_track_id_from_branch`）。本 ADR で再利用する。
- `libs/infrastructure/src/git_cli/mod.rs` — `GitRepository` ポート（`current_branch()`）。git 読み取りの統一先。
