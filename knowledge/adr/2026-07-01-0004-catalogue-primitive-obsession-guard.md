---
adr_id: 2026-07-01-0004-catalogue-primitive-obsession-guard
decisions:
  - id: D1
    user_decision_ref: "chat_segment:catalogue-primitive-obsession-guard:2026-07-01"
    candidate_selection: "from:[field-only-string, all-positions-parametrized, all-positions-parametrized-incl-result-err] chose:all-positions-parametrized-incl-result-err"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:catalogue-primitive-obsession-guard:2026-07-01"
    candidate_selection: "from:[regex-match, syn-walk-with-fn-skip, syn-walk-full-recursive] chose:syn-walk-full-recursive"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:catalogue-primitive-obsession-guard:2026-07-01"
    candidate_selection: "from:[domain-port+infra-impl, infra-codec-validate] chose:domain-port+infra-impl"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:catalogue-primitive-obsession-guard:2026-07-01"
    candidate_selection: "from:[per-field-escape-hatch, no-escape-generic-newtype] chose:no-escape-generic-newtype"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:catalogue-primitive-obsession-guard:2026-07-01"
    candidate_selection: "from:[staged-warn-then-block, blocking-no-migration] chose:blocking-no-migration"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:catalogue-primitive-obsession-guard:2026-07-01"
    candidate_selection: "from:[reuse-existing-rulekind, new-rulekind-variant] chose:new-rulekind-variant"
    status: proposed
  - id: D7
    user_decision_ref: "chat_segment:catalogue-primitive-obsession-guard:2026-07-01"
    candidate_selection: "from:[errortype-special-cased, dto-command-exempt-only-errortype-general] chose:dto-command-exempt-only-errortype-general"
    status: proposed
---
# 型カタログの primitive obsession guard（syn AST 走査・粒度 config）

## Context

型カタログ（`<layer>-types.json`）は type-designer が Phase 2 で書く型契約 SSoT である。ここで struct / enum のフィールド型や関数のエラー型に裸のプリミティブ（`String` / `i32` / `bool` 等）が現れると、primitive obsession（意味を持つ値を newtype で表現せず裸のプリミティブで持つこと）を型契約の段階で許してしまう。`knowledge/conventions/prefer-type-safe-abstractions.md` の Newtype ルールは存在するが、機械的な強制がなく、レビューやメモリ頼りで形骸化しうる。

本 lint を追加するにあたり、`prefer-type-safe-abstractions.md` 冒頭ルール（「バグクラスは lint で禁止するのではなく型で排除せよ、lint は最後の手段」）との緊張を明示しておく。本 lint は同ルールの Decision Flow ステップ3（最後の手段の lint）ではなく、**ステップ2「型で排除する（newtype 化する）」ことを type-designer に強制するメタ lint** である。コンパイラは「裸の `String` を使うな」を強制できない（`String` は正当な型）ため、newtype 化の規律を機械化する現実解がカタログ lint となる。

primitive obsession は `String` に限らない。`i32` / `u32` / `bool` / `f64` / `usize` 等の裸のプリミティブも同じ問題（意味を持つ値を裸で持つ）であり、同じ検出機構をプリミティブ集合でパラメータ化すれば全プリミティブに一貫適用できる（D1）。

設計に先立って全カタログの実態を集計した（出現位置 × 層 × role マトリクス、`action != "reference"` の宣言箇所のみ）。検出**エンジン**は全位置を走査可能とするが（D1・D2）、**既定 config でどの (layer, role, 位置) を効かせるか**の判断根拠として次が重要である:

