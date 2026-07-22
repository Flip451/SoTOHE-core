---
adr_id: 2026-07-16-1438-consumer-scaffold-host-first-makefile
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session-01NfUF2m4L6FLSqApp87wB4j:2026-07-17"
    candidate_selection: "from:[fork-copy-prune, zero-base-rebuild] chose:zero-base-rebuild"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session-01NfUF2m4L6FLSqApp87wB4j:2026-07-17"
    candidate_selection: "from:[keep-passthrough-wrappers, bin-sotp-direct-calls] chose:bin-sotp-direct-calls"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session-01NfUF2m4L6FLSqApp87wB4j:2026-07-17"
    candidate_selection: "from:[docker-first-everywhere, host-first-everywhere, hybrid-consumer-host-first] chose:hybrid-consumer-host-first"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:session-01NfUF2m4L6FLSqApp87wB4j:2026-07-17"
    candidate_selection: "from:[no-pinning, rust-toolchain-toml-plus-preflight, rust-toolchain-toml-plus-pinned-install] chose:rust-toolchain-toml-plus-pinned-install"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:session-01NfUF2m4L6FLSqApp87wB4j:2026-07-17"
    candidate_selection: "from:[keep-3-duplicate-tasks, deduplicate-to-1] chose:deduplicate-to-1"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:session-01NfUF2m4L6FLSqApp87wB4j:2026-07-17"
    status: proposed
  - id: D7
    user_decision_ref: "chat_segment:session-01NfUF2m4L6FLSqApp87wB4j:2026-07-17"
    candidate_selection: "from:[commit-binary-to-git, install-sotp-in-ci] chose:install-sotp-in-ci"
    status: proposed
  - id: D8
    user_decision_ref: "chat_segment:session-01NfUF2m4L6FLSqApp87wB4j:2026-07-17"
    candidate_selection: "from:[no-docker-variant, standalone-docker-makefile, override-patch-with-shared-base, symmetric-env-files-extend-switch] chose:symmetric-env-files-extend-switch"
    status: proposed
---
# 配布 scaffold の実行系を host-first に刷新し、Makefile をワークフロー参照タスクのみでゼロベース再構成する

## Context

テンプレート配布物の `overlay/Makefile.toml` は、ソースリポジトリの `Makefile.toml` を手動で複製・編集したフォークコピーになっている（101 タスク・744 行、ソースの約 88%）。同期を強制する機構はなく、ソース側の問題（同一コマンドを 3 回実行する verify タスク、`ci-local` と `ci-container` の依存列の完全重複、開発者個人の環境依存である `asdf` 参照、どこからも設定されない `WORKER_ID` 変数）がそのまま配布物に伝播している。

配布物のうちワークフローを規定する面から `cargo make` タスクへの参照を全数抽出して分類した。参照面の内訳は、workflow logic SSoT（`.harness/workflows/`）、capability 仕様 SSoT（`.harness/capabilities/` — implementer / reviewer 系 subagent への指示に含まれるタスク呼び出し）、briefing 面（`.harness/briefings/`・`.harness/custom/review-prompts/`）、process 強制（`.githooks/`）、配布される CI 定義（`.github/workflows/ci.yml`）、および workflow SSoT を持たない utility command が command-local に定義している呼び出し（provider adapter: `.claude/commands/`・`.claude/skills/` 配下。adapter は workflow logic の SSoT ではなく、SSoT を持つ command の adapter 側参照は SSoT 側と重複するのみ）である。結果:

- ワークフロー定義面から直接参照されるタスク: 26 + sotp バイナリの案内文が参照する 1
- そこからの推移的必要（集約タスクの依存列・スクリプト内呼び出し）: 34
- 推移的に必要だが内容が重複しているタスク: 2（D5 の統合対象）
- 配布文書のみが参照するタスク: 16（+ その実体 6）
- どこからも参照されないタスク: 16

さらに、タスク数の約半分は docker 境界を跨ぐための配管である: 全ゲートの「compose ラッパー + コンテナ内 `-local` 実体」の二重化（約 20 ペア）、compose run と docker exec という 2 つのコンテナ入口に対応する集約タスクの依存列コピペ、bind mount の所有権対策の mkdir、target ディレクトリの環境変数マッピング。ゲートのロジック自体はすでに `bin/sotp` に移管済みであり、残った複雑さの正体は「どの環境で実行するか」の配管に集中している。

