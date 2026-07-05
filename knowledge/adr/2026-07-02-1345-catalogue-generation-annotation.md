---
adr_id: 2026-07-02-1345-catalogue-generation-annotation
decisions:
  - id: D1
    user_decision_ref: "chat:session-e823e003:2026-07-02 「shape-lite には懐疑的。型カタログに完全な型情報を持たせ、カタログ段階の linter でコードスメルを早期検出する。アーキテクチャの強制という形でも実現する。そのためには完全な型情報が必要」"
    candidate_selection: "from:[完全型情報維持+生成注釈, shape-lite縮小+被覆検査] chose:完全型情報維持+生成注釈"
    status: proposed
  - id: D2
    user_decision_ref: "chat:session-e823e003:2026-07-02 ユーザー提案「type-designer は意図を sotp が提供する API に入力する。穴埋め箇所が埋め込まれたエントリーが json に追記される。スキーマの深い理解なしに意図を伝えて穴を埋めるだけでカタログが完成する」+ 裁定「決定の主眼はカタログ自体を軽量化することではなく生成+注釈に移行すること」"
    status: proposed
  - id: D3
    user_decision_ref: "chat:session-e823e003:2026-07-02 ユーザー提案「既存の型を簡単に reference する API も追加する」+ 適用範囲の確認「既存型 reference API は modify action のエントリの執筆にも使える」"
    status: proposed
  - id: D4
    user_decision_ref: "chat:session-e823e003:2026-07-02 「D1~D5までは確認しました。承認します」+ 2026-07-03 穴表現の再裁定: ユーザー提案「リーフを TODO バリアント付き enum に」→「リーフに限らず任意ノード」への発展に承認「OK」"
    candidate_selection: "from:[string値埋め込み限定, null予約, typed Slotリーフ限定, $todoノード任意位置] chose:$todoノード任意位置"
    status: proposed
  - id: D5
    user_decision_ref: "chat:session-e823e003:2026-07-02 「D1~D5までは確認しました。承認します」"
    status: proposed
  - id: D6
    user_decision_ref: "chat:session-e823e003:2026-07-02 インターフェース契約 hearing 裁定「per-entry サブコマンド」"
    candidate_selection: "from:[per-entryサブコマンド, 意図マニフェスト一括, ハイブリッド] chose:per-entryサブコマンド"
    status: proposed
  - id: D7
    user_decision_ref: "chat:session-e823e003:2026-07-02 インターフェース契約 hearing 裁定「check で一括再検証」"
    candidate_selection: "from:[checkで一括再検証, 編集も全てsotp経由, 下流signal任せ] chose:checkで一括再検証"
    status: proposed
  - id: D8
    user_decision_ref: "chat:session-e823e003:2026-07-03 CLI 命名の裁定「catalogue → catalog としてください。それ以外は、その命名でよいです。ADRに決定として書いてください」"
    status: proposed
---

# 型カタログ作成の「生成 + 注釈」への移行 — 意図入力スキャフォールディング API

## Context

信号機評価（SoT Chain 信号 🔵🟡🔴）の本来の狙いは、設計文書にない不要な機能を LLM が自動生成することを含む「自然言語による設計文書」からの逸脱の抑止である。実装 → 型カタログ → spec → ADR と連なる参照の連鎖が、実装から ADR への grounding を与える。

型カタログが**完全な型情報**（引数型・返り値型・generics・bound 等）を持つことには、grounding とは独立した検査価値がある。カタログ段階の linter は稼働済みの検査システムであり、`.harness/catalogue-lint/config.json` の有効ルール群が DDD ロール意味論（invariants / identity の署名検査、不変性の強制など）・Aggregate 境界（メンバーのロール整合・カプセル化）・アーキテクチャ配置（ロールの層制約、primary adapter 署名への内側ロール混入禁止）・primitive obsession 防止（syn ベースの型スロット走査）を検査し、commit gate（track-active-gate の依存チェーン）にも配線されている。これらの検査面はいずれも完全な型情報（メソッド署名の型・trait impl 宣言・field 可視性・generic bound）を入力とし、実装前の型宣言段階（Phase 2）で検査するため、修正コストが最小の時点で欠陥を捕まえられる。したがってカタログの情報量は削減しない。

