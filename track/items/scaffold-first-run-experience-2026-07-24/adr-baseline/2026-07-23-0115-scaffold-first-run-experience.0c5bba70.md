---
adr_id: "2026-07-23-0115-scaffold-first-run-experience"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session_67282137-412e-4cc0-b687-505dd036e653:2026-07-25"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session_67282137-412e-4cc0-b687-505dd036e653:2026-07-25"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session_67282137-412e-4cc0-b687-505dd036e653:2026-07-22"
    status: proposed
---
# scaffold の初期化列を単一タスクへ畳む

## Context

export 直後の scaffold からテンプレート利用プロジェクトを立ち上げる実走（2026-07-21〜22）で、初回体験に 4 つの摩擦源が観測された。

1. export 直後のツリーは git リポジトリではないため、`git init` なしで `cargo make bootstrap` を回すと hooks 設定の段が生の git fatal で停止する。原因の説明も復旧の案内も出ない。
2. 出荷される `.harness/config/branch-strategy.json` が maintainer 運用値（base = develop）のまま include されており、`git init` 直後の新規リポジトリ（main のみ）と必ず不一致になる。track 開始時に解消プロンプトが挟まる。
3. bootstrap のゲート実行で `Cargo.lock` が生成され、初期コミット後の差分として残る。hooks 設置後なので素の commit では拾えない。
4. 出荷 command adapter（plan / adr2pr）が進捗トラッキングに `TaskCreate` を参照しており、ホストにこのツールが無いセッションではワークフローが引っかかる。可用性はセッション依存であることを実機で確認済み。

現状の回避策は利用者手順側で吸収している。
`git init` ⇒ `cargo generate-lockfile` ⇒ `git add -A` ⇒ `git commit` ⇒ base branch 作成 ⇒ `cargo make bootstrap` の 6 手を、利用者が順序どおり打つことが前提になっている。

摩擦ごとに個別の機構（bootstrap への前提チェック、lockfile の同梱、専用 CLI サブコマンド）を足す構成も検討したが、摩擦 1 と 3 は初期化列が手動であることの派生にすぎない。
列そのものを単一タスクへ畳めば、これらは発生機会ごと消える。

## Decision

### D1: 初期化列を overlay の単一 `cargo make` タスクへ畳む

scaffold に `cargo make init` を追加し、export 直後から初回ゲート通過までを 1 コマンドで到達可能にする。
タスクは `git init` ⇒ `cargo generate-lockfile` ⇒ `git add -A` ⇒ 初期コミット ⇒ `cargo make bootstrap` を順に実行する。
`cargo generate-lockfile` を初期コミットより前に置くことで、bootstrap のゲート実行が生む lockfile が未コミット差分として残らない。

タスクは、すでにコミットを持つ git リポジトリで起動された場合、明示エラーで fail-closed に停止する。
このタスクが素の git を実行できるのは hooks 設置前の窓に限られ、2 回目以降の実行では `.githooks` の ref 更新ガードがトランザクションごと停止させるため、利用者には原因不明のブロックだけが残る。
前提チェックはこれを利用者可読なエラーに置き換える。

素の `git add` / `git commit` を禁じる既存のガードレールとの関係を明示する。
禁止の対象は、正規経路を迂回する暗黙の git 操作である。
本タスクは利用者が明示的に選ぶ opt-in コマンドであり、かつ終端の bootstrap が hooks を設置するため、素の git が通る窓は同一コマンドの内側で閉じる。
この 2 条件を満たさない経路に git 操作を組み込むことは、引き続き認めない。

タスクは overlay の Makefile にのみ定義する。
ソースリポジトリは既に git リポジトリであり、本タスクを必要としない。
出荷面にだけ存在するタスクであるため、出荷 Makefile のタスク集合を検査する既存のスモーク検査の対象に含める。

### D2: 出荷 `branch-strategy.json` を overlay へ移し、既定を base = main / merge target = main にする

新規リポジトリの実在ブランチと出荷設定を一致させ、不一致プロンプト自体を消す。
maintainer 運用値（develop ベース）はソースリポジトリ側にのみ残す。