また、git 書き込み操作を `cargo make` 経由に限定する根拠とされてきた「Bash コマンド文字列の git キーワード一括走査」は、現行のガード実装には存在しない。現行実装（`libs/domain/src/guard/policy.rs`）は shell を構造化パースし、実効コマンドが `git` の場合のみ書き込み系サブコマンドをブロックする。実際の git 書き込みの強制は process-level の git hooks（`.githooks/reference-transaction` / `pre-push`）と sotp の git_cli 層によるトークン注入に移管済みである（`knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md`）。したがって `bin/sotp git <sub>` の直接呼び出しはガードにブロックされず、単発パススルーのラッパータスクを Makefile に残す機構的必然性はない。

`bin/sotp` は `.gitignore` 対象のため、配布物を採用したリポジトリの CI クローンにはバイナリが存在しない。配布物にはバイナリ transplant（初回）と `install-sotp`（pinned tag からの再取得）の 2 経路がすでに存在する。`rust-toolchain.toml` による toolchain 固定は未導入。

## Decision

対象は配布 scaffold（`overlay/Makefile.toml` とその付随配布面: 配布される CI 定義・bootstrap・toolchain 固定・配布文書・permissions allowlist）に限る。SoTOHE 本体の `Makefile.toml` の実行系（docker-first、コンテナ内ビルド、`ci-container` 構成）は本 ADR の対象外とし、別 ADR で扱う。

### D1: 配布 Makefile はワークフロー参照タスクのみでゼロベース再構成する

既存ファイルからの剪定ではなく、ゼロベースで積み上げる。採録基準は「workflow logic SSoT（`.harness/workflows/`）・capability 仕様 SSoT（`.harness/capabilities/`）・briefing 面（`.harness/briefings/`・`.harness/custom/review-prompts/`）・process 強制（`.githooks/`）・配布される CI 定義・workflow SSoT を持たない utility command の command-local 定義・sotp バイナリの案内文、のいずれかから参照されるタスク + その推移的必要」のみ。配布文書のみが参照するタスク・参照ゼロのタスクは採録せず、文書側を共同更新する。

### D2: Makefile に残すのは「オーケストレーションを組むタスクとその構成員」のみとし、単発パススルーはワークフローが bin/sotp を直接呼ぶ

Makefile タスクとして残す機構的基準を次の 2 つに限定する:

1. Makefile 上で複数ステップのオーケストレーションを組んでいるタスク（コミットゲート連鎖、signal 再生成連鎖、レビュー前ゲートの依存連鎖、bootstrap）
2. そのオーケストレーションに組み込まれているタスク（CI 集約の依存列の構成員、オーケストレーションが `cargo make` で呼ぶタスク）

単発で `bin/sotp <sub>` を呼ぶだけのラッパー（staging・sync・ブランチ操作・PR 操作・note 操作など約 10 タスク）は廃止し、ワークフロー定義面が `bin/sotp git/pr/track <sub>` を直接呼ぶ形に共同更新する。Context に記したとおり、現行ガード実装はこの直接呼び出しをブロックしない。

### D3: 配布 scaffold は host-first（docker 任意）、SoTOHE 本体は docker-first を維持する

配布 scaffold の品質ゲート（fmt チェック・clippy・test・deny・`bin/sotp verify` 群）はホスト toolchain で直接実行する。Docker は配布物の前提条件から外れる。SoTOHE 本体は従来どおり docker compose による隔離実行を維持する。

両者の共存はワークフロー面の呼び出し規約で担保する: ワークフロー定義面の呼び出しは「ゲート集約の `cargo make <task>`」と「単発の `bin/sotp <sub>`」の 2 種に正規化し、**同名タスクの実行環境（ホスト直実行か docker 経由か）は各リポジトリの Makefile が決める**。ワークフロー文書は両リポジトリで共有されたまま変更不要になる。

### D4: host-first の再現性は rust-toolchain.toml と補助ツールの pinned インストールで担保する

docker が担っていた toolchain 固定の代替として:

