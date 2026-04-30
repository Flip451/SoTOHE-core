---
adr_id: 2026-04-29-1547-review-scope-lookup-commands
decisions:
  - id: D1
    user_decision_ref: "chat_segment:adr-add-review-scope-lookup-commands:2026-04-29"
    candidate_selection: "from:[results-flag(A),independent-2cmds,files-zero-arg(E),direct-ReviewCycle(F)] chose:independent-2cmds"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:adr-add-review-scope-lookup-commands:2026-04-29"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:adr-add-review-scope-lookup-commands:2026-04-29"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:adr-add-review-scope-lookup-commands:2026-04-29"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:adr-add-review-scope-lookup-commands:2026-04-29"
    candidate_selection: "from:[text-only,JSON(C)] chose:text-only"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:adr-add-review-scope-lookup-commands:2026-04-29"
    candidate_selection: "from:[new-service,direct-ReviewCycle(F)] chose:new-service"
    status: proposed
  - id: D7
    user_decision_ref: "chat_segment:adr-add-review-scope-lookup-commands:2026-04-29"
    candidate_selection: "from:[CSV-1line,multi-line(D)] chose:CSV-1line"
    status: proposed
---
# scope 分類ロジックの CLI 公開 (classify / files)

## Context

review v2 の scope routing は `track/review-scope.json` の glob pattern と `libs/domain/src/review_v2/scope_config.rs` の `classify` ロジックが「どのファイルがどの scope に属するか」を決めている。一方で「ある path X が今の routing で何 scope に分類されるか」「scope Y に今 何ファイル含まれているか」を CLI 経由で問い合わせる正規 API は存在しない。

直接的な不便:

1. `track/review-scope.json` のメンテナンス時、pattern を編集して期待通りに routing されるかを確認するには review 全体を実行して results 出力から逆算するしかなく、pattern の試運転コストが高い。
2. agent / 人間が「このファイルは domain か infrastructure か」を判定したいときに、`scope_config.rs` の glob を目視で当てるか review.json から逆引きするしかない。
3. 「scope に属する現 diff ファイル一覧」を取りたい場面 (agent briefing の組成 / 個別 scope の作業範囲確認) があるが、`ReviewCycle::get_scope_target` は library API として存在するのみで CLI 経路がない。

一方、新コマンドの実装に必要な API はすべて既に揃っている:

- `ReviewScopeConfig::classify(&[FilePath]) -> HashMap<ScopeName, Vec<FilePath>>` — 任意 file 列 → scope 別 routing
- `ReviewScopeConfig::all_scope_names()` — scope universe (named + `Other`) の列挙
- `ReviewScopeConfig::contains_scope()` — scope 名の有効性検査
- `ReviewCycle::get_scope_target(scope) -> ReviewTarget` — scope → 現 diff 対象ファイル
- `compose_v2::build_review_v2()` / `load_scope_config_only()` — composition root

`FilePath::new` はパス文字列のバリデーション (Empty / Absolute / Traversal) のみで、ファイル存在検証は行わない。よって新規作成予定の path に対しても `classify` を回せる。

既存 `sotp review results` は scope 別の **state + 最新 round の verdict + 履歴** を出力するが、scope に属する **個別ファイル一覧は出さない** (output 肥大を避けるため意図的に省略)。本 ADR の 2 コマンドはこの欠落を埋める read 系の独立した query として配置する。

## Decision

### D1: 2 つの新 CLI コマンドを `sotp review` 配下に追加する

`sotp review` 配下に以下 2 つの read-only サブコマンドを追加する:

- `sotp review classify --track-id <id> <path>...` — 任意の workspace 内 repo-relative path を受けて scope 名を返す
- `sotp review files --scope <name> --track-id <id>` — scope 名を受けて、現 diff base からの diff 範囲のうち当該 scope に属するファイル一覧を返す

両者とも read-only / side-effect なし。state を書かない。

### D2: コマンド名は `classify` / `files`

採用形式:

<!-- illustrative, non-canonical -->
```bash
sotp review classify --track-id <id> <path>...
sotp review files --scope <name> --track-id <id>
```

選定理由:

- 動詞 `classify` は「path を scope に分類する」の意味で direct。`scope-of` のような of-pattern より短い
- `files` は「scope の対象ファイル」の意味で名詞ベース。`--scope` flag を必須にすることで「review 対象 file 全体を返すコマンド」との誤解を避ける
- 既存 review subcommand (`results` / `check-approved` / `codex-local`) も品詞が混在しているため、新規 2 本の品詞統一にはこだわらない

### D3: `classify` の入力 — workspace 内 repo-relative path、存在検証なし

`classify` は workspace 内の任意の repo-relative path を受け付ける。