- **フィールド位置の裸プリミティブはほぼ誤検出源を持たない**。named struct field と enum variant field は「型が保持するデータ」であり、意味を持つ値の primitive obsession がそのまま現れる。
- **`result_err`（エラーを生 String で返す）はアンチパターンの典型**。`Result<_, String>` の Err 位置の `String` は `thiserror` enum に矯正したい。Ok 位置（`Result<String, E>` の成功値 String）はレンダラ/シリアライザの戻り値として正当なので、Err スロットだけを狙い撃つ（D1・D2）。エラー型の**フィールド**の生 `String` も特別扱いせず、フィールド位置の一般対象に含める（下記 role 別の項）。
- **newtype 内部表現はカタログに出現しない（0 件）**。`pub struct Email(String)` の内部 `String` は private フィールドで rustdoc が strip する（`has_stripped_fields: true`）ため、カタログに書かれない。したがって lint は newtype を壊すことも誤検出することも構造的に不可能である。
- **引数・戻り値位置（Ok 側・一般の param/return）は誤検出が多い**。`impl Into<String>`（境界入力）、`&str`（借用）、`Arc<dyn Fn(String, ...) -> ...>`（クロージャシグネチャ）、`Result<String, ConcreteError>`（レンダラの Ok 値）が支配的で、これらを既定で違反にすると例外が膨大になる。したがって param / return（Err スロットを除く）は既定 OFF とし、必要なプロジェクトが config で opt-in する（D7）。

さらに role 別の集計から、フィールド位置で既定除外するのは次の 2 role のみとし、残りは一般対象とする（D7）:

- **除外する `Dto`**（全層、計 130 件）: 境界の外部形式（JSON/CLI/API）の入れ物で、生データを保持して domain 境界で newtype に変換する。
- **除外する `Command`**（usecase、計 39 件）: ユースケースへの外部入力に近い。

これ以外の role（`ValueObject` / `Entity` / `AggregateRoot` / `ErrorType` 等）のフィールド String は一般対象である。とりわけエラー型（`ErrorType`、全層計 180 件）を特別扱いはしない — 生 String 禁止を敷けば、その構成要素の String も他と同じく自然に対象になる。エラーメッセージは `#[error("...")]` Display 属性、下位エラーは `#[from]` + `#[error(transparent)]` 透過ラップ、固有値は newtype、外部ライブラリの生文字列エラーは newtype ラップ、で排除できるため現実的である（外側の層ほど下位エラーの透過ラップが効く）。

既存資産が本 lint を後押しする:

- 型カタログ linter framework（`CatalogueLinterRule = RuleTarget（role selector）+ CatalogueLinterRuleKind` と `.harness/catalogue-lint/` の宣言的ルール config）が既にあり、role 配置制約などの前例がある。粒度制御（D7）はこの framework にそのまま乗る。
- カタログの TypeRef（型を表す文字列）を `syn` でパースする経路が infrastructure に既にある（`libs/infrastructure/src/tddd/type_ref_parser/`）。`syn = { version = "2", features = ["full", "visit"] }` が依存として入っており、`visit` feature（AST visitor 走査）も有効化済みである。
- 層名・層構成は `architecture-rules.json`（workspace structure と layer policy の SSoT）に定義される。本 lint の layer 参照は常にここを参照する（既定 config の層名列挙もここから生成し、直書きしない。D6・D7）。
- TDDD の signal chain（カタログ ↔ rustdoc の型名比較）により、カタログで newtype を宣言すれば実装側も newtype でないと signal が 🔵 にならない。つまりカタログ lint 単体で実装側の primitive obsession も間接的に縛れる。

## Decision

### D1: 検出対象は「プリミティブ集合 × 出現位置（Result の Err スロットを含む）」とし、config でパラメータ化する

検出対象の**プリミティブ型**は config で指定する集合とする（既定 `String`、`i32` / `u32` / `bool` / `f64` / `usize` 等を追加できる）。`String` は primitive obsession の一例にすぎず、裸プリミティブ全般を同じ機構でパラメータ化して扱う。

検出**位置**はフィールドに限定しない。全出現位置を検出可能とする:

- named struct field（plain struct の `shape.fields[].ty`）
- enum variant のフィールド（`variants[].payload.fields[].ty`）
- メソッド / 関数の param / return / bound
- **`result_err`**: 任意位置の TypeRef の型木に現れる `Result<_, E>` の **Err スロット**（`Result<T, String>` の `String`）。エラーを生 String で返すことを、Ok スロット（`Result<String, E>` のレンダラ戻り値）を巻き込まずに狙い撃つための専用位置。
- type alias target など、TypeRef が現れる位置一般

