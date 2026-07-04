---
adr_id: 2026-07-04-0155-git-sync-dedicated-command
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session_012jadeg7kj8gmuSLLYhvuiD:2026-07-04"
    candidate_selection: "from:[D1,A,B,C,D] chose:D1"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session_012jadeg7kj8gmuSLLYhvuiD:2026-07-04"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session_012jadeg7kj8gmuSLLYhvuiD:2026-07-04"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:session_012jadeg7kj8gmuSLLYhvuiD:2026-07-04"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:session_012jadeg7kj8gmuSLLYhvuiD:2026-07-04"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:session_012jadeg7kj8gmuSLLYhvuiD:2026-07-04"
    status: proposed
  - id: D7
    user_decision_ref: "chat_segment:session_012jadeg7kj8gmuSLLYhvuiD:2026-07-04"
    status: proposed
  - id: D8
    user_decision_ref: "chat_segment:session_012jadeg7kj8gmuSLLYhvuiD:2026-07-04"
    status: proposed
  - id: D9
    user_decision_ref: "chat_segment:session_012jadeg7kj8gmuSLLYhvuiD:2026-07-04"
    status: proposed
---
# remote sync 専用コマンドの新設と git 操作の hexagonal 是正 — switch と pull の分離、意味論 port への全面移管

## Context

branch-strategy の config-driven 化で `cargo make track-switch-main`（実体は `bin/sotp git switch-and-pull main`。任意のブランチ上から動作する generic な switch + pull）が `cargo make track-switch-base`（実体は `bin/sotp track switch-base`）に置き換えられた。

`track switch-base` は現在の git ブランチ名から active track を解決し、その track の `metadata.json#branch_strategy_snapshot` から base branch を読む（CN-02: post-init 操作は global config を再読しない）。このため track ブランチ上でしか動作せず、base branch（develop）上で実行すると「not on a track branch」エラーになる。

一方、素の `git pull` は `.githooks/reference-transaction` → `sotp hook dispatch git-ref-update` が sotp の git_cli layer が注入する env token を持たない ref 更新を一律拒否するため、ターミナルからの手動実行も含めてブロックされる。

結果として「base branch 上で remote の最新を取り込む」動線の正規経路が消失し、唯一の回避策が `bin/sotp git switch-and-pull develop` の直叩き（ブランチ名の手動指定）になっていた。

根本原因は switch と pull が単一コマンドに密結合していることにある。「switch 先の解決」（track snapshot / global config のどちらを読むか）という関心が、「現在ブランチを remote と同期する」だけの単純な操作にまで持ち込まれ、本来引数も設定も不要な操作を track 文脈に依存させていた。

さらに実装調査により、問題は switch と pull の複合に留まらないことが判明した。git workflow 操作（stage / commit / note / switch-and-pull / unstage）には hexagonal 的に正しい既存 chain — usecase port `GitWorkflowService`（`libs/usecase/src/git_workflow.rs`）→ infrastructure adapter `FsGitWorkflowAdapter`（`libs/infrastructure/src/git_cli/workflow_adapter.rs`）→ delivery `GitDriver`（`apps/cli-driver/src/git.rs`）、配線 `GitCompositionRoot::git_driver()` — が完備されているにも関わらず、production caller はゼロで dead code になっている。実際に生きている経路は `GitCompositionRoot` の inline method 群（`apps/cli-composition/src/git.rs`）で、generic passthrough（`GitRepository::status` / `output`）越しに git コマンド文字列とポリシーを composition root に直埋めしており、commit の fail-closed track-branch guard は両系統にほぼ逐語で重複している。同種の直埋めは track branch 操作（`switch -c` / `switch` / `mv`）、pr 系（`fetch` / `show` / `rev-parse`）、review_v2 系（`rev-parse`）、さらに delivery 層 `apps/cli` の branch_ops（`rev-parse --verify`）にも及ぶ。

## Decision

### D1: 現在ブランチ pull 専用の `bin/sotp git sync` を新設する

現在ブランチを upstream から pull するだけの subcommand を新設する。switch は行わず、ブランチ引数も取らない。sync 対象は常に「現在ブランチ」なので branch 解決も config 読み取りも不要になり、CN-02（snapshot 優先、global config 再読禁止）と構造的に衝突しない。

