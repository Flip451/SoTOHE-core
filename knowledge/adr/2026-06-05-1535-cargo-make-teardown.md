---
adr_id: 2026-06-05-1535-cargo-make-teardown
decisions:
  - id: D1
    user_decision_ref: "chat_segment:cargo-make-teardown:2026-06-04"
    candidate_selection: "from:[delete-daemon-and-exec, keep-daemon, keep-but-undocument] chose:delete-daemon-and-exec"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:cargo-make-teardown:2026-06-05"
    candidate_selection: "from:[delete-dead-composition-twin-only, abolish-make-subcommand-entirely] chose:abolish-make-subcommand-entirely (D6 で make 完全廃止が確定したため、clap 層 apps/cli/src/commands/make.rs と composition 層 apps/cli-composition/src/make.rs の make ディスパッチを両方削除する)"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:cargo-make-teardown:2026-06-04"
    candidate_selection: "from:[native-direct-drop-wrapper, keep-thin-cargo-make-wrapper, keep-sotp-make-namespace] chose:native-direct-drop-wrapper"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:cargo-make-teardown:2026-06-04..2026-06-05"
    candidate_selection: "from:[via-sotp-make-layer, cargo-make-direct-to-bin-sotp-git, cargo-make-direct-raw-git] chose:cargo-make-direct-to-bin-sotp-git (git 書き込みは cargo make 経由を維持しつつ、冗長な bin/sotp make 層を削除し、cargo make task が command/args 配列で直接 bin/sotp git <sub> を呼ぶ。sotp git のロジック=scratch除外/branch guard/path検証/checkout+pull連結 は保持。引数安全性は @shell ではなく command/args 配列で担保)"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:cargo-make-teardown:2026-06-04"
    candidate_selection: "from:[keep-orchestration-and-docker-gates, flatten-everything] chose:keep-orchestration-and-docker-gates"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:cargo-make-teardown:2026-06-05"
    candidate_selection: "from:[shrink-make + cargo-make-script, shrink-make + sotp-native-ci, abolish-make + cargo-make-thin-orchestration] chose:abolish-make + cargo-make-thin-orchestration (bin/sotp make 完全廃止。commit/track-commit-message は cargo make script の薄い繋ぎこみ=git add-all → cargo make ci → review check-approved → dry check-approved → git commit-from-file → track set-commit-hash のみ。ロジックは native に内包し cargo make へ流出させない: しきい値解決→dry check-approved、hash永続化+復旧ヒント→track set-commit-hash(新設)、CIログ整形→廃止。CI は cargo make ci で同一ツール内、逆流サイクル解消)"
    status: proposed
  - id: D7
    user_decision_ref: "chat_segment:cargo-make-teardown:2026-06-05"
    candidate_selection: "from:[delete-orphans-and-fix-ghost, keep-orphans] chose:delete-orphans-and-fix-ghost (参照のないタスク test-nightly / track-baseline-capture wrapper / verify-doc-links public wrapper を削除。hooks-selftest ghost 参照は hook が bin/sotp hook dispatch の Rust 実装で Rust テストにカバーされるため専用タスクを新設せずドキュメント参照を削除)"
    status: proposed
  - id: D8
    user_decision_ref: "chat_segment:cargo-make-teardown:2026-06-05"
    candidate_selection: "from:[sync-all-consumers, leave-references-stale] chose:sync-all-consumers"
    status: proposed
  - id: D9
    user_decision_ref: "chat_segment:cargo-make-teardown:2026-06-05"
    candidate_selection: "from:[dedicated-config-file, agent-profiles-field, hardcoded-constant] chose:dedicated-config-file-fail-closed (.harness/config/dry-check.json を新設し threshold を管理。優先順位 CLI --threshold(Option,既定なし) > 設定ファイル。設定が読めなければ 0.85 フォールバックせず fail-closed でエラー。読み込みは native subcommand 内包。0.85 ハードコード3箇所と resolve_commit_dry_threshold を廃止し .harness/config/dry-check.json を SSoT、初期値はテンプレートの .harness/config/dry-check.json のみに置く)"
    status: proposed
---
# cargo make ラッパー層の解体 — bin/sotp 直叩きへの一本化と git 操作・再現性ゲートの選択的維持