- **存在しないファイルも許可** — 新規作成予定の path について「このファイルはどの scope に分類されるか」を事前確認する用途を含めるため
- **検証は `FilePath::new` の string-level チェックのみ** — Empty / Absolute / Traversal を拒否することで「workspace 内」を担保。ファイルの実在は確認しない (I/O を回さない方針)
- **複数 path 引数を受け付ける** — `classify <p1> <p2> <p3>` のように複数渡せる。出力は path ごとに 1 行
- **`--track-id` 引数** — `files` と同等の必須引数として受ける。`track/review-scope.json` の `<track-id>` プレースホルダ展開と `other_track` 除外判定 (どのパスが「現在のトラックのパス」か) に必要

不正な path (絶対パス / `..` 含む / 空文字) は `FilePathError` を CLI 層で human-readable に整形し、exit code != 0 で返す。

### D4: `files` の入力 — scope 名 + 現 diff base からの diff 範囲

`files` は `--scope <name>` を必須引数として取り、当該 scope に属する現 diff 対象ファイル一覧を返す。

- **diff base** は既存 `review results` と同じ解決規則 (`.commit_hash` ファイル → 不在時は `git rev-parse main` fallback)
- **scope 名検証** — `ReviewScopeConfig::contains_scope` を流用。未定義 scope 名は `Unknown scope: <name>. Known scopes: <list>` を stderr に出して exit != 0
- **`--track-id` 引数** — 既存 `results` / `check-approved` と同等の必須引数として受ける (`track/review-scope.json` の `<track-id>` プレースホルダ展開に必要)

### D5: 出力フォーマットは text のみ

両コマンドとも text 1 系統のみ。JSON 出力は提供しない (Rejected Alternatives §C 参照)。

`classify` の出力 — `<path>\t<scope-csv>`:

<!-- illustrative, non-canonical -->
```text
$ sotp review classify --track-id <id> libs/domain/src/lib.rs apps/cli/src/main.rs track/items/foo/spec.md
libs/domain/src/lib.rs    domain
apps/cli/src/main.rs      cli
track/items/foo/spec.md   plan-artifacts
```

`files` の出力 — `<path>` 1 行ずつ:

<!-- illustrative, non-canonical -->
```text
$ sotp review files --scope domain --track-id <id>
libs/domain/src/foo.rs
libs/domain/src/bar.rs
```

レイアウト規則:

- `classify` は path と scope の間に **タブ 1 文字**。`cut -f1`, `cut -f2` で機械抽出可能
- `files` は 1 path = 1 行
- 両コマンドとも入力順または diff getter 由来の順を保つ
- 該当 0 件のときは empty stdout + exit 0 (エラーではない)

### D6: scope routing query は review cycle orchestrator とは独立した application service として usecase 層に配置する

`classify` / `files` の処理は CLI 層に直書きせず、review cycle 全体を回す既存の orchestrator (`ReviewCycle`) とも独立した application service (interactor) として usecase 層に配置する。CLI 層はその service を呼び出す薄いラッパーに留める。

理由:

- `ReviewCycle` は review cycle 全体の orchestrator として `Reviewer` / `ReviewHasher` / `DiffGetter` の複数 port に依存する。`classify` / `files` は review cycle 自体を回さない read-only クエリで、reviewer / hasher を必要としない
- `ReviewCycle::get_scope_target` を流用すると不要 dependency を CLI 経路から wire することになり、「scope query を review cycle の一部として実行している」という誤った設計シグナルが残る
- `.claude/rules/04-coding-principles.md` の Trait-Based Abstraction (Hexagonal Architecture) における最小 dependency 原則に従い、用途ごとに必要 port のみを持つ application service を立てる方が層責務が明確になる

domain 層の `ReviewScopeConfig` (`classify` / `all_scope_names` / `contains_scope`) は無変更で流用する想定。

**既存 `ReviewCycle` 統合パターンとの関係 (refactor 前提):**

既存 `ReviewCycle` は review cycle の write 系 (`review()` / `fast_review()`) に加えて read-only query (`get_review_states` / `evaluate_approval` / `get_scope_target` / `get_review_targets`) も統合的に提供しており、`review results` などの既存 CLI もこのパターンに乗っている。本 ADR の方針 (scope query を別 service に切り出す) は既存パターンと一致しない。

ただし既存パターンは最小 dependency 原則からの逸脱であり、**いずれ read-only query 群を別 application service に切り出すリファクタリングの対象**と位置付ける。本 ADR の新規追加分 (scope routing query) はそのリファクタリング後の正しい責務分離の形を先取りして実装し、新たな逸脱を上塗りしない方針を採る。既存 `ReviewCycle` の refactor 自体は本 ADR のスコープ外で、別途扱う。