一方で運用実測は、カタログ**作成方式**のコスト問題を示している。

1. **Phase 2（type-design）の執筆コスト**: type-designer はカタログ schema の知識（フィールド構成・schema_version・書式）を前提に JSON を手書きしており、時間がかかる。既存型をカタログに載せる際もシグネチャの手動転記が発生する。
2. **schema 変更への追従コスト**: schema の知識が writer prompt 側にあるため、schema 変更のたびに prompt の追従が必要になる。
3. **手書き起因の欠陥クラス**: 書式崩れ・必須フィールド漏れに加え、実在しない anchor への `spec_refs` 参照や語彙外の role 値のように、writer が構造化フィールドを自由記述する経路は、発覚が下流の signal 評価・レビューまで遅れる欠陥クラスを生む。

本 ADR の主眼はカタログ自体の軽量化ではなく、作成方式を「執筆」から「**生成 + 注釈**」へ移行することである。情報量は維持し、転記・書式・hash 計算という機械的作業を sotp（Rust）側へ移す。

## Decision

### D1: カタログは完全な型情報を維持する

カタログエントリの情報量（引数型・返り値型・generics・bound・codec 等の完全な型情報）は削減しない。稼働済みの catalogue linter ルール群（ロール意味論・Aggregate 境界・アーキテクチャ配置・primitive obsession 防止）は、いずれも完全な型情報を検査面として前提とする。impl → catalogue の構造検証（type-signals の宣言 vs 実装一致検査）も従来どおり維持する。

### D2: Phase 2 を「生成 + 注釈」に転換する — 意図入力スキャフォールディング API

type-designer はカタログ JSON を直接執筆しない。`sotp` が提供する API に意図（どの型に、どの role、どの spec anchor、どんな型形状）を入力すると、schema 準拠のエントリ骨格が穴埋め箇所付きでカタログ JSON に追記される。type-designer の仕事は意図の入力と穴埋めのみとなり、カタログ schema の深い理解を必要としない。

schema の知識（フィールド構成・schema_version・書式・エントリ順序）は sotp 側に一元化する。writer prompt が持つ知識は catalogue schema 全体から sotp API の使い方（意図入力と穴埋めの手順）に縮小する。prompt の更新が必要になるのは意図入力の表面（入力項目・role 語彙など）が変わる場合に限られ、表現・書式レベルの schema 変更は sotp の追従のみで完結する。穴埋め箇所（`$todo` ノード、D4）には記入指示を埋め込めるため、記入方法の案内は静的な prompt ではなく生成物自身が担える。schema 違反・必須フィールド漏れ・書式崩れという欠陥クラスは生成側で構造的に消滅する。機械生成によりエントリの順序・整形が安定するため、diff ノイズ起因のレビュー hash 陳腐化も減る。

### D3: 既存型 reference API

既存の型をカタログに載せる場合は、rustdoc 抽出（既存の実装カタログ計算機構を流用）から完全な型情報（同定情報・シグネチャ・フィールド）を取り込む reference API を使う。reference API の入力元になる schema export JSON も、type alias の alias target、struct の unit / tuple / plain 形状と `has_stripped_fields`、impl target の module path を落とさず保持する。type-designer による既存 shape の手動転記を廃止する。

reference API は action の種類を問わず使える。変更なしで参照するだけのエントリに加え、既存型を変更する modify エントリでは現状 shape を baseline として取り込んだ上で変更差分だけを type-designer が編集し、削除エントリは同定情報の取り込みで足りる。いずれの action でも「現状の記述」を人手で書き起こす工程が消える。手動転記が残らないのは新規型の add のみで、そこは型形状の設計自体が Phase 2 の作業であるため、type-designer が設計した形状を D6 の検証付き入力（Rust シグネチャ文字列）として渡す。