透過コンテナの型引数に現れるプリミティブも対象とする（`Option<String>` / `Vec<String>` / `BTreeMap<String, _>` → `Option<NewType>` / `Vec<NewType>` へ矯正）。

明示的な対象外は設けない（`Result<_, String>` はむしろ `result_err` 位置で対象に含める）。newtype 内部（tuple struct の private フィールド）は rustdoc-stripped でそもそもカタログに出現しないため、除外条項を要しない。

実際にどの `(layer × role × 出現位置 × プリミティブ)` の組を効かせるかは D7 の粒度 config で制御する。既定は全層 × (named field + variant field + result_err) × `String` で ON とし、フィールド位置については `Dto`（全層）・`Command`（usecase）の role のみ除外する（それ以外は `ErrorType` を含め一般対象。`result_err` は role を問わず ON）。

### D2: 検出手段は既存 syn ベース TypeRef パーサの AST を全再帰走査する

正規表現による文字列マッチではなく、infrastructure の `type_ref_parser` がカタログの TypeRef を `syn::Type` にパースする経路を再利用し、`syn::visit::Visit` で型木を**全再帰走査**して、指定プリミティブの ident を検出する。透過コンテナ（`Option` / `Vec` / `Box` / `Arc` / `BTreeMap` 等）の型引数も、関数型（`Fn` / `FnMut` / `FnOnce`）シグネチャ内（例: `Box<dyn Fn(String) -> _>`）も**区別せず全て走査する**（特例を設けない）。ident 完全一致で判定するため、`NonEmptyString` / `OsString` のような「プリミティブ名を部分文字列に含むが別物の型」は自動的に除外される。

`result_err` 位置は、走査中に `Result<T, E>`（path segment が `Result` で型引数が 2 つ）を見つけたとき、その**第 2 型引数（E スロット）**を `result_err` 位置として記録することで識別する。第 1 型引数（Ok スロット）は通常の走査位置として扱い、`result_err` とは区別する。これにより `Result<T, String>` の Err の String だけを `result_err` 位置に、`Result<String, E>` の Ok の String は Ok 側位置に振り分けられる。

どの位置を実際に違反とするかは D7 の粒度 config が決めるため、走査エンジン側で位置や深さの特例（Fn 内除外など）を持たせる必要はない。エンジンは「この TypeRef の型木のどの位置に指定プリミティブが現れるか」を答え、適用可否は config が判断する。

### D3: syn 走査は infrastructure に置き、lint ルール宣言は domain に置く（port 接続）

domain に「型木のどの位置に指定プリミティブが含まれるか」を判定する port を定義し、infrastructure が既存 `type_ref_parser` の syn 走査でこれを実装する。`catalogue_linter`（domain）は port 経由で判定を呼ぶ。これにより domain の syn 非依存と hexagonal 純粋性を維持する（domain は判定の意味論だけを持ち、syn への依存は infrastructure に閉じる）。

### D4: per-instance の escape hatch は設けない

個別フィールドに付ける例外マーク（per-instance の opt-out 注釈）は導入しない。この種の例外は `prefer-type-safe-abstractions.md` の「lint ルールは例外追加で形骸化する」に該当し、gate を無条件 blocking に保てなくなる。真に自由文が本質のフィールド（自由記述メッセージ・生ログ等）には、per-instance 例外ではなく **意味を与えた汎用 newtype**（`FreeText` / `LogLine` 等）を使う。これは「例外」ではなく「型付け」であり、規約と整合する。

なお、`(layer × role × 出現位置 × プリミティブ)` というカテゴリ単位での ON/OFF は D7 の粒度 config で提供する（例: Dto / Command の除外）。これは per-instance escape hatch とは**別次元**（個別フィールドの気まぐれな例外ではなく、`.harness/catalogue-lint/` で一元管理・レビューされる構造的なカテゴリ宣言）であり、D4 の否定対象ではない。

### D5: gate は最初から blocking とし、移行措置を設けない