### D2: fast-forward only + 明確なエラー

pull は fast-forward only とする。divergence（ff 不可）時は明確なエラーで停止し、手動 merge はユーザー責務とする（merge / rebase を AI ハードブロックとする既存の enforcement 方針と整合）。upstream 未設定時も明確なエラーとし、graceful skip にしない。

### D3: 独立 subcommand + 薄い wrapper

ロジックは CLI 側（`bin/sotp git sync`）に実装し、Makefile 側は薄い wrapper task のみとする。ブランチ名・条件分岐などのロジックを Makefile に書かない。

### D4: `track-switch-base` は存続、`git switch-and-pull` は廃止

`track-switch-base` は「track ブランチから base branch に戻る」動線として存続させ、内部を branch switch + sync の合成に refactor する（外部挙動は不変）。generic な `git switch-and-pull` subcommand は廃止する（sync と既存の branch switch 系コマンドで代替可能になるため）。

### D5: cli 系レイヤーの git 操作を infrastructure の意味論 port に全面移管し、fail-closed error を型化する

cli 系レイヤー（cli / cli_composition / cli_driver）に埋め込まれた git コマンド文字列と判定ロジックを、infrastructure（`libs/infrastructure/src/git_cli/`）の意味論的 method 群へ全面移管する。write 系（stage / commit / note / branch create / branch switch / mv / fetch）と read 系（`rev-parse` 系 / `show`）の両方を対象とする。cli_composition 側から git command 文字列と ff-only 意味論が漏れ出ている現状（`apps/cli-composition/src/git.rs::git_switch_and_pull_impl` に `repo.status(&["pull", "--ff-only"])` が埋め込まれている状態）を hexagonal 的に是正する。

sync path はその旗艦事例とする。現在ブランチを upstream から fast-forward only で pull する infrastructure adapter primitive（例: `SystemGitRepo::sync_current_branch(&self) -> Result<(), SyncError>`）を新設し、CN-02 の 3 種 fail-closed failure modes（upstream 未設定 / non-fast-forward / worktree unresolved）を `SyncError` enum の variant として infrastructure boundary で型化する。この method は git process I/O と失敗分類を所有する下位 primitive であり、command-facing の公開契約ではない。他の操作の失敗も同様に infrastructure 側で意味論 error として分類し、上位層は自層の error 型への変換のみを行い、error 判別ロジックを持たない。

D1 の `bin/sotp git sync` CLI 実装と D4 の `track switch-base` 内部 refactor は、いずれもこの typed method を経由する。

### D6: 死んでいる hexagonal chain（`GitWorkflowService` 系）を唯一の live chain に統合する

git workflow 操作の二重実装を解消する。usecase port `GitWorkflowService` → infrastructure adapter `FsGitWorkflowAdapter` → delivery `GitDriver` の既存 chain を正とし、`GitCompositionRoot` の inline git method 群（Chain B）は削除する。`apps/cli` の git subcommand dispatch は配線済み `GitDriver` を経由する。

両系統にほぼ逐語で重複している commit の fail-closed track-branch guard は、単一箇所に集約する。

command-facing の公開契約は usecase port `GitWorkflowService` に置く。`GitWorkflowService::sync_current_branch() -> Result<(), GitWorkflowError>` を `bin/sotp git sync` の唯一の service API とし、infrastructure adapter `FsGitWorkflowAdapter` が D5 の `SystemGitRepo::sync_current_branch` / `SyncError` を `GitWorkflowError` へ写像する。`apps/cli` は parse + dispatch、`apps/cli-driver` は service 呼び出し + presentation 変換、`apps/cli-composition` は `GitDriver` への wiring のみを担う。

sync 新設（D1）と switch / pull 分離（D4）は、この port の method 再編として実施する: `switch_and_pull` を port trait から削除し、sync（現在ブランチ pull 専用）と branch switch（既存の branch 操作系）を分離した method として整備する。`track switch-base` は usecase 側の orchestration として「snapshot から base branch を解決 → branch switch service → sync service」を合成し、composition root にはこの手順を置かない。

### D7: generic passthrough を封じ、再発を compile-time で防止する