### D4: 穴は `$todo` ノードで表現する — 任意位置・指示文付き・完成境界は try_complete

JSON はコメントを持てないため、穴埋め箇所は機械検出可能なノードで表現する。木の任意のノード（リーフ・オブジェクト・配列要素・セクション丸ごと）を `{ "$todo": "記入指示" }` に置き換えられる。`$todo` はカタログ schema 全域の予約キーとし、正規フィールド名としては使わない。

<!-- illustrative, non-canonical -->
```json
{
  "name": "BranchStrategySnapshot",
  "role": "value-object",
  "docs": { "$todo": "この型が固定する不変条件を一行で記述" },
  "methods": { "$todo": "identity アクセサ以外に必要なメソッドを設計して埋める" },
  "shape": {
    "kind": "plain",
    "fields": [ { "name": "base_branch", "ty": { "$todo": "branch 名の domain 型を記述" } } ]
  }
}
```
<!-- illustrative, non-canonical -->

draft の扱いは型付きスキーマの手前の層に置く: カタログ file を JSON tree として走査して `$todo` ノードを検出する汎用関数と、完成境界 `try_complete`（`$todo` が残っていれば穴のパス + 指示文の一覧を Err で返し、なければ従来の typed DTO へ parse する）で構成する。既存の catalogue DTO・signal evaluator・linter・renderer は完成カタログのみを受け取るため無変更で済み、draft を読みうる入口は codec の共有ロード地点 1 箇所で「`$todo` あり → 当該エントリ pending 扱い」に分岐する。typed catalogue が try_complete 経由でしか構築できないという境界で、完成状態の保証を型に残す。

残存 `$todo` の検出は D7 の check に統合し、残存 = 非零 exit で block する（tech-stack.md の未解決 `TODO:` が実装を block する既存ゲートと同型の binary check）。

### D5: 構造化フィールドは穴埋めではなく検証付き入力にする

自由テキストの穴埋めは、intent / doc / 型詳細スロットなど、内容の記述自体が type-designer の設計作業であるフィールドに限定する。機械的に導出・検証できる構造化フィールドは sotp の検証付き入力で埋める:

- `spec_refs`: 参照先 anchor の実在を入力時に検証し、実在しない anchor は reject する
- `role`: セクションごとの role 語彙に対する enum 検証で fail-closed（typo は入力時に reject）

いずれも従来は下流の signal 評価・レビューで発覚していた欠陥を入力時 reject に前倒しするものである。なお鮮度 hash は従来どおり catalogue-spec-signals 側の機械計算であり、カタログ schema に writer が hash を記入する箇所は存在しない。本 decision の対象は writer が記入する参照値・role 値の入力時検証である。

### D6: API の入力契約 — per-entry サブコマンド、形状は検証付き入力、重複は fail-closed

意図入力は 1 エントリ = 1 回の sotp サブコマンド呼び出しとし、意図（layer / kind / name / role / spec anchor / 型形状）は引数として渡す。意図マニフェストのような第二の入力 schema は作らない（schema 知識が writer prompt に戻るため）。入力仕様の参照面は `--help` とする。コマンド群の命名は D8 で定める。