warn → block の段階導入は行わない。lint は active track のカタログのみを検査する既存の `track-active-gate` に乗るため、過去の完了トラックのカタログは遡及検査されない。したがって後方互換性・既存違反の段階移行を考える必要はなく、最初から blocking gate として導入できる。新規トラックの type-designer が書くカタログにのみ適用される。既定の適用範囲は D7 の通り（フィールド位置は Dto / Command のみ除外、それ以外の全 role と `result_err` は ON）。

### D6: 新しい `CatalogueLinterRuleKind` variant として実装する

既存の `CatalogueLinterRuleKind`（現行の rule 種別群）に「型木に現れるプリミティブの禁止」に相当するものはない（`NoRoleInMethodSignature` はメソッドシグネチャに現れる **role 型**を対象とし、プリミティブや型木走査を対象としない）。したがって本 lint は新しい rule variant を追加して実装する。

新 variant の payload は次を持つ:

- **禁止プリミティブ集合**（例: `["String"]`）
- **適用対象 layer 集合**（層名のリスト。`architecture-rules.json` に定義された名前のみ受け付け、存在検証も同 SSoT 基準。`"all"` のような特殊値は設けない — 全層を対象にするなら層名を列挙する。既定 config を生成する際に同 SSoT から列挙する）
- **検出位置集合**（named field / variant field / param / return / bound / result_err …）

**role 軸**による絞り込みは新 variant に持たせず、既存の `RuleTarget`（`target_roles`）を再利用する。

### D7: 適用範囲を (layer × role × 出現位置 × プリミティブ) の 4 軸粒度で設定可能にする

lint は一律ではなく、**4 軸の粒度**で ON/OFF を宣言的に切り替えられるものとする。既存 framework にそのまま乗せる:

- **role 軸**: 既存の `RuleTarget`（`target_roles`）。
- **layer 軸**: 層名のリスト（`architecture-rules.json` 定義済みの名前のみ）。`"all"` 特殊値は設けず、全層対象なら層名を列挙する。
- **出現位置軸**: named field / variant field / param / return / bound / result_err …（将来位置を足す余地）。
- **プリミティブ軸**: `String` / `i32` / `bool` / …（D1 のパラメータ）。

宣言は既存の `.harness/catalogue-lint/` config で行う（role-placement-lint と同じ config 面）。既定 config（テンプレート同梱）は **`architecture-rules.json` の層名を列挙して生成**し、位置ごとに次の 2 系統を持つ:

1. **`result_err` × `String`**: 全層 × **全 role** で ON（`Result<_, String>` を型付きエラーに矯正。role 除外なし）。
2. **named field + variant field × `String`**: 全層 × 全 role で ON、ただし次の role のみ**除外**する:
   - `Dto`（全層） — 境界の外部形式の入れ物。
   - `Command`（usecase） — 外部入力に近い。

   除外はこの 2 role のみで、`ErrorType` を含む他の全 role（`ValueObject` / `Entity` / `AggregateRoot` 等）は対象である。エラー型を特別扱いしないのは、生 String 禁止を敷けばその構成要素の String も一般対象として自然に消える（メッセージは Display 属性、下位エラーは `#[from]` transparent、固有値は newtype で排除できる）ためである。

`String` 以外のプリミティブ、および一般の param / return（Err スロットを除く）は既定 OFF とし、利用者が config に追加して opt-in する。`ValueObject` / `Entity` / `AggregateRoot` / `ErrorType` などのフィールドは既定 ON（primitive obsession の対象）。

config イメージ（層名は SSoT から列挙生成した結果の例）:

```json
// <!-- illustrative, non-canonical -->
[
  // (1) result_err は全 role で禁止（Result<_, String> → 型付きエラー）
  { "target_roles": ["*"],
    "kind": { "ForbidPrimitiveInTypes": {
      "primitives": ["String"],
      "layers": ["domain", "usecase", "infrastructure", "cli", "cli_driver", "cli_composition"],
      "positions": ["result_err"] } } },
  // (2) フィールドは Dto / Command を除く全 role で禁止（ErrorType は対象に含む）
  //     （target_roles の絞り込み、または除外エントリで Dto を全層、Command を usecase から外す）
  { "target_roles": ["ValueObject", "Entity", "AggregateRoot", "ErrorType", "..."],
    "kind": { "ForbidPrimitiveInTypes": {
      "primitives": ["String"],
      "layers": ["domain", "usecase", "infrastructure", "cli", "cli_driver", "cli_composition"],
      "positions": ["named_field", "variant_field"] } } }
]
```