## Status

Proposed

## Context

`cargo make`（`Makefile.toml`）は約 90 タスクを抱えるが、その多くが Rust CLI バイナリ `bin/sotp` を薄く包むだけのラッパーになっている。経緯として、`Makefile.toml` の `@shell` スクリプトに散在していた引数補間（shell の文字列展開）の安全性問題を解消するため、各タスクのロジックを Rust 側の `bin/sotp make <task>` ディスパッチャに移し、`cargo make` をその後方互換ラッパーとして残した。この「`cargo make` を正規入口にして `bin/sotp` を包む」方針が、結果的にほぼ全タスクをラップする方向へ広がり、重複・循環・死蔵を生んでいる。

現状、ワークフロー系タスクの大半は 3 層を通る。

```text
cargo make track-resolve          層1: Makefile.toml の wrapper
  └→ bin/sotp make track-resolve  層2: sotp の "make" 互換ディスパッチャ
       └→ bin/sotp track resolve  層3: native subcommand（実体）
```

確認できた問題は 4 つ。

1. **3 層パススルー**: `bin/sotp make` の 30 タスクのうち 27 は、native subcommand（`track` / `pr` / `git` / `review` / `plan` / `dry`）への引数整形だけを行う薄いパススルーである。実体のあるオーケストレーションは `commit` / `track-pr` / `track-commit-message` の 3 つだけ。`make` 名前空間は `track-*`（ハイフン名）として native の `track <sub>` を二重化している。

2. **二重実装（死蔵コード）**: `make` ディスパッチは 2 か所に存在する。clap 層（`apps/cli/src/commands/make.rs`）と composition 層（`apps/cli-composition/src/make.rs`）である。clap 層は `track-commit-message` のみ composition 層へ委譲し、残りは自前で `bin/sotp` を呼ぶ。その結果、composition 層の `make_*` メソッド群（約 30 個）は `make_track_commit_message` を除いて呼び出し元が無く、死蔵している（内部の相互呼び出しとテストを除く）。

3. **逆流（循環依存）**: `bin/sotp make commit` と `bin/sotp make track-commit-message`（およびその composition 双子）は、内部で `cargo make ci` をサブプロセス起動する。`cargo make → bin/sotp → cargo make → docker → cargo make` という循環が成立しており、依存方向が一方向でない。

4. **未使用のリゾート常駐デーモン**: `compose.yml` の `tools-daemon`（`tools` サービスを `extends` し `command: sleep infinity` を足したもの）は「高速 TDD ループ」のための任意の最適化である。bootstrap・CI・すべての `docker compose run --rm` ゲートはこれにまったく依存しない。維持コストとして、`Makefile.toml` の 10 タスク（`tools-up` / `tools-down` と `*-exec` 8 個）、`.claude/settings.json` の許可エントリ、`bin/sotp make exec`（`-local` サフィックス規約を Rust に焼き込む箇所）、テスト、ドキュメント数か所が紐づく。

加えて、削除方針を制約する重要な仕組みがある。git 操作ブロックフック（`bin/sotp hook dispatch block-direct-git-ops`、実体は `libs/domain/src/guard/policy.rs`）は、**「git」ではないコマンドでも、その引数のどこかに `git` という文字列が部分一致で含まれていれば即ブロックする**（`command_contains_git`）。このため `bin/sotp git <subcommand>` を Bash から直接実行すると常にブロックされる。一方 `cargo make <task>` や `bin/sotp make <task>` は（タスク名に `git` を含まなければ）フックを通過し、その**プロセス内部**で `bin/sotp git ...` を実行する（プロセス内実行はフックの走査対象外）。つまり現状の `cargo make` 経由の git ラッパーは、引数安全化だけでなく**フック回避の正規ルート**としても機能している。これは git 操作系の扱いを他と分ける根拠になる。

## Decision

棚卸しの原則は次の 2 つとする。