- 型形状（fields / methods / variants / generics / where predicates / impl blocks）は Rust 宣言断片またはシグネチャ文字列として渡し、sotp が parse して構造化する（既存の TypeRef parser / syn 資産を流用）。parse できない宣言・シグネチャは入力時 reject。生成時に確定できない箇所は引数で渡さず、生成されたエントリの該当ノードを `$todo` として保留できる（D4）。catalogue DTO が必須 payload を持つ role（`Entity.identity`、`AggregateRoot.identity`、`EventPolicy.reacts_to`、`Repository.aggregate`）は、空オブジェクトではなく該当 payload 位置に `$todo` を生成する
- generics / where 句は、型・trait・function・method・impl block の各宣言レベルを区別して入力し、カタログ schema の対応フィールド（`generics` / `where_predicates` / `impl_generics` / `impl_where_predicates` 等）へ構造化する。ある宣言レベルの generics / where 句に現行 schema の正規フィールドがない場合、D1 の完全型情報維持に従い、本 API 実装と同じ変更内で正規フィールドを追加してから生成する。silent drop、docs 文字列への退避、別宣言レベルへの代用は行わない
- 既存型の形状は D3 の reference API が rustdoc から取り込む
- 同名エントリが既に存在する場合は error とする。lenient / force 系フラグは設けない（既決の方針に従う）
- 書き込みはエントリ単位で原子的とし、エントリの順序・整形は決定論的とする

### D7: 強制境界 — 生成後の編集は自由、fail-closed の網は check で張る

生成後のカタログ編集（穴埋め・修正）は通常の file edit として自由とする（1 ファイル = 1 writer の原則は従来どおり）。入力時検証（D5）は早期フィードバックであり、強制ではない。強制は `sotp catalog check`（D8）— `$todo` 残存（try_complete の Err）・anchor 実在・role 語彙・schema 妥当の一括再検証 — が担い、Phase 2 完了ゲートと commit gate（track-active-gate の依存チェーン）の両方に配線する。check の fail は非零 exit で block する。

`catalog check` は呼び出し側から判定モードを受け取らない。catalogue file の存在状況から状態を自動判定する。対象 catalogue file がまだ 1 つも存在しない Phase 0 / 1 では no-op + warning で skip し、非零終了にしない。一方で、対象 track に catalogue file が 1 つでも存在する場合は catalogue 作成が開始済みとみなし、TDDD 対象 layer の catalogue file 不在、`$todo` 残存、anchor 不在、role 語彙外、schema 不正を非零 exit で block する。`catalog check` 自体は `.harness/config/signal-gates.json` の存在・値・妥当性を prerequisite にしない。利用者所有の gate 設定を CI で強制しないという `knowledge/conventions/responsibility-boundary.md` の境界を保つ。

### D8: CLI surface — `sotp catalog` コマンド群

コマンド群は top-level group `sotp catalog` とする（track artifact を扱う `review` / `dry` / `ref-verify` が top-level group である前例に従う）。綴りは `catalog` を採用する。既存機構の名称（`.harness/catalogue-lint/` や catalogue-spec-signals 等）の rename は本 ADR の scope 外であり、既存の綴りのまま残る。

動詞は 5 つ:

- `sotp catalog init` — active track の全 TDDD 対象 layer（architecture-rules.json から解決）の catalogue file を空の skeleton（schema_version 付き）として生成する。既存 file があれば error とし、一部生成はしない。エントリゼロの layer にも file の存在を要求する現行運用（全 layer 被覆の pre-review 検査、documentation-only track の空 catalogue）の bootstrap を機械化する
- `sotp catalog add` — 新規型の意図入力（D2 / D6）。`--layer` / `--kind` / `--name` / `--role` / `--anchor`（repeatable）に加え、`--field` / `--method` / `--variant` / `--trait-impl`（Rust 宣言断片またはシグネチャ文字列、repeatable）で形状を渡す。宣言レベルの型パラメータと where 句は `--generic` / `--where`（repeatable）で渡し、impl block レベルでは `--impl-generic` / `--impl-where`（repeatable）で `impl_generics` / `impl_where_predicates` に対応させる。method / function シグネチャ内の generics / where 句はそのシグネチャから parse する。function entry では `--name` の末尾と `--method` から parse した `fn` 名が一致しない入力を reject し、signature 名の silent drop を起こさない。省略した形状ノードと、選択 role の catalogue DTO が要求する未入力 payload は `$todo` で生成される（D4）
- `sotp catalog import` — 既存型の取り込み（D3）。`--layer` / `--type`（Rust パス、rustdoc から解決）/ `--action reference|modify|delete`（既定 `reference`）/ `--anchor`。`--anchor` は reference / modify のみで受理し、delete は spec_refs を持たない identity-only tombstone なので reject する
- `sotp catalog cite` — 生成後の anchor 追加（D5 の検証付き入力）。`--layer` / `--entry` / `--anchor`。delete tombstone は spec_refs を持たないため対象にできず reject する
- `sotp catalog check` — 完成検査（D7）。`--layer` 省略時は全 TDDD layer を対象とし、fail は非零 exit。対象 catalogue file がまだ 1 つも存在しない Phase 0 / 1 では no-op + warning で skip し、対象 track に catalogue file が 1 つでも存在する場合は `$todo` 残存・anchor 不在・role 語彙外・schema 不正・TDDD 対象 layer の catalogue file 不在を block する。`catalog check` は signal-gates 設定ファイルを必須入力にしない