この変更は D1 のタスクからも 1 手を取り除く。
出荷既定が develop のままであれば、タスクは base branch 作成を含めざるを得ず、maintainer の分岐運用を全利用者プロジェクトへ輸出することになる。

### D3: 出荷 command adapter の `TaskCreate` 参照を任意化する

「利用可能なら使う。無ければテキストの進捗報告で代替し、ワークフローは止めない」に改める。
ホスト提供機能への hard 依存を出荷面に置かない。

## Rejected Alternatives

### A. bootstrap が初期化列を暗黙実行する

bootstrap の呼び出しに git リポジトリ作成と初期コミットの副作用を持たせる案。
利用者の git 設定とリポジトリ化の意図に踏み込む暗黙の副作用となり、責任分界に反するため却下した。
明示 opt-in の専用タスク（D1）が正しい形である。

### B. 初期化列を `sotp` のサブコマンドとして実装する

`sotp scaffold init` のような CLI サブコマンドとして提供する案。
実体は `git init` と初期コミットの逐次実行であり、ドメインロジックを持たない。
これを sotp に置くと、層構成の規約に従って値オブジェクト、usecase、port、adapter、driver、composition の配線を通す必要が生じ、シェル手順の逐次実行に対して過大な構造を要求するため却下した。
複合手順であり終端が集約ゲートであることから、`cargo make` タスクが適切な住所である。

### C. 出荷既定を develop ベースのまま維持し、初期化タスク内で develop を作成する

初期化列の手数は変わらないまま、maintainer 側の運用事情を出荷面から輸出し続けることになるため却下した。

### D. overlay に placeholder の `Cargo.lock` を同梱する

export 直後から lockfile 込みで出荷する案。
D1 のタスクが初期コミット前に lockfile を生成するため、摩擦 3 の解決には不要である。
placeholder の依存を更新するたびに lockfile を再生成し、スモークゲートで乖離を検出する保守コストのみが残るため却下した。

### E. bootstrap に git リポジトリ前提チェックを追加する

摩擦 1 を bootstrap 側の前提チェックで解決する案。
D1 により正規経路では非リポジトリ状態の bootstrap に到達しなくなるため、独立の決定としては採らない。
前提チェックの実質は D1 のタスク側に、二重実行の防御として逆向きの条件で残る。

### F. `Cargo.lock` を `.gitignore` に入れる

bin を含むアプリケーションの scaffold では lockfile のコミットが再現性の標準であり、bootstrap のゲート実行の再現性も lockfile に依存するため却下した。

## Consequences

### Positive

- 初回体験の摩擦 4 件が、利用者手順の増加ではなくタスク 1 本と設定既定の修正で解消される。
- 利用者手順書から回避策の逐次列が消え、export 後の手順が単一コマンドになる。
- 新規プロジェクトの実在ブランチと出荷設定が最初から一致する。
- 進捗トラッキング機能を持たないホストでもワークフローが停止しない。

### Negative

- 素の git 操作を含むタスクが出荷面に 1 本増え、その実行可能な窓（hooks 設置前かつ初回のみ）を規約として維持する必要が生じる。
- overlay の Makefile とソースの Makefile でタスク集合が非対称になる。

### Neutral

- 摩擦 1 と 3 は個別の機構ではなく、初期化列の畳み込みによって発生機会ごと解消される。
- ソースリポジトリの base branch 運用は変更しない。

## Reassess When

- リリースブランチ運用（develop 相当）を scaffold 既定に含めたい要望が出たとき。
- 初期化タスクが逐次実行以上の判断（対話、条件分岐、外部状態の照会）を持つようになり、shell では表現が苦しくなったとき。
- ホスト側の進捗トラッキング機能が安定供給されるようになったとき（D3 の再昇格）。

## Related

- `knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md`
- `knowledge/adr/2026-07-06-1717-template-extraction-boundary.md`
- `knowledge/adr/2026-07-23-0117-export-surface-minimization.md`
- `knowledge/conventions/responsibility-boundary.md`
- `overlay/Makefile.toml` — bootstrap 段と新規タスクの定義先
- `.harness/config/{template-boundary,branch-strategy}.json`
- `.claude/commands/track/{plan,adr2pr}.md` — `TaskCreate` 参照元