1. **`bin/sotp` の native subcommand を直接呼べば済むものは `cargo make` でラップしない**。複数コマンドのオーケストレーション（繋ぎこみ）、Docker による再現性ゲート、および git 書き込み操作（フック制約）だけを `cargo make` に残す。
2. **ロジックは `bin/sotp`（Rust）に置き、`cargo make` へ流出させない**。`cargo make` に残すのは「複数 native コマンドの順次実行（繋ぎこみ）」と「docker / setup の入口」だけとする。計算・条件分岐・データ処理・出力整形といったロジックを `cargo make` の `@shell` script に書かず、テスト可能な Rust の native subcommand に置く。`make_track_commit_message` のような複合処理は、繋ぎこみ部分を `cargo make` script に、各ステップの中身を native subcommand に分離する。

### D1: tools-daemon と daemon-exec タスク群を削除する

`compose.yml` の `tools-daemon` サービス定義と、それに依存する `Makefile.toml` の 10 タスク（`tools-up` / `tools-down` / `fmt-exec` / `clippy-exec` / `test-exec` / `test-one-exec` / `check-exec` / `machete-exec` / `deny-exec` / `llvm-cov-exec`）を削除する。`bin/sotp make exec`（`tools-daemon` へ `docker compose exec` するバリアントで、`{task}-local` の命名規約を Rust 側に焼き込んでいる）も併せて削除する。`tools` サービス本体と Dockerfile の `tools` ステージ、`docker compose run --rm tools ...` 経由のゲート群は影響を受けない。日々のフィードバックループは `run --rm` ラッパーに一本化する。並列ワーカー隔離（`WORKER_ID` / `CARGO_TARGET_DIR_RELATIVE`）は `run --rm` でも使えるため維持する。

### D2: bin/sotp make サブコマンドを完全に削除する

D6 で `bin/sotp make` を完全廃止すると決めたため、clap 層（`apps/cli/src/commands/make.rs`、`MakeTask` enum と `dispatch_*`）と composition 層（`apps/cli-composition/src/make.rs`、`make_*` メソッド群）の双方を削除する。`apps/cli/src/main.rs` の `CliCommand::Make` 配線も外す。これにより 3 層パススルー（問題 1）と二重実装の死蔵コード（問題 2）が同時に消える。`make_track_commit_message` が持っていた複雑なゲート連結は D6 に従い cargo make script へ移す。`make` が呼んでいた個々の native subcommand（`track *` / `pr *` / `git *` / `review *` / `plan *` / `dry *`）はそのまま残り、新しい入口（cargo make script または native 直叩き）から呼ばれる。

### D3: 非 git のパススルータスクを bin/sotp native 直叩きへ移行し、対応する cargo make wrapper と sotp make バリアントを削除する

git 書き込みを伴わないタスクは、呼び出し側（スラッシュコマンド・スキル・エージェント定義・運用ドキュメント）を `bin/sotp <native> <args>` の直接呼び出しに書き換え、`Makefile.toml` の該当 wrapper と `bin/sotp make` の該当バリアントを削除する。対象は以下のような、native subcommand への単純パススルーである。

- `track-resolve` → `bin/sotp track resolve`
- `track-transition` / `track-add-task` / `track-next-task` / `track-task-counts` / `track-set-override` / `track-sync-views` → `bin/sotp track ...`
- `track-review-results` / `track-check-approved` → `bin/sotp review results` / `bin/sotp review check-approved`
- `track-local-plan` / `track-local-review` / `track-local-review-fix-codex` / `track-local-dry-fix` → `bin/sotp plan codex-local` / `bin/sotp review local` / `bin/sotp review fix-local` / `bin/sotp dry fix-local`
- `track-signals` / `track-baseline-capture` → `bin/sotp track signals` / `bin/sotp track baseline-capture`
- `pr` 系のうち git push を伴わないもの（`track-pr-status`=gh 読み取り / `track-pr-merge`=git fetch + gh merge / `track-pr-ensure`=gh pr create のみ）→ `bin/sotp pr ...`。git push を伴う `track-pr-push` / `track-pr` / `track-pr-review`（`pr_review_cycle` は内部で push する）は D4（cargo make 経由）で扱う。

これらは現状 native subcommand が既に存在し、`bin/sotp make` 側は引数整形（`--items-dir track/items` の注入、`--` の除去、positional→flag 昇格など）を行っているだけである。整形の必要分は native subcommand 側の既定値・引数処理に寄せ、中間 2 層を消す。

### D4: git 書き込み操作は cargo make 経由を維持し、cargo make が直接 bin/sotp git を呼ぶ

