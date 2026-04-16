# TDDD-05: Secondary Adapter variant の追加 — infrastructure 層における hexagonal port 実装の検証

## Status

Accepted

## Context

### §1 Track 1 (`domain-serde-ripout-2026-04-15`) からの引継ぎ

ADR `2026-04-14-1531-domain-serde-ripout.md` (Track 1, PR #99 マージ済) で `architecture-rules.json` の `infrastructure.tddd` を `enabled: true` に切り替え、`infrastructure-types.json` に **最小限の初期エントリ** (9 件: `schema_export_codec` の DTO 6 件 + enum 2 件 + error 1 件) を投入した。Track 1 §6 で意図的にスコープを絞った結果、infrastructure 層の TDDD は「domain serde 依存の除去 + dogfood の最小単位」に留まっていた。本 ADR はその後続 Track 2 (`tddd-05-infra-wiring-2026-04-15`) における **本格的な運用化** を担う。

### §2 既存 12 variants の不足

ADR `2026-04-13-1813-tddd-taxonomy-expansion.md` で `TypeDefinitionKind` は 12 種類の variant (`Typestate` / `Enum` / `ValueObject` / `ErrorType` / `SecondaryPort` / `ApplicationService` / `UseCase` / `Interactor` / `Dto` / `Command` / `Query` / `Factory`) に拡張された。これらは domain 層と usecase 層には十分だが、**infrastructure 層の主役である adapter (secondary port を実装する struct) を表す variant が存在しない**。

infrastructure 層には 17 件の trait 実装が存在する (Phase 1 の grep で確認、`Drop` および syn `Visit<'ast>` は除外):

| # | Adapter (struct) | 実装する trait | trait の所属層 | ファイル位置 |
| --- | --- | --- | --- | --- |
| 1 | `SystemReviewHasher` | `ReviewHasher` | usecase | `libs/infrastructure/src/review_v2/hasher.rs:19` |
| 2 | `CodexReviewer` | `Reviewer` | usecase | `libs/infrastructure/src/review_v2/codex_reviewer.rs:147` |
| 3 | `FsReviewStore` | `ReviewReader` | domain | `libs/infrastructure/src/review_v2/persistence/review_store.rs:181` |
| 4 | `FsReviewStore` | `ReviewWriter` | domain | `libs/infrastructure/src/review_v2/persistence/review_store.rs:209` |
| 5 | `FsCommitHashStore` | `CommitHashReader` | domain | `libs/infrastructure/src/review_v2/persistence/commit_hash_store.rs:38` |
| 6 | `FsCommitHashStore` | `CommitHashWriter` | domain | `libs/infrastructure/src/review_v2/persistence/commit_hash_store.rs:72` |
| 7 | `GitDiffGetter` | `DiffGetter` | usecase | `libs/infrastructure/src/review_v2/diff_getter.rs:20` |
| 8 | `InMemoryTrackStore` | `TrackReader` | domain | `libs/infrastructure/src/lib.rs:58` |
| 9 | `InMemoryTrackStore` | `TrackWriter` | domain | `libs/infrastructure/src/lib.rs:68` |
| 10 | `FsTrackStore` | `TrackReader` | domain | `libs/infrastructure/src/track/fs_store.rs:113` |
| 11 | `FsTrackStore` | `TrackWriter` | domain | `libs/infrastructure/src/track/fs_store.rs:119` |
| 12 | `GitShowTrackBlobReader` | `TrackBlobReader` | usecase | `libs/infrastructure/src/verify/merge_gate_adapter.rs:110` |
| 13 | `RustdocSchemaExporter` | `SchemaExporter` | domain | `libs/infrastructure/src/schema_export.rs:36` |
| 14 | `ConchShellParser` | `ShellParser` | domain | `libs/infrastructure/src/shell/conch.rs:28` |
| 15 | `SystemGitRepo` | `WorktreeReader` | domain | `libs/infrastructure/src/git_cli/mod.rs:193` |
| 16 | `SystemGitRepo` | `GitRepository` | infrastructure 内部 | `libs/infrastructure/src/git_cli/mod.rs:170` |
| 17 | `SystemGhClient` | `GhClient` | infrastructure 内部 | `libs/infrastructure/src/gh_cli.rs:169` |

trait の所属層別の内訳:

- domain が所有する secondary port — 9 種類 (`TrackReader`/`Writer`, `ReviewReader`/`Writer`, `CommitHashReader`/`Writer`, `SchemaExporter`, `ShellParser`, `WorktreeReader`)
- usecase が所有する secondary port — 4 種類 (`Reviewer`, `DiffGetter`, `ReviewHasher`, `TrackBlobReader`)
- infrastructure 内部の trait — 2 種類 (`GitRepository`, `GhClient`)

これらの adapter を既存 variant (例: `ValueObject` や `Dto`) で代用すると port 実装としての意味論が失われ、L1 メソッドシグネチャ検証 (port との契約一致) も実行されない。

### §3 `code_profile_builder.rs` の trait 実装フィルタ

`libs/infrastructure/src/code_profile_builder.rs:36` の `build_type_graph` には次のフィルタがある:

```rust
.filter(|i| base_name(i.target_type()) == type_info.name() && i.trait_name().is_none())
```

`i.trait_name().is_none()` の条件により **trait 実装は完全に除外** されており、`TypeNode::methods` には inherent 実装のみが集められる。

#### inherent impl と trait impl の違い

Rust の impl ブロックには 2 種類がある:

```rust
// (A) inherent impl — struct 自身が直接持つメソッド
impl Draft {
    fn publish(self) -> Published { ... }
}

// (B) trait impl — trait の契約を満たすためのメソッド
impl Display for Draft {
    fn fmt(&self, f: &mut Formatter) -> Result { ... }
}
```

`build_type_graph` は (A) のみを `TypeNode::methods` に集め、(B) を捨てている。

#### なぜ trait impl を除外しているか — outgoing 汚染の防止

`TypeNode::outgoing` は「この型の inherent メソッドが返す typestate 遷移先」を記録するフィールドである。inherent impl の戻り値型を調べて、typestate として宣言された型名と照合する:

```rust
impl Draft {
    fn publish(self) -> Published { ... }  // 戻り値 Published → outgoing に追加
    fn archive(self) -> Archived { ... }   // 戻り値 Archived → outgoing に追加
}
// → Draft.outgoing = {Published, Archived}
```

もし trait impl を outgoing 計算に混ぜると、trait メソッドの戻り値型が遷移先として誤検出される:

```rust
impl Display for Draft {
    fn fmt(&self, f: &mut Formatter) -> Result { ... }
}
// → Result が outgoing に混入してしまう (これは typestate 遷移ではない)
```

`Display::fmt` の戻り値 `Result` は typestate 遷移とは無関係であり、混入すると `evaluate_typestate` が誤った signal を返す。これがフィルタの存在理由である (line 22-23 のドキュメントコメント "Trait impls are excluded so that transition detection focuses on the type's own behaviour" 参照)。

#### Adapter variant への含意

`SecondaryAdapter` の順方向チェックでは「`FsReviewStore` は `ReviewReader` を impl しているか?」を確認するため、trait impl の情報が必要になる。しかし outgoing 計算には trait impl を混ぜたくない。この矛盾を解決するのが D3 / D4 の Strategy S1 である。inherent impl は従来通り `TypeNode::methods` に集めて outgoing 計算に使い、trait impl は新フィールド `TypeNode::trait_impls` に隔離して `SecondaryAdapter` 評価にのみ使う。

### §4 既存 `schema_export.rs` における trait 名抽出パターン

`libs/infrastructure/src/schema_export.rs:157-167` で trait 実装の trait 名は **既に取得済み** であり、`ImplInfo::new(target, trait_name, methods)` の形で domain `ImplInfo` に格納されている:

```rust
if let ItemEnum::Impl(i) = &item.inner {
    if i.is_synthetic || i.blanket_impl.is_some() {
        continue;
    }
    let target = format_type(&i.for_);
    let trait_name = i.trait_.as_ref().map(|p| p.path.clone());
    let methods = extract_methods(&i.items, krate);
    if !methods.is_empty() || trait_name.is_some() {
        impls.push(ImplInfo::new(target, trait_name, methods));
    }
    continue;
}
```

つまり「rustdoc JSON → `domain::schema::ImplInfo`」の経路は完成しており、`SchemaExport::impls()` から trait 実装を逐次参照できる。`build_type_graph` で意図的に除外されているのは「`TypeNode::methods` への取り込み」だけであり、trait 実装の情報自体は schema 層に既に到達している。

#### `is_negative` フィールドについて

Rust の **negative impl** (否定実装) とは、「この型はこの trait を実装しない」と明示的に宣言する構文である:

```rust
impl !Send for MyType {}  // MyType は Send を実装しないことを明示
```

auto-trait (`Send`, `Sync`, `Unpin` 等) はコンパイラが条件を満たす型に自動で実装するが、negative impl でそれを明示的に拒否できる。`Rc<T>` や `*mut T` のように、スレッド安全性を意図的に持たせたくない型で使われる。

rustdoc JSON ではこれが `Impl` 構造体の `is_negative: bool` フィールドとして表現される (researcher 調査 `knowledge/research/2026-04-16-tddd-05-rustdoc-impl.md` 参照)。`SecondaryAdapter` の順方向チェックで trait impl を検索する際、negative impl は「trait を実装しない」という宣言なので除外すべき対象である。既存の `schema_export.rs:158` では `is_synthetic` と `blanket_impl` はフィルタ済みだが、`is_negative` はフィルタに含まれていない。

`is_negative` フィールドが rustdoc-types 0.57.3 に実際に存在するかは、実装中に確認する必要がある (コードベース内に使用例がないため、researcher の残課題として `knowledge/research/2026-04-16-tddd-05-rustdoc-impl.md` の Addendum に記載)。

### §5 `catalogue_codec.rs` の duplicate name 検証制約

`libs/infrastructure/src/tddd/catalogue_codec.rs:261-322` の duplicate name 検証は次の 4 段階を強制する:

1. 同一 `name` は最大 2 件まで (line 275-282)
2. 2 件ある場合は厳密に「1 件の delete + 1 件の add」(line 284-294)
3. delete + add のペアは **異なる kind** であること (line 296-305)
4. delete + add のペアは **trait / non-trait 区分を跨ぐ** こと (line 306-319)

これにより「同じ name で同じ kind の重複」は構造的に禁止されている。本 ADR の Adapter variant 設計 (D2) は、この制約を保ったまま、複数の trait を実装する adapter を表現する必要がある。

### §6 layer-agnostic の不変条件

ADR `2026-04-11-0002-tddd-multilayer-extension.md` の D6 で確立された通り、`libs/domain/src/tddd/` (TDDD のコア部) は **層に関する情報を一切持たない (layer-agnostic)** 設計であり、`"domain"` や `"usecase"` といった層名をハードコードしない。本 ADR の variant 設計はこの不変条件を継承する。

## Decision

### D1: 新 variant `SecondaryAdapter` を `TypeDefinitionKind` に追加する

`libs/domain/src/tddd/catalogue.rs` の `TypeDefinitionKind` enum に新しい variant を追加する:

```rust
/// hexagonal の secondary (driven) port trait を 1 つ以上実装する struct を表す。
SecondaryAdapter {
    /// この adapter が実装することを期待される trait のリスト。
    implements: Vec<TraitImplDecl>,
},
```

`kind_tag` は `"secondary_adapter"` とする (snake_case の既存パターンに合わせる)。

`SecondaryAdapter` は struct であるため、`consistency.rs:170-195` の type / trait 区分では **type 側に分類** される (既存のフィルタ `matches!(e.kind(), TypeDefinitionKind::SecondaryPort { .. } | TypeDefinitionKind::ApplicationService { .. })` の補集合に自動的に含まれる)。`SecondaryPort` / `ApplicationService` (trait 側) と `SecondaryAdapter` (type 側) が対称をなすことで、port の所有者と adapter の所有者を同じカタログ内で表現できるようになる。

### D2: `implements: Vec<TraitImplDecl>` で 1 エントリに複数 trait を持たせる

複数の trait を実装する adapter (例: `FsReviewStore: ReviewReader + ReviewWriter` をはじめ 8 件) を **1 エントリ** で表現するため、`implements` を `Vec` として持つ。新しい型を追加する:

```rust
/// `SecondaryAdapter` エントリ内の単一の trait 実装宣言。
///
/// adapter が実装する trait の名前と、必要に応じた期待メソッドシグネチャ (L1 解像度) を保持する。
/// `expected_methods` が空のときは、評価器は「trait 実装の存在のみ」を確認する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitImplDecl {
    /// trait 名 (L1 の最終セグメント短縮名、`::` を含まない)。
    trait_name: String,
    /// 期待するメソッドシグネチャ (省略可能)。空の場合は実装の存在のみ確認する。
    expected_methods: Vec<MethodDeclaration>,
}
```

採用根拠:

1. **`catalogue_codec` の duplicate name 検証 (`catalogue_codec.rs:261-322`) は無変更で通る**。1 つの adapter は 1 エントリのみとなるため name は一意であり、duplicate name の 4 段階チェックはどれも発動しない。
2. **`TypeSignal::type_name` の単一キー前提を維持できる**。signal の評価経路 (`evaluate_type_signals`, `check_type_signals`, `merge_gate`) は無変更で済む。
3. **既存の集約 signal パターンと同型**。「すべての trait の実装が確認できれば Blue、1 つでも未確認なら Red」という形は、`evaluate_enum` / `evaluate_error_type` の「すべての `expected_variants` が見つかれば Blue」と同じ集約パターンであり、既存設計と一貫する。
4. **trait 単位の状態を表現できる**。`TypeSignal::found_items` (実装が確認できた trait 名のリスト) と `missing_items` (未確認の trait 名のリスト) で trait ごとの状態を表現できる。新たなスキーマ拡張は不要。

`expected_methods` を `TraitImplDecl` の内部に持たせる理由:

- adapter 側で実装メソッドのシグネチャを明示できる
- port 側カタログ (`secondary_port` エントリ) との横断参照が不要 — `infrastructure-types.json` 単独で adapter の契約を読み取れる
- ADR `2026-04-13-1813-tddd-taxonomy-expansion.md` の D3 (新 variants は YAGNI の方針で「存在チェックのみ」とする) と整合する。`expected_methods: []` を許容する (省略可能) ため、最小実装から始めて必要に応じてシグネチャ検証を強化できる

### D3: `TypeNode::trait_impls` を追加する (Strategy S1)

`libs/domain/src/schema.rs::TypeNode` に新しいフィールドを追加し、各 type が実装する trait の情報を保持させる:

```rust
/// type に対する単一の trait 実装。
#[derive(Debug, Clone)]
pub struct TraitImplEntry {
    trait_name: String,
    methods: Vec<MethodDeclaration>,
}

// TypeNode に追加するフィールド
trait_impls: Vec<TraitImplEntry>,
```

`TypeGraph` に新しいアクセサを追加する:

```rust
/// 与えられた (type 名, trait 名) のペアに対応する trait 実装エントリを返す。
pub fn get_impl(&self, type_name: &str, trait_name: &str) -> Option<&TraitImplEntry> {
    self.types.get(type_name)?
        .trait_impls()
        .iter()
        .find(|e| e.trait_name() == trait_name)
}
```

採用根拠:

1. **既存 `outgoing` ロジックを保護できる**。`outgoing: HashSet<String>` (typestate 遷移) の計算は inherent メソッドのみを使う設計。trait 実装を別フィールドに格納することで、`outgoing` 計算ロジックには手を入れずに済む。
2. **スキーマ拡張は加法的**。既存の `TypeNode::methods` / `outgoing` / `module_path` には変更がなく、新フィールド追加のみ。
3. **Strategy S2 (別 `ImplGraph`) との比較**: 別管理にすると `TypeGraph` と `ImplGraph` の 2 つのグラフを評価器に渡す必要があり、`check_consistency` / `evaluate_type_signals` / `baseline_builder` の引数構造が変わる。S1 のほうが影響範囲が限定的。
4. **Strategy S3 (`TraitNode::implementors` で逆引き) との比較**: adapter 側からの参照が `TraitNode::implementors` の線形検索になり O(N)。S1 の `get_impl(type, trait)` のほうが直接的。

### D4: `code_profile_builder.rs` の trait 実装フィルタを緩和する

`libs/infrastructure/src/code_profile_builder.rs:36` のフィルタ (`i.trait_name().is_none()`) を解除し、trait 実装を別経路で収集する。outgoing 計算には引き続き inherent メソッドのみを使う:

```rust
// inherent メソッドは従来通り outgoing 計算に使う
let inherent_methods: Vec<&FunctionInfo> = schema.impls().iter()
    .filter(|i| base_name(i.target_type()) == type_info.name() && i.trait_name().is_none())
    .flat_map(|i| i.methods())
    .collect();

// trait 実装は別経路で収集して TypeNode::trait_impls に格納する
let trait_impls: Vec<TraitImplEntry> = schema.impls().iter()
    .filter(|i| base_name(i.target_type()) == type_info.name() && i.trait_name().is_some())
    .map(|i| {
        let trait_name = i.trait_name().expect("filtered above").to_string();
        let methods = i.methods().iter().map(function_info_to_method_decl).collect();
        TraitImplEntry::new(trait_name, methods)
    })
    .collect();
```

`is_synthetic` (auto-trait) と `blanket_impl.is_some()` (ブランケット実装) の除外は **schema_export の段階で既に処理されている** (`schema_export.rs:158` のフィルタで両方とも除外済み) ため、`code_profile_builder.rs` 側で再度除外する必要はない。ただし `is_negative` フィールド (negative impl `impl !Trait for Type` を表す。§4 参照) の存在については、実装中に rustdoc-types 0.57.3 で確認する必要がある (researcher の残課題)。存在が確認された場合は `schema_export.rs:158` の既存フィルタ (`is_synthetic || blanket_impl.is_some()`) に `|| i.is_negative` を追加し、negative impl が trait impl 収集に混入しないようにする。

### D5: infrastructure 内部の trait のみ本 track の対象外とする

`GitRepository` (`git_cli/mod.rs:170` 周辺) と `GhClient` (`gh_cli.rs:169` 周辺) は port の所有者と adapter の所有者が **同じ infrastructure 層** に存在する。Hexagonal Architecture における secondary port (driven port) は **driving 層 (usecase) もしくは domain 層が所有者** であるべきで、infrastructure が自ら定義し自ら実装する trait は「テスト容易性のための抽象境界 (mock boundary)」と意味論的に異なる。

**例外**: `SystemGitRepo` は infrastructure 内部 trait `GitRepository` に加え、domain が所有する `WorktreeReader` も実装している (`git_cli/mod.rs:193`)。`WorktreeReader` は domain port であるため、`SystemGitRepo` の `WorktreeReader` 実装は本 track のカタログ対象に含める。`SystemGitRepo` の `GitRepository` 実装のみを除外する。

`SystemGhClient` の `GhClient` 実装は infrastructure 内部のため除外し、後続 track `tddd-06-cli-wiring-YYYY-MM-DD` で別途 variant を検討する。除外理由は `description` フィールドに明記する運用とする。

### D6: layer-agnostic の不変条件を維持する

`TraitImplDecl` および `SecondaryAdapter` variant に `trait_layer: "domain" | "usecase"` のような層名をハードコードしない。trait の所属層情報が必要な場合は description フィールドに記述する (運用ルール、構造化はしない)。

ADR `2026-04-11-0002-tddd-multilayer-extension.md` D6 の不変条件 (TDDD のコア部 `libs/domain/src/tddd/` は層に依らない) を継承する。

## Rejected Alternatives

### A. Option B: 既存 `ValueObject` で代用する案

却下理由:

- adapter が hexagonal port 実装であるという意味論的な情報が失われる
- L1 メソッドシグネチャ検証 (port との契約一致) が実行されず、原始型への退化 (primitive obsession) などを検出できない
- `infrastructure-types.json` を読む人が「これは adapter なのか、それとも汎用の値オブジェクトなのか」をカタログから判別できない

### B. Option C: 既存 `Dto` で代用する案

却下理由:

- `Dto` は behavior を持たないデータ容器であり、impl ブロックを持つ adapter とは意味的に矛盾する
- ADR `2026-04-13-1813-tddd-taxonomy-expansion.md` D3 で `Dto` は「層境界を越える純粋データ容器」と明確に定義されており、adapter は該当しない

### C. N エントリ方式 (1 つの adapter × N 個の trait → N エントリ)

`FsReviewStore` を `secondary_adapter, implements: ReviewReader` と `secondary_adapter, implements: ReviewWriter` の 2 エントリで書く案。

却下理由:

- `catalogue_codec.rs:261-322` の duplicate name 検証 (同一 name は最大 2 件 + delete + add のみ + 異なる kind) と矛盾する
- `kind_a == kind_b == "secondary_adapter"` となるため line 296-305 で `InvalidEntry` になる
- 検証ロジックを複合キー (`(name, kind, implements)`) に拡張する案 (下記 Option D-1) は影響範囲が大きすぎる

### D. Option D-1: codec の duplicate validation を複合キーに拡張する案

`(name, kind_tag, implements)` を一意性のキーとし、`secondary_adapter` の場合は delete + add ペアのチェックを skip する案。

却下理由:

1. `TypeSignal::type_name: String` (`catalogue.rs:454-467`) は signal を識別する単一キーとして設計されている。複合キー化は signal 評価経路全体の書き換えを伴う
2. `TypeCatalogueDocument::signals` (`catalogue.rs:549`) は `Vec<TypeSignal>` を `Option` で持ち、`set_signals` (`catalogue.rs:578`) は entries との 1:1 対応を暗黙の前提としている
3. 影響を受けるファイル数: `catalogue.rs` / `signals.rs` / `consistency.rs` / `catalogue_codec.rs` / `baseline_codec.rs` / cli 側の signal 書き出しコード — Track 2 のスコープを大幅に超過する

### E. Option D-3: 代表 trait のみ宣言する案 (`implements: String` 単一)

1 つの adapter につき 1 エントリ、`implements: String` (1 つの trait のみ宣言) とする案。

却下理由:

- 複数の port を実装する adapter (`FsReviewStore: ReviewReader + ReviewWriter`, `FsCommitHashStore: ...Reader + ...Writer`, `InMemoryTrackStore: ...`, `FsTrackStore: ...` の合計 8 件) で、片方の trait しか TDDD 検証されず、もう片方は検証の穴になる
- テンプレートとしての品質を損なう (本番運用のカバレッジが不十分)

### F. Option D-4: 合成された name (`FsReviewStore::ReviewReader`) を使う案

カタログエントリの name を `FsReviewStore::ReviewReader` のように合成する案。

却下理由:

- `evaluate_secondary_adapter` の順方向チェックで `profile.get_type(name)` を呼ぶ際、合成された name では型検索が失敗する
- 評価器内で name から struct 名を分割する特殊処理が必要になり、文字列パースが domain 層に混入する → layer-agnostic 原則 (ADR 0002 D6) と矛盾する
- カタログの読みやすさが落ちる (struct 名とエントリ名が一致しなくなる)

### G. Strategy S2: 別 `ImplGraph` (TypeGraph と分離管理)

`TypeGraph` には trait_impls を持たせず、別の関数 `build_impl_graph(schema) -> ImplGraph` で trait 実装を別管理する案。

却下理由:

- `check_consistency` / `evaluate_type_signals` の引数構造が変わる (`TypeGraph` だけでなく `ImplGraph` も渡す必要が生じる)
- `baseline_builder.rs` / `baseline_codec.rs` も変更が必要になる
- リグレッションのリスクが S1 より高い

### H. Strategy S3: `TraitNode::implementors` による逆引き

`TraitNode` に `implementors: Vec<String>` (その trait を実装する struct 名のリスト) を追加し、trait 側から adapter 名を逆引きする案。

却下理由:

- adapter 側からの検索が `TraitNode::implementors` の線形検索になり O(N)
- `evaluate_secondary_adapter` の順方向チェックは「この struct はこの trait を実装しているか?」を struct → trait の方向で確認するため、`TypeNode::trait_impls` のほうが直接的
- 将来的に「この port を実装する adapter がカタログに存在するか?」という逆方向チェックが必要になった場合に S3 は有用かもしれないが、Track 2 のスコープ外

### I. infrastructure 内部の trait をカタログに含める案 (Q2 の B1-B3)

`GitRepository` / `GhClient` を `secondary_port` + `secondary_adapter` でカタログ化する案 (B1)、または新 variant `internal_port_pair` で表現する案 (B2)、または adapter のみ ValueObject で代用する案 (B3)。

却下理由:

- port の所有者と adapter の所有者が同じ infrastructure 層に存在し、hexagonal の secondary port の意味論と異なる
- 内部 trait は「テスト容易性のための抽象境界」であり、port 抽象とは目的が異なる
- 本 track の主目的 (domain / usecase が所有する port の adapter 検証) を複雑化する
- 後続 track `tddd-06-cli-wiring` で別途扱う

## Consequences

### Good

- **本番カバレッジの向上**: infrastructure 層の 11 件の adapter (17 件の trait impl 行 - 内部 trait 2 件 = 15 行、D2 の Vec<TraitImplDecl> で adapter 単位に集約すると 11 unique entries) がカタログ化され、TDDD の検証対象になる
- **複数の port を実装する adapter を自然に表現できる**: `FsReviewStore: ReviewReader + ReviewWriter` のような複数 port 実装を 1 エントリで表現可能
- **既存の codec / signal 評価経路は無変更**: `catalogue_codec.rs` の duplicate name 検証、`TypeSignal::type_name` の単一キー前提、`TypeCatalogueDocument::signals` の 1:1 対応 — いずれも変更不要
- **スキーマ拡張は最小限**: `TraitImplEntry` の新型と `TypeNode::trait_impls` フィールドと `TypeGraph::get_impl` アクセサのみ
- **既存の trait 名抽出パターンを流用できる**: `schema_export.rs:158-167` の `i.trait_.as_ref().map(|p| p.path.clone())` パターンは実証済みで、再利用可能
- **アクティブでない track のデータには影響しない**: active-track guard (ADR `2026-04-15-1012-catalogue-active-guard-fix.md` D1) により、`sotp track type-signals` は completed / archived track に対して実行を拒否する。`SecondaryAdapter` variant の追加や evaluator の変更は active track にのみ影響する。各 track は独自のカタログ / baseline / signal を持つため、他の track との後方互換性を考慮する必要はない
- **Strategy S1 で outgoing 計算は影響を受けない**: trait 実装を別フィールドに格納するため、typestate 遷移検出 (`evaluate_typestate`) は変更なし
- **layer-agnostic を維持**: `TraitImplDecl` に層名をハードコードしない選択により ADR 0002 D6 と整合する

### Bad

- **trait ごとの状態を 1 signal に集約することの制約**: 複数の trait を実装する adapter で「どの trait がミスマッチしているか」は `missing_items: Vec<String>` のリストでのみ表現される。`TypeSignal` 自体は 1 つに集約される。詳細は rendered view (`<layer>-types.md`) で確認する運用
- **`evaluate_impl_methods` という新ヘルパーが必要**: `evaluate_trait_methods` (`signals.rs`) は `profile.get_trait(name)` を呼ぶため直接は再利用できない。ただしメソッド一致判定 (`method_structurally_matches`) は再利用可能
- **`code_profile_builder.rs` のフィルタ緩和に伴うテスト**: 既存テスト `test_build_type_graph_with_trait_impl_excludes_outgoing` は trait 実装が outgoing に流入しないことを保証している。フィルタ緩和後もこのテストを維持し、新規テスト `test_build_type_graph_trait_impl_populated` で `trait_impls` への格納を検証する
- **infrastructure 内部の trait はカバーされない**: `SystemGitRepo` / `SystemGhClient` は本 track の対象外。後続 `tddd-06` で対応する
- **delete アクションの意味論的な隙間**: `evaluate_delete` は `SecondaryAdapter` の場合 `profile.get_type(name)` で struct 自身の存在確認のみを行う (struct が消えれば impl も消えるため一体的に扱う)。impl 単独の削除を検出するには別途逆方向チェックが必要だが、本 track のスコープ外

### Neutral

- **`is_negative` の確認**: rustdoc-types 0.57.3 に `is_negative` フィールドが存在するかを実装中に確認する (researcher の残課題、`knowledge/research/2026-04-16-tddd-05-rustdoc-impl.md` の Addendum を参照)
- **`TypeBaselineEntry` への影響なし**: baseline の比較対象は inherent メソッドのみであり、trait 実装は baseline に保存しない (planner の Q3 推奨)
- **`is_method_bearing` (catalogue_codec.rs:307) に `secondary_adapter` を追加しないこと**: `secondary_adapter` は type 区分 (struct) であるため `is_method_bearing` には追加しない。delete + add ペアは trait / non-trait 区分を跨ぐ必要があるが、`secondary_adapter` は non-trait 側に分類される
- **既存 12 → 13 variants**: ADR `2026-04-13-1813-tddd-taxonomy-expansion.md` の 5 → 12 拡張パターン (D1) を踏襲する加法的拡張

## Reassess When

1. **rustdoc-types の `Item::Impl` 構造が変わった場合**: nightly の仕様変動、`p.path` の意味論的変化、新フィールドの追加など。`schema_export.rs:158-167` のパターンが壊れた時点で再評価する
2. **infrastructure 内部 trait のカタログ化要求が出た場合**: `tddd-06-cli-wiring` などの後続 track で `GitRepository` / `GhClient` をカタログ化する設計判断が必要になった場合、B1 / B2 / 新 variant などを再評価する
3. **複数 trait を実装する adapter で trait ごとの粒度の signal が必要になった場合**: `missing_items` リストでは表現しきれない要求 (例: trait ごとに Yellow / Red を区別したい) が出た場合、`schema_version` 3 と複合キー導入を再評価する
4. **L2 (generics / lifetime) 検証導入時**: ADR `2026-04-11-0002-tddd-multilayer-extension.md` の L2 拡張 (generics や bounds の検証) が実装される時、`TraitImplDecl::expected_methods` の `MethodDeclaration` 拡張に追従する
5. **`TraitNode::implementors` (Strategy S3) の必要性**: 「この port を実装する adapter がカタログに存在しない」を逆方向チェックで検出する要求が出た場合、S3 の追加導入を再評価する
6. **Adapter variant 固有の検証ルール強化**: 例えば「adapter は対応する port の **すべてのメソッド** を実装すること」のような順方向チェックルールを追加する場合、本 ADR の `expected_methods: optional` の方針を見直す

## References

- ADR `2026-04-11-0002-tddd-multilayer-extension.md` — TDDD multilayer の SSoT、D6 の layer-agnostic 不変条件
- ADR `2026-04-13-1813-tddd-taxonomy-expansion.md` — 5 → 12 variants 拡張パターン (本 ADR の 12 → 13 拡張の前例)
- ADR `2026-04-11-0001-baseline-reverse-signals.md` — 4 グループ評価 (A\B / A∩B / B\A / ∁(A∪B)∩C)
- ADR `2026-04-11-0003-type-action-declarations.md` — TypeAction (add / modify / delete / reference) との整合
- ADR `2026-04-12-1200-strict-spec-signal-gate-v2.md` — strict merge gate (Yellow がマージをブロック)
- ADR `2026-04-14-1531-domain-serde-ripout.md` — Track 1 の親 ADR、§D1 の ADR-first 原則
- ADR `2026-04-15-1012-catalogue-active-guard-fix.md` — Track 1 後の構造変更 (active-track guard / multi-layer sync)
- `libs/infrastructure/src/code_profile_builder.rs:36` — D4 の trait 実装フィルタ緩和の対象
- `libs/infrastructure/src/schema_export.rs:158-167` — D4 で流用する trait 名抽出パターン (既存)
- `libs/infrastructure/src/tddd/catalogue_codec.rs:261-322` — D2 採用根拠 (duplicate name 検証制約)
- `libs/domain/src/tddd/catalogue.rs` — `TypeDefinitionKind` enum と `TraitImplDecl` の追加対象
- `libs/domain/src/tddd/signals.rs` — `evaluate_secondary_adapter` の追加対象
- `libs/domain/src/schema.rs` — `TraitImplEntry` 新型と `TypeNode::trait_impls` / `TypeGraph::get_impl` の追加対象
- `knowledge/research/2026-04-16-tddd-05-rustdoc-impl.md` — rustdoc Item::Impl 構造調査 (researcher 出力)
- `knowledge/conventions/hexagonal-architecture.md` — port の配置と adapter のルール
- `knowledge/conventions/source-attribution.md` — signal のソースタグ付け
- `knowledge/conventions/typed-deserialization.md` — codec の DTO 設計原則
- `knowledge/conventions/prefer-type-safe-abstractions.md` — 型システムでバグクラスを排除する原則
- `knowledge/conventions/nightly-dev-tool.md` — rustdoc nightly 利用ルール