共通仕様: track は省略時に現ブランチから自動解決する。`init` / `add` / `import` / `cite` は `track/items/<id>/` 配下を書き込む WRITE 操作なので、明示 `--track-id` は現在ブランチから導出した id と一致する場合だけ受理し、不一致または非 track ブランチでは fail-closed で停止する。これらのコマンドでは `--track-id` を cross-track override として扱わない。`check` は読み取り専用のため、明示 `--track-id` を READ override として使える。`add` / `import` / `cite` は対象 layer の catalogue file 不在時に error として `sotp catalog init` を案内し、暗黙の file 生成は行わない。

## Rejected Alternatives

### A. shape-lite 化 — カタログの情報量を縮小し、構造検証を被覆検査に再定義する

エントリを同定情報 + role + メンバー名 + spec_refs に縮小し、type-signals を「rustdoc 抽出 public item のカタログ被覆検査」に再定義する案（オーケストレーターの当初提案）。執筆コストと型詳細層の整合 findings は減るが、稼働済みの catalogue linter ルール群（ロール意味論・Aggregate 境界・アーキテクチャ配置・primitive obsession 防止）が検査面（完全な型情報）を失う。実装前の最小コスト時点で欠陥を捕まえるというカタログの検査価値を手放すことになるため、ユーザーが明示的に却下。

### B. 現状維持 — schema 知識を前提とした手書き執筆の継続

type-designer が schema を理解して JSON を書き下ろす現行方式。Phase 2 の執筆時間・既存型の転記コスト・schema 変更への prompt 追従・手書き起因の欠陥クラス（書式崩れ、placeholder hash）がすべて残る。却下。

### C. 構造化フィールドも自由テキスト穴埋めで入力する

spec_refs や role も sentinel 置換の自由記述で埋める案。実在しない anchor や語彙外の role を書き込める経路が残り、発覚が下流の signal 評価・レビューまで遅れる。検証付き入力（D5）を採用。却下。

### D. コメント形式 `/*TODO: 指示*/` の穴表現

JSON はコメントを持てず、カタログ（JSON）内では成立しない。`$todo` ノード（D4）の方が sotp / jq による残存検出も決定論的になる。却下。

### E. string フィールド限定の sentinel 値埋め込み

穴を intent / docs のような自由テキスト（string）フィールドへの値埋め込みに限定し、非 string 構造は生成時入力で埋め切る案（本ドラフトの旧稿）。schema・codec を無変更で済ませられるが、型形状の部分保留が表現できず、生成時に形状を渡し切ることを強制する。`$todo` ノード（D4）は draft 層の走査で同等の無変更性を保ちながら任意粒度の保留と指示文を両立する上位互換のため、置き換えて却下。

### F. null を穴専用に予約する sentinel

null は型を問わず穴を表現できるが、記入指示を運べない。また null を正規の最終値として永久に使わないという全域予約規約が必要になる。却下。