`git add` / `git commit` / `git push` / `git switch` / `git branch -d` / `git notes add` / `git restore --staged` などの git 書き込みを伴う操作は、`cargo make` を入口として維持する。ただし冗長な `bin/sotp make` 層は通さず、`cargo make` task が `command`/`args` 配列形式で直接 `bin/sotp git <sub>`（ブランチ作成など track 経由のものは `bin/sotp track branch ...`）を呼ぶ。

対象タスク: `add` / `add-all` / `unstage` / `note` / `track-note` / `track-add-paths` / `track-branch-create` / `track-branch-switch` / `track-switch-main`、および git push を伴う `track-pr-push` / `track-pr` / `track-pr-review`（`pr_review_cycle` は内部で push する）。`commit` / `track-commit-message` は複数ステップのオーケストレーションなので D6 で扱う。push を伴わない `track-pr-status` / `track-pr-merge` / `track-pr-ensure` は D3（native 直叩き）に分類する。

**sotp git のロジックは保持する**。`bin/sotp git` の各サブコマンドは単純な git ラッパーではなく、捨てると退行する処理を持つ（`add-all`=transient scratch ファイル除外、`commit-from-file`=track ブランチガード、`add-from-file`=パス検証と重複排除、`switch-and-pull`=checkout と pull の連結）。よって git ロジックは `bin/sotp git` 側に残し、`cargo make` からはそれを呼ぶだけにする。`cargo make` の `command`/`args` 配列（`@shell` script ではない）を使えば、`bin/sotp make` を作った元の動機である引数安全性（shell 文字列展開の回避）も満たせる。

**フック制約の根拠**: Context 末尾のとおり、`block-direct-git-ops` フックは引数のどこかに `git` 文字列があれば弾くため、`bin/sotp git <sub>` の Bash 直叩きは常にブロックされる。`cargo make` task（タスク名に `git` を含まない）から `bin/sotp git ...` を起動すれば、`git` 文字列は `cargo make` のプロセス内に閉じ、フックの走査対象外になる。これが git ロジックを呼び出せる唯一の正規ルートである。

### D5: docker 再現性ゲート・セットアップ・オーケストレーションは cargo make に維持する

以下は `cargo make` を入口として残す。

- **Docker 再現性ゲート**: `ci` / `ci-rust` / `fmt` / `clippy` / `test` / `deny` / `machete` / `check` / `check-layers` / `verify-*`（ただし外部から直接呼ばれる verify ゲートに限る。D7 で削除する参照のない public wrapper `verify-doc-links` は対象外） / `scripts-selftest` 等。これらは `docker compose run --rm tools` による隔離実行が本質であり、再現性ゲートとして要求されている。
- **セットアップ**: `bootstrap` / `build-sotp` / `build-tools`。
- **オーケストレーション**: 複数コマンドを束ねるもの（`track-active-gate` の signal 再生成シーケンス、git ゲート連結など。git 連結の最終形は D6）。

ホスト側 `cargo make` がコンテナ内 `cargo make --allow-private <task>-local` を呼ぶ二重 `cargo make` 構造は、本 ADR のスコープでは現状維持とする（簡素化は別途）。

### D6: bin/sotp make を完全廃止し、git オーケストレーションは cargo make script へ戻す

`bin/sotp make` サブコマンドを完全に廃止する（D2）。`make` が担っていたものは入口別に次へ移す。

- **非 git のパススルー** → `cargo make` を介さず `bin/sotp <native>` 直叩き（D3）。
- **git の単純ラッパー** → `cargo make` task が `command`/`args` で直接 `bin/sotp git <sub>` を呼ぶ（D4）。
- **複数ステップの git オーケストレーション**（`commit` / `track-commit-message`）→ `cargo make` の script task として、各 native subcommand の連結で再構成する。

`track-commit-message` の再構成例（順に実行し、失敗で停止）:

1. `bin/sotp git add-all`（ステージング）
2. `cargo make ci`（CI。同一ツール内なので循環ではない）
3. `bin/sotp review check-approved`（レビューゲート）
4. `bin/sotp dry check-approved`（DRY ゲート）
5. `bin/sotp git commit-from-file tmp/track-commit/commit-message.txt --cleanup`（ブランチガード内蔵のコミット）
6. `bin/sotp track set-commit-hash`（`.commit_hash` の永続化。native subcommand 化が要る）

