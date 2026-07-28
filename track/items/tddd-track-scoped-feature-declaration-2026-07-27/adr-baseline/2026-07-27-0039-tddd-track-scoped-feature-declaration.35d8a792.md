---
adr_id: "2026-07-27-0039-tddd-track-scoped-feature-declaration"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:current-task:2026-07-27:tddd-track-scoped-feature-declaration-hearing"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:current-task:2026-07-27:tddd-track-scoped-feature-declaration-hearing"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:current-task:2026-07-27:tddd-track-scoped-feature-declaration-hearing"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:current-task:2026-07-27:tddd-track-scoped-feature-declaration-hearing"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:current-task:2026-07-27:tddd-track-scoped-feature-declaration-hearing"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:current-task:2026-07-27:tddd-track-scoped-feature-declaration-hearing"
    status: proposed
  - id: D7
    user_decision_ref: "chat_segment:current-task:2026-07-27:tddd-track-scoped-feature-declaration-hearing"
    status: proposed
---
# TDDD chain ③ の rustdoc 抽出を track 単位の feature 宣言に基づかせる

## Context

TDDD chain ③ は、baseline 取得と実測取得の双方が各 layer crate の rustdoc JSON を取得し、その差分から signal を評価する。この rustdoc 呼び出しは cargo feature フラグを一切渡していない。

そのため `#[cfg(feature = "...")]` 配下の public 要素は抽出面に現れない。実装と catalogue 宣言が一致していても「型が見つからない」と評価され、non-blue signal になる。

現状の抽出には feature という概念自体が無い。したがって、ある抽出がどの feature を見るべきかを宣言する場所が無く、baseline 取得と実測取得が同じ feature 集合を観測することを保証する仕組みも無い。

cargo feature は crate ごとに定義が異なり、workspace 全体で一様ではない。単一のフラットな feature リストを全 layer に適用することはできない。

必要なのは、抽出が見るべき feature を宣言する正本と、その宣言を chain ③ の両側が共有する構造である。

## Decision

### D1: track 単位の feature 宣言を専用成果物として新設する

`track/items/<id>/` 配下に専用の宣言成果物を置き、各 layer に対して、その layer の crate を TDDD 抽出時にどの cargo feature 付きでビルドすべきかを対応づける。

この成果物は commit 対象とする。生成 view でも gitignore 対象でもない。review scope は `types` とする。必要な feature が無い layer は空リストを宣言する。宣言そのものは省略しない。

具体的なファイル名と JSON schema は本 ADR では固定せず、実装 track の型設計に委ねる。

<!-- illustrative, non-canonical -->
```
layer ごとに feature 名の配列を持つ map 構造。
domain / usecase / cli_driver のように feature を持たない layer は空配列。
```

### D2: 宣言の書き手は type-designer とし、baseline 取得の直前に書く

宣言成果物は type-designer capability が author する。Phase 2 パイプラインの先頭に置き、baseline 取得の直前に完了させる。

どの型を declare するかを決める主体が、その型が抽出面に存在するためにはどの feature が可視である必要があるかを決める主体でなければならない。両者を分けると、型を宣言する側が抽出条件を知らないまま catalogue を書くことになる。

### D3: rustdoc を叩くコマンドは宣言を入力とし、不在なら fail-closed とする

rustdoc を実際に呼び出す TDDD コマンド、すなわち baseline 取得と実測取得は、この宣言を入力として読む。宣言が存在しない場合は fail-closed で停止する。

既に永続化された JSON を読むだけのコマンドは、この宣言を要求しない。それらは feature 選択に依存せず、要求しても保護にならないまま儀式だけが増える。

chain ③ の突合の両側が同一の宣言を読むことで、baseline と実測の feature 集合が食い違う状態が構造的に発生しなくなる。

経過措置は設けない。宣言の不在を暗黙の空宣言とみなす期間を置かず、導入時点から fail-closed とする。

### D4: 宣言は全 track で必須とする

すべての track が catalogue を持つ。型に変更を加えない track は空の catalogue を宣言し、実装フェーズで型に変更が無いことを検証する。