- `rust-toolchain.toml` を配布物に含め、rustc / clippy / rustfmt のバージョンを固定する（rustup が自動強制する）
- cargo-nextest / cargo-deny 等の補助ツールは bootstrap が pinned バージョンを `--locked` でインストールし、CI（D7）も同じ pin を使う。ローカルの乖離は merge gate である CI で顕在化する
- 専用の preflight 検査機構は設けない。ドリフト起因のゲート問題が実際に観測された場合に再評価する

### D5: 同一コマンドを 3 回実行する verify タスクは 1 本に統合する

`bin/sotp track views validate` を実行する 3 タスク（plan-progress / track-metadata / track-registry 名義）を、ワークフローが参照している名前（track-metadata 名義）の 1 本に統合し、説明文を「track views 全体（metadata.json / plan.md / registry.md）の整合検証」に是正する。

### D6: 開発者個人の環境依存と未使用の変数規約を配布物から排除する

- レビュー系タスクの `asdf which codex` 参照を廃し、`CODEX_BIN` が未設定の場合は `command -v codex` のみにフォールバックする
- どこからも設定されない `WORKER_ID` 分岐を廃し、並列分離は既存文書と compose 定義が使う `CARGO_TARGET_DIR_RELATIVE` に一本化する。対象の cache ディレクトリ準備タスクは、D8 の構成では docker 環境ファイル側にのみ置く（host 実行では準備自体が不要なため）

### D7: 配布 CI はホスト実行に書き換え、bin/sotp は CI 内で install-sotp により取得する

配布される CI 定義はコンテナ（docker exec）前提をやめ、ホストランナー上で D4 の固定 toolchain を用いて同じゲート集約を実行する形に書き換える。`bin/sotp` は gitignore 対象でクローンに存在しないため、CI 内では `install-sotp`（pinned tag からの取得）で調達する。バイナリの git コミットは採らない。

ビルドキャッシュについて: ローカルは `target/` の永続と incremental compilation で足りる（docker レイヤーキャッシュ用の cargo-chef はホスト実行では不要）。CI は依存キャッシュまたは sccache（docker 非依存で動作し、CI キャッシュをバックエンドにできる）で担保する。なお CI での `install-sotp` は pinned tag からの sotp 本体のソースビルド（protoc と重量級の依存を含む）であり、毎回実行すると CI 時間を支配するため、取得済みバイナリ（`.cargo-install` 配下）を pinned tag をキーに CI キャッシュへ載せることを事実上必須とする。具体形は実装フェーズで確定する。

### D8: 環境依存ゲートを環境ファイルに分離し、docker 実行は `Makefile.toml` の extend 参照先の書き換えで選択する

配布 Makefile を「共通部」と「環境依存部」に分離する:

- `Makefile.toml` — 本体。オーケストレーション・`bin/sotp` 系ゲート・CI 集約の依存列などの環境非依存タスクをすべて持ち、環境依存部を取り込む `extend` を 1 行だけ置く（既定の参照先は `Makefile.host.toml`）
- `Makefile.host.toml` — cargo toolchain 系ゲート（fmt チェック・clippy・test・test-doc・deny）のホスト直実行定義
- `Makefile.docker.toml` — 同名ゲートの docker 実行定義（compose から cargo を直接呼ぶ。コンテナ内に二段目の cargo make 層は設けない）。`Makefile.host.toml` と対称な peer であり、相互に依存しない

docker への切替は `Makefile.toml` の `extend` 参照先を `Makefile.docker.toml` に書き換える 1 行編集、復帰はその逆とする。cargo-make の `extend` は読み込んだ全ファイルを単一のタスク表に合成してから依存解決するため、本体側の集約の依存列が環境ファイル側の同名ゲートを解決できることは実測で確認済み。共通部と環境部でタスク名が重ならないため、上書き規則にも依存しない。

`bin/sotp` 系ゲートが共通部に住む理由: 配布物の `bin/sotp` はホスト向けバイナリであり、コンテナ内での実行可能性を保証できないため、docker 選択時もホスト実行のままとする。タスク名の契約（D3）は不変で、切替時もワークフロー面・capability 面の変更は不要。

## Rejected Alternatives