これにより、現状 `bin/sotp make commit` / `make_track_commit_message` が内部で `cargo make ci` をサブプロセス起動して生んでいた `cargo make → bin/sotp → cargo make` の逆流（問題 3）が解消する。CI 実行は `cargo make ci` のまま同一ツール内に閉じる。可変入力（コミットメッセージ・ノート本文）は現行どおりファイル経由（`tmp/track-commit/*.txt`）に限定し、引数の shell 展開を避ける。

**付随ロジックの移植先（確定）**: cargo make script には繋ぎこみ（bin/sotp native subcommand と cargo make の順次呼び出し）だけを置き、ロジックは `bin/sotp` native subcommand に内包する（原則 2）。`make_track_commit_message` に現在埋め込まれている付随ロジックの移植先を以下に確定する（impl-plan へ先送りしない）。

- **類似度しきい値の解決** → `bin/sotp dry check-approved` が設定ファイル（D9）から自己解決する。cargo make script は `--threshold` を渡さない。現在の `resolve_commit_dry_threshold`（`track/items/<id>/dry-check.json` の最新レコードからしきい値を引き継ぐロジック）は D9 により廃止し、`.harness/config/dry-check.json` の設定ファイル読み込みへ置き換える（単体テストも設定読み込み側へ移す）。
- **`.commit_hash` 永続化と失敗時の復旧ヒント** → `bin/sotp track set-commit-hash`（native subcommand として新設）に内包する。現在 composition 層 (`persist_commit_hash_for_track`) にあるロジックを track サブコマンド配下へ移す。
- **CI ログのキャプチャと失敗時末尾表示** → 廃止する。`cargo make ci` の標準出力をそのまま見せ、失敗時は script が停止してユーザーが CI 出力を直接読む。track-commit-message 固有のログ整形は持たない（ログ整形を残すなら `cargo make ci` task 自身の責務であり、track-commit-message のロジックではない）。

結果として cargo make script に残るのは「6 ステップ（bin/sotp native 5 つ + cargo make ci 1 つ）を順に呼び、失敗で止める」だけになり、計算・条件分岐・データ処理は一切 cargo make 側に置かない。

### D7: orphan / ghost タスクを整理する

参照のないタスクは削除する。

- `test-nightly`（参照ゼロ）: 削除する。
- `track-baseline-capture`（cargo make wrapper が参照ゼロ）: 削除する（D3 で `bin/sotp track baseline-capture` 直叩きへ移行）。
- `verify-doc-links`（public compose wrapper が単独利用されず、`ci-local` の内部依存としてのみ機能）: public wrapper を削除し、内部依存（`ci-local` の dependency）だけ残す。
- `hooks-selftest`: `.claude/rules/07-dev-environment.md` が `cargo make hooks-selftest` を参照するが `Makefile.toml` に実体がない（ghost 参照）。hook は `bin/sotp hook dispatch` の Rust 実装で、その回帰テストは Rust ユニットテスト（`cargo make test` でカバー）に含まれる。専用タスクは新設せず、`07-dev-environment.md` の ghost 参照を削除する（D8 のドキュメント同期に含める）。

### D8: 削除に伴う波及を同期する

タスク削除・改名は、入口文字列を保持している全消費者と同期する。対象は以下。

- **Rust 検証ロジック**: `libs/infrastructure/src/verify/orchestra.rs` の許可リスト（`cargo make <task>` 名の静的配列）、`doc_patterns.rs`（ドキュメントに特定 `cargo make` 名を要求するチェック）、`view_freshness.rs` / `convention_docs.rs` のエラーメッセージ、`libs/domain/src/guard/policy.rs` のブロックメッセージ、`libs/domain/src/skill_compliance/mod.rs` のリマインダー。
- **権限**: `.claude/settings.json` の `permissions.allow`（`Bash(cargo make ...)` エントリを、native 直叩きへ移行したものは `Bash(bin/sotp ...)` 相当へ）。フック制約（D4）と矛盾しないことを確認する。
- **スラッシュコマンド / スキル / エージェント定義**: `.claude/commands/**` / `.claude/skills/**` / `.claude/agents/**` が呼ぶ `cargo make` 入口を、移行先に書き換える。
- **運用ドキュメント**: `DEVELOPER_AI_WORKFLOW.md` / `track/workflow.md` / `.claude/rules/07-dev-environment.md` / `CLAUDE.md` / `LOCAL_DEVELOPMENT.md` 等。
- **テスト**: `scripts/test_make_wrappers.py` の該当ケース。