複数の rule entry を並べることで、例えば「domain の Entity role で `String` と `i32` を named field で禁止」「usecase の Dto / Command では String を許可」のような細粒度の制御ができる。

この粒度制御は D4 が否定した per-instance escape hatch とは**別次元**である。escape hatch は個別フィールドに付ける気まぐれな例外で形骸化しやすいのに対し、本 config は `(layer, role, 位置, プリミティブ)` というカテゴリ単位の構造的宣言であり、一元管理・レビューできるため形骸化しにくい。

## Rejected Alternatives

### A. 正規表現によるプリミティブ検出

`\bString\b` 等でカタログの型文字列をマッチする案。

却下理由: 単語境界マッチでも `dyn Fn(String) -> _` の内部や、型引数のネスト位置・意味（Result の Ok/Err スロットの区別など）を扱えない。型を構造として扱えないため、透過コンテナの再帰走査も Err スロットの狙い撃ちもできない。`syn` の構造化走査が明確に優る（D2）。

### B. 実ソース（`.rs`）を直接 lint する（dylint / clippy）

カタログではなく実装コードの struct / enum フィールド型を直接 lint する案。

却下理由: 本課題の主眼は型契約（カタログ）の primitive obsession であり、カタログ lint + TDDD signal chain で実装側まで波及する（Context 参照）。実ソース lint は dylint 等の追加ツール導入コストを伴い、当初の課題設定（カタログ対象）からも外れる。

### C. param / return（Ok 側・一般位置）を既定 ON にする

Err スロットに限らず、引数・戻り値位置全体を既定の適用範囲に含める案。

却下理由: 実態集計で、param / return 位置は `impl Into<String>`（境界入力）、`&str`（借用）、`dyn Fn` クロージャ、`Result<String, E>`（レンダラの Ok 値）が支配的で誤検出が過大。エンジンは全位置を検出できる（D1）が、既定は誤検出の少ないフィールド位置と `result_err`（エラーを生 String で返すアンチパターンが明確）に絞り、その他の param / return は D7 config での opt-in とする。

### D. per-instance の escape hatch 機構を設ける

自由文フィールド向けに個別の例外注釈を用意する案。

却下理由: 個別例外は形骸化する（`prefer-type-safe-abstractions.md`）。自由文が本質のフィールドは汎用 newtype で型付けする方が規約と整合し、gate を無条件 blocking に保てる（D4）。カテゴリ単位の除外は D7 の粒度 config で構造的に扱う。

### E. カタログの `ty` フィールドを構造化型に変える（schema v6 化）

`ty` を文字列ではなく構造化オブジェクトにして、文字列パースを排除する案。

却下理由: 全カタログの大規模 migration を伴う。「TypeRef は文字列のまま、infrastructure で syn パースする」既存方針（`type_ref_parser`）を維持し、パーサの走査を再利用するだけで足りる。

### F. 段階的移行（warn → block）を行う

既存違反を warn で可視化してから blocking に切り替える案。

却下理由: 移行不要・後方互換性不要の方針であり、`track-active-gate` が過去トラックを遡及しないため、最初から blocking で問題が生じない（D5）。

### G. 適用範囲を全層一律に固定する（粒度制御を持たせない）

粒度を設けず、全カタログに一律で lint を効かせる案。

却下理由: プリミティブが設計上正当なカテゴリ（`Dto` の外部形式、`Command` の外部入力）を構造的に除外できず、per-instance escape hatch に頼らざるを得なくなる（D4 と衝突）。既存 framework が `RuleTarget` + config による粒度宣言を既に備えているため、それに乗せて 4 軸粒度で制御する方が自然（D7）。

### H. 検出対象を `String` のみに固定する（プリミティブ非パラメータ化）

`String` 専用ルールとして実装し、他プリミティブは扱わない案。

