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
---
# remote sync 専用コマンドの新設 — switch と pull の分離

## Context

branch-strategy の config-driven 化で `cargo make track-switch-main`（実体は `bin/sotp git switch-and-pull main`。任意のブランチ上から動作する generic な switch + pull）が `cargo make track-switch-base`（実体は `bin/sotp track switch-base`）に置き換えられた。

`track switch-base` は現在の git ブランチ名から active track を解決し、その track の `metadata.json#branch_strategy_snapshot` から base branch を読む（CN-02: post-init 操作は global config を再読しない）。このため track ブランチ上でしか動作せず、base branch（develop）上で実行すると「not on a track branch」エラーになる。

一方、素の `git pull` は `.githooks/reference-transaction` → `sotp hook dispatch git-ref-update` が sotp の git_cli layer が注入する env token を持たない ref 更新を一律拒否するため、ターミナルからの手動実行も含めてブロックされる。

結果として「base branch 上で remote の最新を取り込む」動線の正規経路が消失し、唯一の回避策が `bin/sotp git switch-and-pull develop` の直叩き（ブランチ名の手動指定）になっていた。

根本原因は switch と pull が単一コマンドに密結合していることにある。「switch 先の解決」（track snapshot / global config のどちらを読むか）という関心が、「現在ブランチを remote と同期する」だけの単純な操作にまで持ち込まれ、本来引数も設定も不要な操作を track 文脈に依存させていた。

## Decision

### D1: 現在ブランチ pull 専用の `bin/sotp git sync` を新設する

現在ブランチを upstream から pull するだけの subcommand を新設する。switch は行わず、ブランチ引数も取らない。sync 対象は常に「現在ブランチ」なので branch 解決も config 読み取りも不要になり、CN-02（snapshot 優先、global config 再読禁止）と構造的に衝突しない。

### D2: fast-forward only + 明確なエラー

pull は fast-forward only とする。divergence（ff 不可）時は明確なエラーで停止し、手動 merge はユーザー責務とする（merge / rebase を AI ハードブロックとする既存の enforcement 方針と整合）。upstream 未設定時も明確なエラーとし、graceful skip にしない。

### D3: 独立 subcommand + 薄い wrapper

ロジックは CLI 側（`bin/sotp git sync`）に実装し、Makefile 側は薄い wrapper task のみとする。ブランチ名・条件分岐などのロジックを Makefile に書かない。

### D4: `track-switch-base` は存続、`git switch-and-pull` は廃止

`track-switch-base` は「track ブランチから base branch に戻る」動線として存続させ、内部を branch switch + sync の合成に refactor する（外部挙動は不変）。generic な `git switch-and-pull` subcommand は廃止する（sync と既存の branch switch 系コマンドで代替可能になるため）。

## Rejected Alternatives

### A. `track switch-base` への global config fallback

track ブランチ外で実行された場合に `.harness/config/branch-strategy.json` の `base_branch` へ fallback する案。switch + pull 複合という根本原因を温存し、CN-02（snapshot 優先）の適用境界も曖昧化するため却下。

### B. ブランチ名直書きの Makefile wrapper 復活

`git switch-and-pull develop` 相当を Makefile に直書きする案。config-driven 方針に反する（Makefile へのブランチ名ハードコード禁止）ため却下。

### C. 素の `git pull` を hook で許容

reference-transaction hook に pull 例外を設ける案。「ref 更新は sotp 経由のみ」という enforcement モデルに穴を開け、hook 判定も複雑化するため却下。

### D. `switch-and-pull` 直叩き運用の継続

現状の回避策（`bin/sotp git switch-and-pull develop` の直接実行）を正規動線として文書化する案。毎回ブランチ名の手動指定が必要で、switch との複合意味論も残るため却下。

## Consequences

### Positive

- base branch（develop）上での最新化動線が回復する
- branch 解決・config 読み取り不要のプリミティブとなり、CN-02 と無縁になる
- ff-only により暗黙の merge commit 生成を構造的に排除する

### Negative

- 新 subcommand の実装・テストコストが発生する
- `git switch-and-pull` 廃止・`git sync` 新設に伴い、関連運用文書の**同時更新**が必要になる。実装 track のスコープに以下の更新を含めること（maintainer checklist の「影響レイヤー同時更新」原則）:
  - `knowledge/conventions/branch-strategy.md` — ブランチ操作コマンドの記述（`track-switch-base` の説明更新、sync 動線の追記）
  - `.claude/rules/07-dev-environment.md` — `cargo make` タスク一覧と `bin/sotp` native subcommand 一覧
  - `.claude/commands/track/done.md` / `.harness/workflows/track/done.md` — `/track:done` の base 復帰手順
  - `.codex/rules/default.rules` — wrapper task の prefix_rule（旧 `track-switch-main` の stale 参照除去を含む）
  - `.claude/settings.json` — `permissions.allow` の wrapper エントリ
  - `Makefile.toml` — wrapper task の追加・整理（enforcement 面）

### Neutral

- `track-switch-base` の外部挙動は不変（内部 refactor のみ）

## Reassess When

- cargo make wrapper 層の解体（`bin/sotp` 直叩きへの一本化）が進行したとき（D3 の wrapper 前提が変わる）
- multi-remote 等で sync 対象の解決に config が必要になったとき
- divergence が頻発し、ff-only 運用が実務上回らなくなったとき

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/conventions/branch-strategy.md` — ブランチ運用の現行規約