### G. typed `Slot<T>`（Filled | Todo）をリーフ単位で schema に織り込む

各リーフの型を Todo バリアント付き enum にして、穴を型付きスキーマ内で表現する案。指示文を運べ、穴の位置を compile-time に固定できるが、catalogue DTO 全体と全消費者（signal evaluator / linter / renderer）への改修波及が最大になる。`$todo` ノード（D4）は draft 層（JSON tree 走査）で同じ表現力を確保し、typed 境界を try_complete に集約できるため却下。

## Consequences

### Positive

- schema 違反・必須フィールド漏れ・書式崩れ・実在しない anchor 参照という手書き起因の欠陥クラスが、生成と検証付き入力で構造的に消滅する
- 既存型のシグネチャ手動転記が reference API で消滅する
- 表現・書式レベルの schema 変更が sotp のコード変更のみで完結する（writer prompt の追従は、意図入力の表面が変わる場合に限られる）
- エントリの順序・整形が安定し、diff ノイズ起因のレビュー hash 陳腐化・再レビューが減る
- カタログ段階 linter の検査面（完全な型情報）は無傷で維持され、生成による構造保証で lint 入力の構文的な質も安定する
- テスト義務導出（role × spec anchor）を前提とする設計と要求フィールドが一致し、両立する
- 生成後の編集の自由度を保ったまま、`sotp catalog check` の一括再検証が Phase 2 完了・commit で `$todo` 残存を fail-closed にする（入力時検証と gate 時強制の二段構え）。catalogue file がまだ 1 つも存在しない Phase 0 / 1 の欠損入力 skip 意味論だけ維持する
- 残存穴の一覧がパス・記入指示付きで try_complete の Err として機械導出され、drift レポートを別途実装する必要がない

### Negative

- 新規型の型詳細の設計・記入コストは残る（型形状の設計自体が Phase 2 の作業であり、本 ADR の対象外）
- 型詳細層の artifact 間矛盾 findings は完全には消えない（手書き自由度の縮小による低減は見込めるが、発生源の除去ではない）
- sotp に新しいサブコマンド群（`catalog init / add / import / cite / check`）の実装・保守コストが乗る
- schema の権威が sotp のコードに移るため、schema 変更はコード変更を伴う（従来は JSON 書式の合意のみ）
- `sotp catalog check` が Phase 2 完了ゲートと commit gate に加わり、ゲートの実行時間が増える
- draft 状態は型付きドメインの外（JSON tree 層）で表現されるため、穴の位置に対する compile-time 検査はない（検査は try_complete の実行時全域関数が担う）

### Neutral

- type-signals（宣言 vs 実装の一致検査）の意味は不変
- メソッド単位の spec_refs は従来どおり任意の精緻化として残る
- `$todo` はカタログ schema 全域の予約キーになる（正規フィールド名としては使用不可）

## Reassess When

- telemetry の前後比較で Phase 2 所要時間が十分改善しない場合 — 意図入力からの自動導出範囲（生成の粒度）の再設計を検討
- 型詳細層の整合 findings が高止まりする場合 — カタログ段階 linter の拡充（矛盾の機械検出化）を検討
- role 語彙またはカタログ schema の大きな変更が入った場合 — 生成 API と検証付き入力の再設計
- テスト義務導出ゲートの導入で要求フィールドが変わった場合 — 生成骨格の既定フィールド集合を再確定

## Related

- `knowledge/adr/` — ADR 索引（TDDD taxonomy・カタログ linter・SoT Chain 意味論検証の各 ADR はここから辿る）
- `knowledge/conventions/prefer-type-safe-abstractions.md` — primitive obsession 防止 / Newtype パターン（D1 の linter 検査面の背景）
- `knowledge/conventions/tddd-product-correctness.md` — TDDD と実装正しさの分担
- `knowledge/conventions/workflow-ceremony-minimization.md` — binary gate / file 存在 = phase 状態の原則（D4 の sentinel 残存検査が従う）