却下理由: primitive obsession は `String` に限らず `i32` / `bool` 等でも同型の問題であり、検出機構は同一（型木走査 + ident 一致）。プリミティブ集合を config パラメータにすれば 1 つの variant で全プリミティブを一貫して扱え、プリミティブごとにルールを増やさずに済む（D1）。

### I. 関数型シグネチャ内を走査対象外にする（エンジンに位置特例を持たせる）

`Box<dyn Fn(String) -> _>` の内部などをエンジン側で走査対象外にする案。

却下理由: どの位置を効かせるかは D7 の config が制御するため、エンジンに位置特例を埋め込むと config の粒度制御と二重管理になる。エンジンは全再帰走査で「どの位置にプリミティブが現れるか」を答え、適用可否は config に委ねる方が単純で一貫する（D2）。

### J. layer を `"all"` 特殊値やルール定義への直書きで表現する

layer 軸に `"all"` のような特殊値を設ける、あるいは新 variant / config に `["domain", "usecase", ...]` を恒久的に直書きする案。

却下理由: `"all"` 特殊値は config パーサに分岐を増やすだけで YAGNI。全層対象は層名の列挙で表現できる。層名・層構成は `architecture-rules.json` が SSoT であり、既定 config はそこから層名を列挙して**生成**する（ルールに直書きせず、層の増減に追従できる）。config は同 SSoT 定義済みの層名だけを受け付け、未定義の層名は検証で弾く（D6・D7）。

### K. `Result<_, String>`（エラー型 String）を lint 対象外にする

エラー型の primitive obsession は error-handling の別軸だとして、本 lint の対象から除外する案。

却下理由: 除外する技術的根拠がない。エラーを生 `String` で持つのは primitive obsession の典型であり、むしろ狙って禁止したい。Ok スロット（`Result<String, E>` のレンダラ戻り値）との区別は `result_err` 位置で解決できる（D1・D2）。

## Consequences

### Positive

- primitive obsession が型契約の段階で機械排除され、Newtype ルールが強制される。`String` に限らず設定した全プリミティブに一貫適用できる。
- エラー処理が型で構造化される。`result_err`（エラーを生 String で返す禁止）と、エラー型のフィールド String が一般対象であることが相まって、下位エラーの `#[error(transparent)]` + `#[from]` 透過ラップ、メッセージの Display 属性化、型付きコンテキスト（newtype）を促す。外側の層で domain エラーを String に潰す型情報喪失を防げる。
- 既定（フィールド + `result_err` × `String`、Dto/Command 除く）では誤検出がほぼ 0。Ok スロットのレンダラ String を巻き込まず、newtype 内部は rustdoc-stripped でカタログに出ないため lint が newtype を阻害しない。
- 既存の syn TypeRef パーサを再利用するため、型パーサの重複実装が生じない。全再帰走査でエンジンに位置特例を持たせないので、走査ロジックが単純になる。
- 適用範囲を `(layer × role × 出現位置 × プリミティブ)` の 4 軸で宣言的に制御でき、正当にプリミティブが必要なカテゴリ（Dto / Command）を構造的に除外できる（per-instance 例外に頼らない）。既存 framework の `RuleTarget` + config にそのまま乗る。
- layer は `architecture-rules.json` を SSoT とし、既定 config を層名列挙で生成するため、層の増減に追従でき、`"all"` 特殊値の分岐も持たない。
- `ValueObject` / `Entity` / `AggregateRoot` / `ErrorType` 等のフィールドは既定 ON で残るため、primitive obsession と構造化不足の主対象を取り逃がさない。
- TDDD signal chain を通じて実装側の primitive obsession も間接的に縛れる。
- 新しい gate 機構を増やさず、既存の catalogue linter framework と `track-active-gate` に乗る。

### Negative