- **フォークコピーの剪定（現行構成の維持 + 削減のみ）**: 手動同期の構造が残り、ソース側の腐敗が今後も伝播する。ゼロベースで積み上げないと採録根拠が説明できない。
- **全面 docker 維持（配布物も docker-first のまま）**: タスク数の約半分を占める境界配管が配布物に残り続け、Docker が採用の前提条件であり続ける。薄い scaffold という配布方針と衝突する。
- **全面 host 化（SoTOHE 本体も host-first に）**: 本体の実行系変更は影響範囲が大きく、別の検討（本体 Makefile の棚卸し）と不可分。本 ADR では判断しない。
- **単発パススルーラッパーの維持**: 維持の根拠だった「git キーワード走査によるブロック」が現行実装に存在しないことを確認済み。参照のためだけに二重の呼び出し層を残す理由がない。
- **bin/sotp バイナリの git コミット（CI 調達の代替）**: バイナリをリポジトリに含めるとサイズ・プラットフォーム差・更新手順の問題を持ち込む。pinned tag からの `install-sotp` で再現的に調達できる。
- **完全独立の docker 変種 Makefile（丸ごと複製の同梱）**: フォークコピー問題を配布物の内部で再発させる。同期義務が全タスクに及ぶのに対し、環境依存ゲートだけを環境ファイルに分離すれば同期面は cargo 系ゲート数本に限られる。
- **非対称 patch 構成（docker 変種が host 定義を `extend` で継承し、運用ファイルは空にする）**: 本体 Makefile が中身のない 1 行ファイルになり、host と docker の関係も非対称になる。共通部を本体に置き環境ファイルを対称な peer にするほうが、切替の意味が構成から読み取れる。

## Consequences

- Good: 配布 Makefile が 101 タスクから 35 前後のタスク規模に縮む（約 6 割削減）。ラッパー/`-local` 二重化・コンテナ集約の依存列コピペが配布物から消え、cache ディレクトリ準備・target ディレクトリの環境変数配管は既定の host 実行経路から消える（docker 環境ファイル側には残る）。
- Good: Docker が配布物の前提条件から外れ、採用の初期コストが下がる。
- Good: バイナリ可用性の問題（どの環境に実行可能な `bin/sotp` があるか）が「CI だけの問題」に縮退し、`install-sotp` で解決が閉じる。
- Bad: ワークフロー定義面の `cargo make` 参照（約 30 箇所）を `bin/sotp` 直接呼び出しへ書き換える共同更新が必要。配布文書・permissions allowlist も同時更新が要る。
- Bad: ホスト toolchain 差による偽のゲート失敗のリスクが残る（D4 の pin で緩和し、必要なら D8 の docker 環境ファイルへ切替できる）。
- Good: docker 隔離を選ぶ利用者は `extend` 参照先の書き換え 1 行で移行でき、ワークフロー面は不変のまま。
- Bad: CI の初回実行とキャッシュ喪失時は `install-sotp` のソースビルドで所要時間が伸びる。
- Bad: 2 つの環境ファイル（host / docker）間で同名ゲートの整合を保つ同期義務が残る（対象は cargo 系ゲート数本のみ）。
- Neutral: ソースリポジトリと配布物で同名ゲートの実行環境が異なる二形態を許容する。ワークフロー面の呼び出し規約（D3）がその境界を吸収する。

## Reassess When

- 配布物の利用側で docker 隔離が必要になったとき（ネイティブ依存の追加、ホスト toolchain 固定で防げない再現性問題の実証）。
- 補助ツールのバージョンドリフトに起因するゲート問題が観測されたとき（preflight 検査の導入を再検討する）。
- sotp の配布形態が変わったとき（プリビルトバイナリ配布、クレート公開など）。
- SoTOHE 本体側の Makefile 実行系を扱う ADR が本体の構成を変えたとき（ワークフロー面の呼び出し規約の再確認が必要になる）。

## Related

- `knowledge/adr/2026-06-05-1535-cargo-make-teardown.md` — cargo make の役割整理。本体側の docker 再現性ゲート維持（D5）はこの ADR の決定であり、本 ADR は配布物側のみを変更する
- `knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md` — git 書き込み強制の process-level 移管。D2 の機構的根拠
- `knowledge/adr/2026-07-06-1717-template-extraction-boundary.md` — 配布境界の定義
- `knowledge/adr/2026-07-08-0541-template-export-sotp-binary-transplant.md` — バイナリ transplant。D7 の初回経路
- `Makefile.toml` / `overlay/Makefile.toml` — 対象ファイル