**Phase 2 (type-designer) への委譲:** 新規 application service の module 名 / 公開 API name / signature / dependency port の具体形 / error 型 / wiring 構成は本 ADR では定めず、`usecase-types.json` を起こす type-designer がカタログ化する。本 ADR は「`ReviewCycle` と独立した最小 dependency の application service として usecase 層に置く」というアーキテクチャ判断のみを記録する。

### D7: scope universe の一致 + multi-match の CSV 表現 + `<excluded>` 表示

`classify` / `files` の出力する scope 名は **`scope_config.all_scope_names()` の universe 内** に収める:

- named scope: `track/review-scope.json` の `groups` で宣言されたもの
- implicit scope: `other` (常に universe に含まれる)

複数の named scope に該当する path は **scope 名カンマ区切り 1 行** で出力する:

<!-- illustrative, non-canonical -->
```text
libs/shared/src/foo.rs    domain,usecase
```

operational / other_track pattern にマッチして `classify` の出力 map から除外されたファイルは `<excluded>` で表示する:

<!-- illustrative, non-canonical -->
```text
track/items/other-track/spec.md    <excluded>
```

`<excluded>` は universe **外** の表示で、CLI 特有の sentinel。これにより operational pattern の「黙って除外する」挙動が初めて user 目に見える形になる。

## Rejected Alternatives

### A. 既存 `sotp review results` に `--show-files` flag を追加して 1 コマンドに統合する

却下理由 (責務の方向性が直交):

- `results` は「scope 別の review state を縦に深掘る」コマンド (state + verdict + 履歴 + commit hint)。`classify` / `files` は「routing そのものを覗く」query で、観測対象が直交している
- `results` の表示は state line で固まっており、ファイル列挙を mix すると state line / file list / history が同一出力に混在して可読性が下がる
- `classify` は「diff に存在しない任意 path」を扱うため、`results` の「現 diff 範囲を report する」前提から外れる
- 統合すると `results` の flag 集合が肥大し、user は同じコマンドの異なる flag 組合せを覚える必要が生じる

### B. 単一 file 用の新 domain API (`classify_one`) を追加する

却下理由 (既存 API で足りる):

- `classify(&[FilePath])` に長さ 1 の slice を渡すだけで意図は満たせる
- 単一 file 専用関数を追加すると、internal で `classify` を呼び出す薄ラッパーが domain 層に発生する。domain 層に CLI 都合のラッパーを下ろすと domain の意味論が CLI の都合で歪む
- multi-match の出力 shape (1 file → N scope) は domain の `HashMap<ScopeName, Vec<FilePath>>` 表現で既に正しい

### C. JSON 出力 (`--format json`) を最初から提供する

却下理由 (YAGNI):

- primary consumer は人間 / agent (LLM) / `cargo make` レベルの shell 連携 — いずれも text を直接読める
- JSON consumer の具体例が未特定。「将来 script で消費したい」は仮想的需要で、現時点で実装を駆動するには弱い
- text → JSON は後から非破壊的に追加可能 (`--format json` flag を後付け)
- 直近の `review results` ADR でも同じ判断 (text 単一フォーマット) が下されており整合する

### D. multi-match を rich display (1 file = N 行) にする

却下理由 (YAGNI / 一貫性):

- 「1 file = 1 行、scope は CSV」は machine-parse でも human-read でも素直で、tab-delimited で `cut -f1`, `cut -f2` の対称な抽出ができる
- multi-match を改行展開 (1 file → N 行) すると、入力順と出力行数が一致しなくなり「path が複数 scope にマッチしているか」を見抜くために行数を数える必要が出る
- リッチな書式 (table 整形 / box drawing 等) は依存の追加と保守を要するが、得られる読みやすさは marginal

### E. `classify` を file path 引数なし (= 現 diff 全体) で呼べるようにする

却下理由 (`results` との重複):

- 「現 diff 範囲の全 path → scope mapping」を見る用途は、`files --scope <name>` を各 scope に対して shell loop で呼び出すことで組み立てられる
- `classify` を引数 0 個でも動くようにすると「`results` の path-aware 版」と「`classify` の任意 path 版」の境界が曖昧化する
- `files --scope <name>` で scope 単位の path 列挙はカバー済み — 全 scope 横断の path 列挙が欲しいなら shell loop で組み立てられる

### F. CLI 層から `ReviewCycle::get_scope_target` を直接呼ぶ (新規 application service なし、既存パターン整合)

却下理由 (既存パターン整合より将来 refactor の先取りを優先):