`GitRepository` trait 自体を削除し、`status` / `output` の generic passthrough を `SystemGitRepo` の `pub(crate)` inherent method に移す（`pub trait` の method は言語仕様上 method 単位で可視性を落とせないため、trait 経由の公開を撤去することで達成する）。既存の意味論 method（`current_branch` / `push_branch` / `index_tree_hash` / `stage_all_excluding` + 新設 `sync_current_branch` 等）は `SystemGitRepo` の `pub` inherent method として残す。crate 外から観測できる意味論 method の集合は変えないが、任意の git コマンド文字列を注入する generic passthrough は crate 内側だけの実装用ハッチとなり、上位層に露出しない。

polymorphism が要る境界（domain / usecase テストの mock 差し替え等）は既存の `WorktreeReader` / `BranchReaderPort` trait でカバー済みで、`GitRepository` trait 削除による能力損失はない。

read 系の直埋め call site（`rev-parse` 系 / `show` / `fetch`。cli_composition の pr / review_v2 / track 系、および delivery 層 `apps/cli` の branch_ops）は意味論 method（branch 存在確認、HEAD 解決、ref 上のファイル読み出し等。シグネチャの詳細は型設計フェーズで確定する）への置き換えで解消する。

### D8: git 関与フローの orchestration を usecase interactor へ移管し、composition root は wiring に限定する

D5–D7 で git 意味論を infrastructure に降ろしても、意味論 method を「呼ぶ側の手順」— track branch create の fail-closed 検証シーケンス、track archive の git mv + logs 移動の合成、pr 系の fetch → 解決 → 参照のフロー、review_v2 の hash 解決フロー — が composition root に残れば、composition root は orchestration を持ち続ける。git を触るフローについては、この手順部分を usecase 層の interactor（既存 `GitWorkflowInteractor` と同型の port + interactor 構造）へ移管し、composition root は adapter の構築と注入（wiring / DI）だけを行う。

適用範囲は git 関与フローに限定する（本 track の census で特定した cli_composition の track branch ops / archive / pr / review_v2 系）。git を触らないフローの composition root 純化は本 ADR の対象外。

### D9: cli 層は parse + dispatch に限定し、infrastructure 直参照を持たない

`apps/cli` の command module は引数 parse と dispatch のみを持つ thin bin とし、infrastructure の型・関数を直接参照しない。既存の cli → usecase 一本化方針を git 系にも徹底し、`apps/cli/src/commands/track/branch_ops.rs` の `rev-parse` 直埋めのような cli 層での git 判定は、usecase 経由（cli_driver の driver または usecase interactor の呼び出し）に置き換える。

## Rejected Alternatives

### A. `track switch-base` への global config fallback

track ブランチ外で実行された場合に `.harness/config/branch-strategy.json` の `base_branch` へ fallback する案。switch + pull 複合という根本原因を温存し、CN-02（snapshot 優先）の適用境界も曖昧化するため却下。

### B. ブランチ名直書きの Makefile wrapper 復活

`git switch-and-pull develop` 相当を Makefile に直書きする案。config-driven 方針に反する（Makefile へのブランチ名ハードコード禁止）ため却下。

### C. 素の `git pull` を hook で許容

reference-transaction hook に pull 例外を設ける案。「ref 更新は sotp 経由のみ」という enforcement モデルに穴を開け、hook 判定も複雑化するため却下。

### D. `switch-and-pull` 直叩き運用の継続

現状の回避策（`bin/sotp git switch-and-pull develop` の直接実行）を正規動線として文書化する案。毎回ブランチ名の手動指定が必要で、switch との複合意味論も残るため却下。

### E. 既存の generic passthrough (`GitRepository::status`) を再利用する

cli_composition 側に `repo.status(&["pull", "--ff-only"])` を直接埋め込む案。他の git wrapper と同じ既存 precedent とは一貫するが、hexagonal port 配置の境界を再度弱め、CN-02 の 3 種 failure modes が cli_composition 側の error string 化（無型）に留まる。sync path を意味論 method 化するという D5 の趣旨とも矛盾するため却下。

### F. composition inline 実装（Chain B）を正として usecase port chain を削除する