検証ロジックと実体が乖離すると CI（`verify-orchestra` / `verify-arch-docs` 等）が落ちるため、削除と検証ロジック更新は同一変更内で行う。

### D9: DRY しきい値を専用設定ファイルで管理する

現在 DRY しきい値のデフォルト `0.85` は 3 箇所にハードコード散在している（`apps/cli/src/commands/dry.rs` の `dry write` と `dry check-approved` の `--threshold` 既定、`apps/cli-composition/src/make.rs` の `DEFAULT_THRESHOLD`）。これを専用設定ファイルへ集約し、明示的に管理できるようにする。

- **設定ファイル**: `.harness/config/dry-check.json`（新設）。JSON で `schema_version` + `threshold`（将来 top-k 等の検出パラメータも拡張できる構造）。`agent-profiles.json` の provider / model ルーティングとは分離し、検出パラメータ専用とする。
- **読み込み**: native subcommand（`bin/sotp dry write` / `bin/sotp dry check-approved`）が設定ファイルを読む。読み込みロジックは infrastructure 層に置く（`AgentProfiles::load` と同様のパターン）。原則 2 のとおり cargo make 側には出さない。
- **優先順位**: `--threshold`（CLI 明示）> 設定ファイルの `threshold`。CLI 引数は実験用に残し上書きを許す。`--threshold` は `Option`（既定値なし）にし、未指定のときだけ設定ファイルを読む。
- **fail-closed（フォールバックしない）**: `--threshold` 未指定で設定ファイルが**読めない**（不在 / パースエラー / I/O エラー）ときは、暗黙の既定値（`0.85`）にフォールバックせず**エラーで停止する**。しきい値が決まらないまま DRY ゲートを通すのを防ぐ。`0.85` という初期値は設定ファイル（`.harness/config/dry-check.json`、テンプレートとしてリポジトリにコミット）の中だけに置き、コード側にフォールバック定数を持たない。これにより現在 3 箇所に散在する 0.85 ハードコードは消える。
- **per-track 検証記録からの履歴引き継ぎの廃止**: 現在 commit gate が使う `resolve_commit_dry_threshold`（`track/items/<id>/dry-check.json` の最新レコードの threshold を引き継ぐロジック）は廃止する。しきい値の SSoT が `.harness/config/dry-check.json` になるため、`dry write` も `dry check-approved` も同じ設定値を読み、引き継ぎは不要になる。`track/items/<id>/dry-check.json` の `threshold` フィールドは「チェック時に使った値の記録（履歴）」として残すが、次回のしきい値決定には使わない。

## Rejected Alternatives

- **全タスクを native 直叩きにし、フック側を緩める**: git 系も `bin/sotp git ...` 直叩きに統一し、`block-direct-git-ops` の `command_contains_git` を「`bin/sotp` 配下は許可」へ緩める案。却下理由: フックの fail-closed な単純さ（git 文字列を含めば一律ブロック）が安全性の要であり、`bin/sotp` を例外化すると `bin/sotp` 名を騙る回避ベクタやネスト検知の穴を生む。git 入口を `cargo make` に保つ方が攻撃面が小さい（D4）。
- **make ラッパーをすべて維持し、tools-daemon だけ消す**: 最小変更案。却下理由: 3 層パススルーと死蔵コードと循環が残り、棚卸しの主目的（不要ラップの除去）を達成できない。
- **オーケストレーションも含めて cargo make を全廃**: すべてを `bin/sotp` native に寄せる案。却下理由: Docker 再現性ゲートは compose による隔離が本質で、Rust 側に Docker オーケストレーションを持つのは責務違反。git 入口はフック制約で `cargo make` が必要。
- **make サブコマンドを git 系のために縮小して残す**（D6 の代替）: `bin/sotp make` を git オーケストレーション専用に縮小する案。却下理由: git ラッパーは `cargo make` の `command`/`args` から直接 `bin/sotp git` を呼べば足り、オーケストレーションは cargo make script で組めるため、`make` 名前空間を残す必要がない。完全廃止の方が層が減る。
- **サイクル解消に sotp native CI runner を持たせる**（D6 の代替）: `bin/sotp` 側で CI を直接実行し `cargo make ci` を呼ばない案。却下理由: CI の構成（Docker 隔離・タスク列）を Rust 側にも複製することになり二重管理になる。CI は `cargo make ci` のまま、入口を cargo make script に閉じれば循環は同一ツール内で解消する。