- 既存 `ReviewCycle` は read-only query (`get_review_states` / `evaluate_approval` / `get_scope_target` / `get_review_targets`) を内包しており、`review results` などの既存 CLI もそのパターンに乗っている。`get_scope_target` を CLI から直接呼ぶ選択肢はこの既存パターンに整合する
- ただし `ReviewCycle` は review cycle orchestrator として `Reviewer` / `ReviewHasher` / `DiffGetter` の複数 port に依存しており、`classify` / `files` の用途では `Reviewer` / `ReviewHasher` を必要としない。既存パターンに従うと、scope query が不要 dependency を CLI 経路から wire することになり、最小 dependency 原則 (`.claude/rules/04-coding-principles.md` Trait-Based Abstraction) からの逸脱を新規追加分でも温存することになる
- 既存パターン自体が将来の refactor 対象 (read-only query を別 service に切り出す) と認識されているため、新規追加分でその逸脱を上塗りせず、refactor 後の姿を先取りする方が長期的に整合する
- 重複は短期的に許容するが、既存 `ReviewCycle` の refactor 完了時点で自然に解消できる方向に揃える

## Consequences

### Positive

- scope routing の「内部で何が起きているか」が CLI 一発で観測でき、`track/review-scope.json` の pattern メンテナンス時の試運転コストが下がる
- agent briefing / 人間の検査での scope 境界確認が SSoT (`scope_config.classify`) に揃い、目視での pattern 当てや review.json 逆引きが不要になる
- scope routing query が `ReviewCycle` (review cycle orchestrator) と独立した application service として置かれることで、責務が分離され、それぞれの dependency が最小化される (hexagonal architecture の原則)
- 出力が text + tab-delimited で shell 連携の自由度が高い
- excluded / Other / named scope の 3 状態を CLI 出力で区別することで、operational pattern の「黙って除外する」挙動が初めて見える

### Negative

- CLI surface が 2 つ増え、`review` 配下のコマンドは `codex-local` / `check-approved` / `results` / `classify` / `files` の 5 つになる。サブコマンド一覧 (`sotp review --help`) の長さは増える
- usecase 層に新規の application service が増える。`ReviewCycle::get_scope_target` と機能的に重なる範囲を別 service として保つことになり、短期的に内部実装の重複を許容する。既存 `ReviewCycle` の read-only query 切り出し refactor が完了した時点で、新規 service と既存 query 群が同じレイヤに揃い重複が解消する想定
- multi-match の意味 (1 file が複数 named scope に属する semantics) を user に説明する必要がある。前提自体は v2 redesign で確立しているが、CLI を通じて user 目に触れる頻度が増えるため help / doc に書く必要がある
- `classify` が「diff 外の任意 path」を受けるため、「diff に含まれている前提」と勘違いすると output 解釈を誤る恐れがある — help 文で「現在の diff 範囲とは無関係」を明示することで mitigate

### Neutral

- `track/review-scope.json` のスキーマ自体は変えない
- `review results` / `check-approved` / `codex-local` の CLI surface は無変更
- domain 層 (`ReviewScopeConfig`) の API surface は無変更
- `ReviewCycle` の API surface も無変更 (review cycle 内部 helper はそのまま残す)

## Reassess When

- script consumer (CI hook / 自動化) が JSON を要求し始めたとき (`--format json` の追加判断)
- multi-match が頻発し、CSV 1 行表現で読みにくくなったとき (display 拡張)
- `classify` が「diff 外の path」を受ける性質と、`files` が「diff 内のみ」を返す性質の非対称が user の混乱源として顕在化したとき (help 改善 or surface 再考)
- scope 数が大幅に増え、`<excluded>` / `other` / named の 3 区別では足りなくなったとき (scope universe の階層化)
- 既存 `ReviewCycle` の read-only query 切り出し refactor が実施されたとき (本 ADR で許容した重複が解消できるタイミング — 新規 scope query service と他 read-only service の API surface 統合判断)

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md` — review v2 (scope-based) データモデル
- `knowledge/adr/2026-04-18-1354-review-scope-prompt-injection.md` — `track/review-scope.json` 経由の scope 定義
- `knowledge/adr/2026-04-28-1905-review-results-command.md` — 直近の review CLI 拡張 (`results` / `check-approved` / `codex-local` の責務分離)
- `knowledge/conventions/review-protocol.md` — review v2 運用ルール
- `track/review-scope.json` — scope universe の SSoT
- `libs/domain/src/review_v2/scope_config.rs` — `ReviewScopeConfig::classify` / `all_scope_names` / `contains_scope`
- `libs/usecase/src/review_v2/cycle.rs` — `ReviewCycle::get_scope_target` / `get_review_targets`
- `libs/infrastructure/src/review_v2/diff_getter.rs` — `GitDiffGetter::list_diff_files`
- `apps/cli/src/commands/review/` — `ReviewCommand` enum / dispatch / `compose_v2`