- 新しい `CatalogueLinterRuleKind` variant の追加に伴い、codec / signal evaluator など variant を網羅する箇所の更新が必要になる。
- 型木走査に `result_err`（Result の第 2 型引数）を識別するロジックが加わる。
- 4 軸 config と、既定除外（Dto 全層 / Command usecase）の設計・保守・レビューコストが生じる。role 除外の表現（denylist / 列挙）を config スキーマで決める必要がある。
- domain 判定 port の追加（hexagonal 分離のためのボイラープレート）。
- type-designer はエラー型を含む該当カテゴリで裸プリミティブのフィールドを書けなくなり、`#[error(...)]` Display + `#[from]` transparent + 型付きコンテキストへの書き換え、および newtype 定義の負荷が増える。
- 真に自由文のフィールド向けに汎用 newtype を用意する初期コストが生じる。

### Neutral

- Ok スロット（`Result<String, E>`）の String は既定では対象外であり、必要なら config で対象化できる。
- 過去の完了トラックのカタログは遡及されないため、既存カタログの数値上の違反は残るが gate には影響しない。
- 既定 config は保守的（一般の param / return と `String` 以外は opt-in、Dto / Command は除外）である。ただしエラー型も一般対象のため、既存カタログでは外側の層のエラー型に多数の String があり（usecase 76 / infra 63）、新規トラックではこれらが透過ラップ／型付き化を要する。

## Reassess When

- 一般の param / return やその他の出現位置を既定 ON にすべき状況（誤検出を許容できる運用が定着した等）になった場合（D7 の既定 config を見直す）。
- `Command` の既定除外が緩すぎると判明した場合（例: `TrackId` 等の型付けしたい入力が生 String で漏れる）。opt-in 個別禁止への切り替えを再検討する。
- `ErrorType` の全層対象化が、透過ラップ不能な最下層のエラー源（外部ライブラリの生文字列エラー等）で過剰と判明した場合（domain の `ErrorType` を position / プリミティブ単位で緩める余地を再検討する。ただし外部エラーは newtype ラップで保持する方針を優先する）。
- Ok スロットの String（`Result<String, E>`）も一律禁止したい需要が出た場合（`result_ok` 位置の追加を検討）。
- 自作ジェネリックコンテナを含む型引数プリミティブは D2 の全再帰走査で検出される前提だが、特定コンテナだけを別 position として分類・除外したい需要が出た場合（D7 の位置語彙またはカテゴリ config を拡張する）。
- per-instance escape hatch なしの前提が実運用で破綻した場合（真に型付け不能な自由文フィールドが頻出し、D7 のカテゴリ粒度でも吸収できない等）。
- `(layer × role × 出現位置 × プリミティブ)` の 4 軸では表現しきれない適用範囲の需要（例: 特定型名単位の制御）が出た場合。
- `architecture-rules.json` の layer 構成や role 語彙が変わった場合（既定 config の層名列挙・role 除外が追従して再生成されることを確認する）。

## Related

- `knowledge/adr/` — ADR 索引
- `architecture-rules.json` — layer 名・層構成の SSoT。本 lint の layer 参照はここを参照し、既定 config の層名も同 SSoT から列挙生成する（D6・D7）
- `knowledge/conventions/prefer-type-safe-abstractions.md` — 本 lint が強制する Newtype / enum-first ルール
- `knowledge/conventions/hexagonal-architecture.md` — domain port + infrastructure 実装の分離根拠（D3）、DTO を境界の外部形式とする根拠、および外側の層が下位エラーを透過ラップする根拠（D7）
- `knowledge/adr/2026-05-25-0000-tddd-pattern-semantics-extension.md` — 型カタログ linter framework（opt-in ルール機構 / `RuleTarget` + config）の確立。本 ADR はその framework に新 variant を足し、D7 の粒度 config も同じ config 面に乗せる
- `knowledge/adr/2026-06-21-1420-cli-layers-tddd-and-role-placement-lint.md` — 既存 framework へルールを追加した前例（`KindLayerConstraint` の layer 粒度・`RuleTarget` の role 粒度を config で宣言した事例）
- `knowledge/adr/2026-06-18-0822-typeref-parser-qualified-path-support.md` — カタログ TypeRef を syn でパースする `type_ref_parser` の実装 ADR（D2 が再利用する走査基盤）
- `knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md` — catalogue linter 機構の導入 ADR