統合方向の逆向き案。生きている側（`GitCompositionRoot` の inline method 群）を残し、dead code になっている `GitWorkflowService` / `FsGitWorkflowAdapter` / `GitDriver` を削除すれば実装距離は最短になる。しかし composition root にロジックが定着し、composition root を wiring に限定する既存の責務分離方針と矛盾する。port 側の抽象は既に完備されており、正しい構造を捨てて違反構造を正当化する理由がないため却下。

### G. 段階実施（write-path のみ本 track、read 系は別 track）

差分規模を抑えるため read 系（`rev-parse` / `show` / `fetch`）の意味論化と delivery 層の是正を別 track に先送りする案。generic passthrough の封じ込め（D7）が完了せず、違反の再発経路が残ったまま中途半端な状態が続く。一括是正を選好する判断により却下。

## Consequences

### Positive

- base branch（develop）上での最新化動線が回復する
- branch 解決・config 読み取り不要のプリミティブとなり、CN-02 と無縁になる
- ff-only により暗黙の merge commit 生成を構造的に排除する
- git workflow 操作の二重実装（逐語重複していた track-branch guard を含む）が解消され、dead code だった port chain が唯一の live chain になる
- generic passthrough の `pub(crate)` 化により、上位層への git 意味論漏出が compile-time で再発防止される
- git 関与フローについて「cli = parse + dispatch のみ / composition = wiring のみ」の層責務が実現し、orchestration が usecase interactor に集約される（テスト可能性も usecase 単体で確保できる）

### Negative

- 新 subcommand の実装・テストコストが発生する
- `git switch-and-pull` 廃止・`git sync` 新設に伴い、関連運用文書の**同時更新**が必要になる。実装 track のスコープに以下の更新を含めること（maintainer checklist の「影響レイヤー同時更新」原則）:
  - `knowledge/conventions/branch-strategy.md` — ブランチ操作コマンドの記述（`track-switch-base` の説明更新、sync 動線の追記）
  - `.claude/rules/07-dev-environment.md` — `cargo make` タスク一覧と `bin/sotp` native subcommand 一覧
  - `.claude/commands/track/done.md` / `.harness/workflows/track/done.md` — `/track:done` の base 復帰手順
  - `.codex/rules/default.rules` — wrapper task の prefix_rule（旧 `track-switch-main` の stale 参照除去を含む）
  - `.claude/settings.json` — `permissions.allow` の wrapper エントリ
  - `Makefile.toml` — wrapper task の追加・整理（enforcement 面）
- 是正範囲が cli 系全域（infrastructure / usecase / cli_composition / cli / cli_driver + テスト）に及び、実装・レビューの差分規模が大きい。feature batch が per-scope diff ceiling を超えて分割される可能性が高い（ceiling は advisory）。
- D8 の interactor 移管により usecase 層に track branch ops / archive / pr / review_v2 系の port + interactor が新設され、型・テストの追加コストが発生する。
- infrastructure adapter primitive の意味論化に伴い、`SystemGitRepo` の inherent impl が追加型（`SyncError` 等の意味論 error）と新 method 群の実装・テストを持つ。さらに `FsGitWorkflowAdapter` が `SyncError` から `GitWorkflowError` への写像を持つ（generic passthrough 案（E）に対して型定義とマッピング層のコストが増える）。

### Neutral

- `track-switch-base` の外部挙動は不変（内部 refactor のみ）

## Reassess When

- cargo make wrapper 層の解体（`bin/sotp` 直叩きへの一本化）が進行したとき（D3 の wrapper 前提が変わる）
- multi-remote 等で sync 対象の解決に config が必要になったとき
- divergence が頻発し、ff-only 運用が実務上回らなくなったとき
- 新しい git 操作を追加するとき（意味論 port method として追加するのが既定。generic passthrough の再公開が必要になった場合は本 ADR を supersede する）
- composition root の wiring 限定（D8）を機械検証する enforcement gate（composition-root purity 検証）を導入するかどうか。違反の再流入が観測されたら別 ADR で検討する
- git を触らないフローにも「composition = wiring only」を拡張するかどうか（D8 の適用範囲拡張トリガ。別 ADR で検討する）

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/conventions/branch-strategy.md` — ブランチ運用の現行規約