## Consequences

### Positive

- 大半のワークフロータスクが `cargo make X` → `bin/sotp <native>` の 1 層に減り、呼び出し経路が追いやすくなる。
- `bin/sotp make` サブコマンド（clap 層 + composition 層の二重実装）が丸ごと消え、3 層パススルーと死蔵コードが同時になくなる。
- `Rust → cargo make ci` の循環が解消し、依存方向が一方向になる（D6）。
- 未使用の常駐デーモンとその維持面（タスク・許可・テスト・ドキュメント）が消える。
- git 入口を `cargo make` に保つことで、フックの fail-closed な安全性を維持したまま不要ラップだけを削れる。
- DRY しきい値を設定ファイルの SSoT に集約し、設定欠落時は fail-closed でエラーにするため、暗黙の既定値で DRY ゲートをすり抜けることがなくなる（D9）。

### Negative

- 波及が広い。スラッシュコマンド・スキル・エージェント定義・運用ドキュメント・Rust 検証ロジック・権限リストを一括で同期する必要があり、変更が大きい（D8）。漏れると CI が落ちる。
- native 直叩きへ移行する箇所では、これまで `bin/sotp make` が補っていた定型引数（`--items-dir track/items` 等）を native subcommand の既定値・引数処理へ寄せる作業が要る。
- `make_track_commit_message` の付随ロジックを native subcommand へ移す実装が要る（しきい値解決を `bin/sotp dry check-approved` に内包、`.commit_hash` 永続化を `bin/sotp track set-commit-hash` native subcommand として新設、CI ログ整形は廃止）。これにより cargo make script は計算・条件分岐を持たない純粋な繋ぎこみになる。
- git 系と非 git 系で入口が分かれる（`cargo make` と `bin/sotp` の混在）。線引きの根拠（フック制約）を運用ドキュメントに明記しないと混乱を生む。
- DRY しきい値の設定ファイル（`.harness/config/dry-check.json`、テンプレートにコミット）新設と、native subcommand への読み込みロジック追加が要る（D9）。設定が読めなければ fail-closed でエラーにする（0.85 フォールバックを廃止）。散在する 0.85 ハードコード 3 箇所と `resolve_commit_dry_threshold` を廃止し、`--threshold` は `Option`（既定なし）に変更、しきい値は設定ファイルを SSoT とする。

### Neutral

- ホスト `cargo make` → コンテナ `cargo make --allow-private <task>-local` の二重構造は本 ADR では触れない（再現性ゲートとして現状維持）。

## Reassess When

- `block-direct-git-ops` フックの `git` 文字列検知方式が変わり、`bin/sotp git ...` の直接呼び出しが安全に許可できるようになったとき（D4 / D6 の前提が変わる）。
- native subcommand の体系が再編され、`make` 名前空間との対応が崩れたとき。
- Docker による再現性ゲートの方針（`run --rm` 隔離）が変わったとき（D5 の前提が変わる）。

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/conventions/pre-track-adr-authoring.md` — ADR ライフサイクルと配置ルール
- `Makefile.toml` — 棚卸し対象のタスク定義
- `apps/cli/src/commands/make.rs` / `apps/cli-composition/src/make.rs` — `bin/sotp make` ディスパッチの実装（二重実装の所在）
- `libs/domain/src/guard/policy.rs` — git 操作ブロックフックのポリシー（`command_contains_git` の制約根拠）
- `libs/infrastructure/src/verify/orchestra.rs` — `cargo make` 許可リストの検証（D8 の同期対象）