したがって feature 宣言も全 track で必須であり、型に触れない track を免除しない。免除条件を設けると、catalogue が常在するという不変条件と宣言の常在条件がずれ、どちらが真かを判断する分岐が増える。

### D5: 宣言と実体の不一致を fail-closed で拒否する

次の 2 つをいずれも gate で fail-closed とする。

- 対応する crate の `Cargo.toml` に存在しない feature を宣言した場合
- track が宣言していない feature 配下の型を catalogue に記載した場合

前者を許すと、宣言と `Cargo.toml` が沈黙のうちに乖離する。後者を許すと、型を declare しながらその型が抽出面に現れないという、本 ADR が解消しようとしている不整合をそのまま再現できてしまう。

### D6: grandfathering を採用しない

feature を有効化すると、その feature 配下の module が持つ既存 public 要素が新たに TDDD の射程に入る。ある feature を最初に宣言する track が、可視化されたそれらの要素を catalogue に整備する責任を負う。

grandfathering リストを設けると、抽出が一度も観測できていなかった集合、すなわち最も乖離している可能性が高い集合を恒久的に検証対象外にすることになる。

### D7: CLI コマンドサーフェイスを変更しない

本 ADR の導入に伴い、新しいサブコマンド、引数、フラグを追加しない。既存コマンドの引数構文、stdout / stderr の出力形式、および exit code の意味も変更しない。宣言は成果物として読まれるものであり、コマンドラインから feature を渡す経路は設けない。

D3 が定める「宣言が不在なら fail-closed」は、既存コマンドに新たな前提条件を課すものであり、この不変条件の例外ではない。前提条件を満たさない場合の停止は、既存の fail-closed 失敗経路と同じ形で報告する。

コマンドラインから feature を渡せるようにすると、宣言という正本と並ぶ第二の入力経路が生まれ、baseline 取得と実測取得が別の feature 集合を観測し得るという、本 ADR が閉じようとしている穴が再び開く。

## Rejected Alternatives

### A. 抽出に `--all-features` を渡す

設定面を新設せずに済む。しかし crate が将来獲得するすべての feature に抽出が結合し、重量依存やテスト専用 feature まで巻き込む。重量 optional 依存を既定ビルドから外すという既存の判断も迂回することになるため採用しない。

### B. feature 集合を track 単位ではなく repository 単位で宣言する

宣言が 1 箇所に集まり把握しやすい。しかし宣言されたすべての feature のビルドコストを、feature-gated なコードに一切触れない track も含めて、全 track が gate 実行のたびに払うことになるため採用しない。

### C. feature-gated なコードを TDDD の射程外とする

chain ③ の対象を default feature の surface に限定し、抽出できないものについて non-blue signal を出さない方針。実装コストは最も小さい。しかし feature gate 配下のコードから型契約の機械検証が恒久的に失われるため採用しない。

## Consequences

### Positive

- baseline 取得と実測取得が単一の宣言を読むため、両者の feature 集合が乖離しなくなる。
- feature-gated な依存のビルドコストを、その feature を宣言した track だけが払う。
- 各 track の抽出面が明示され、レビュー可能になる。

### Negative

- track ごとに commit 対象の成果物と gate 入力が 1 つ増える。
- ある feature を最初に宣言する track が、その feature 配下の既存 public 要素の catalogue 整備を引き受けることになる。
- 宣言と `Cargo.toml` という一致すべき箇所が 2 つになる。D5 の検査で緩和するが解消はしない。

### Neutral

- 起草時点で active track が存在しないため、既存 track への実務上の影響は生じない。

## Reassess When

- 宣言と `Cargo.toml` の乖離が繰り返し発生し、D5 の検査だけでは運用が回らなくなったとき。
- 宣言された重量 feature の baseline 取得におけるビルド時間またはディスク消費が、運用上許容できない水準になったとき。
- feature-gated な型が常態化し、大半の track が同じ feature 集合を宣言するようになって、track 単位の宣言より repository 単位の既定値が適切になったとき。

## Related

- `knowledge/adr/2026-06-27-0440-tddd-rustdoc-document-hidden-items.md`
- `knowledge/adr/2026-07-20-1608-disk-footprint-and-dry-feature-gating.md`
