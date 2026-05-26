---
adr_id: 2026-05-26-1002-typestate-struct-kind-orthogonal
decisions:
  - id: D1
    user_decision_ref: "chat_segment:user-bug-report:typestate-struct-kind-orthogonal:2026-05-26"
    status: proposed
---
# typestate は struct の形状と直交する — 全 struct 形状が typestate 状態になれるよう配置を修正する

## Context

### §1 現状の問題: typestate が PlainStruct にしか付かない

ADR `2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` の D3 / D7 で確定した `TypeKindV2` の 5 flat variant 設計では、typestate membership marker (`TypestateMarker`) は `PlainStruct` variant にのみ付与されている。

```
<!-- illustrative, non-canonical -->
enum TypeKindV2 {
    UnitStruct,
    TupleStruct { fields: Vec<TypeRef>, has_stripped_fields: bool },
    PlainStruct  { fields: Vec<FieldDecl>, has_stripped_fields: bool,
                   typestate: Option<TypestateMarker> },   // ← typestate はここだけ
    Enum { variants: Vec<VariantDecl> },
    TypeAlias { target: TypeRef },
}
```

この設計は `libs/domain/src/tddd/catalogue_v2/composite.rs` に実装されており、コードコメントには「without polluting other struct kinds」と記されている。

### §2 何が問題か: 最もよく使われる typestate パターンが表現できない

Rust の typestate イディオムで **最も多く用いられる**状態型は unit struct である。

```
<!-- illustrative, non-canonical -->
struct Locked;       // unit struct — フィールドなし
struct Unlocked;     // unit struct — フィールドなし

struct SafeBox<S> { inner: T, _state: PhantomData<S> }
```

`Locked` / `Unlocked` はいずれも `UnitStruct` 形状であるが、現行の `TypeKindV2` では `UnitStruct` に `typestate: Option<TypestateMarker>` が存在しない。そのため、これらの型を「typestate の状態として」カタログに宣言できない。

同様の問題は `TupleStruct` にも存在する。`struct Pending(Uuid)` のような tuple struct も typestate 状態として使われるが、現行スキーマはそれを表現できない。

### §3 設計上の経緯と誤り

D3 の flat variant 設計は旧 `TypeKindV2::Struct { pattern: CompositePattern, fields }` が抱えていた問題、すなわち「構造的な形状 (kind) と意味的なパターン (typestate/newtype) を同じ enum で混在させる」というカテゴリーエラーを解消することを目的としていた。

この目的は達成された。しかし、その設計で typestate marker を `PlainStruct` にのみ付与したことは、別の過剰制約を生んでいる: 「typestate の状態は named-field struct でなければならない」という制約は Rust の実際の使われ方と整合しない。typestate の状態型の形状（unit / tuple / plain）は、その型が typestate クラスタに属するかどうかとは **独立した** 問題である。

コメントの「without polluting other struct kinds」という表現が、実は必要な情報の付与を不当に「汚染」と見なしていた。これは make legal states unrepresentable（表現できるべき正しい状態を表現できなくしてしまう）というアンチパターンに当たる。

## Decision

### D1: typestate は struct の形状と直交して配置する

typestate membership marker は struct の形状（unit / tuple / plain）に依存しない。任意の struct 形状が typestate クラスタの状態型になれる。したがって typestate marker の配置は、個別の struct variant ではなく「struct 全体をまとめた位置」に移す。

具体的には、3 つの struct 形状（UnitStruct / TupleStruct / PlainStruct）を同じグループとして扱い、typestate marker をそのグループに対して 1 回だけ付与する構造が目標の姿である。例として `Struct { shape: Unit | Tuple | Plain, typestate: Option<TypestateMarker> }` のような形状だが、**この具体的な Rust 型構造（shape enum の名前、フィールドの持ち方、codec の移行方法など）は type-designer が決定する**。本 ADR が確定するのは次の原則のみである:

- typestate marker は struct 形状（unit / tuple / plain の区別）と直交する位置に置く
- 任意の struct 形状が `typestate: Some(TypestateMarker)` を持てる
- 旧設計が達成したカテゴリーエラーの解消（構造的な形状と DDD 意味論的なパターンの分離）はそのまま維持する

加えて、この変更はスキーマの破壊的変更を許容する。アクティブなトラックのカタログのみを新スキーマへ移行し、非アクティブなトラックのカタログファイルは保護対象として移行しない（トラック間の後方互換性は維持しない）。

なお、この変更後も「UnitStruct はフィールドを持てない」「TupleStruct には named field がない」という各形状の制約は、shape を表す内側の variant 構造によって引き続き保証されなければならない。typestate marker を共通位置に移すことは、形状固有の制約を弱めることを意味しない。

## Rejected Alternatives

### A: typestate フィールドを全 struct variant に個別に追加する

`UnitStruct` / `TupleStruct` / `PlainStruct` それぞれに `typestate: Option<TypestateMarker>` フィールドを追加して、現行の flat variant 構造を維持する案。

却下理由: 3 variant に同じフィールドが分散し、「typestate は全 struct 形状で有効」というルールを schema 構造で表現できず、ドキュメント・codec・linter それぞれで個別に扱う冗長性が生まれる。typestate フィールドを struct 形状をまとめたグループに置くことで、このルールを schema 構造上で一度だけ表現できる。

### B: 現状のまま PlainStruct にのみ typestate を許容し、unit/tuple 状態型を PlainStruct として宣言させる

unit struct な typestate 状態型（`struct Locked;`）を、フィールドを空にした PlainStruct として宣言させる規則を運用ルールとして定める案（`PlainStruct { fields: [], ... }` でも宣言可能なため）。

却下理由: 実際のソース形状（unit struct）と catalogue 宣言の形状（PlainStruct）が乖離する。codec は `rustdoc_types::StructKind::Unit` を受け取ったとき `PlainStruct` として encode しなければならなくなり、codec の論理が歪む。また、A-side（catalogue 宣言）が unit struct を PlainStruct と言い張っている間に C-side（rustdoc 由来）は Unit として評価するため、信号が Yellow/Red になる可能性がある。正確な宣言ができないという codec 側の欠陥を運用ルールで肩代わりさせることになる。

### C: typestate marker を TypeEntry のトップレベルフィールドに置く

`TypeEntry { action, role, kind: TypeKindV2, typestate: Option<TypestateMarker>, ... }` として、typestate marker を kind の外に出す案。

却下理由: `TypeEntry` は struct 系だけでなく Enum / TypeAlias も含む。Enum や TypeAlias に typestate marker フィールドが存在することは意味論的に誤りであり、「Enum + Some(TypestateMarker)」という不正な状態を表現可能にしてしまう。typestate marker の scope は struct 形状に限定されるべきであり、その限定は `TypeKindV2` の構造で encode する。

## Consequences

### 良い影響

- Rust でよく使われる unit struct / tuple struct な typestate 状態型を catalogue で忠実に宣言できるようになる。
- codec が `rustdoc_types::StructKind::Unit` / `Tuple` を受け取ったときに typestate marker を付与できるようになり、A-side と C-side の形状が一致する。
- typestate は struct 形状と直交するというルールが schema 構造で一度だけ表現される（分散しない）。
- カテゴリーエラーの解消（ADR `2026-05-08-0248` D3 が達成した軸分離）はそのまま保たれる。

### 悪い影響

- スキーマの破壊的変更であるため、codec と linter のコードを新しい構造に合わせて更新する実装作業が発生する。非アクティブなトラックのカタログファイルは保護対象であり移行しない。
- 具体的な Rust 型構造の設計（shape enum 等）は type-designer の領域であり、実装はその型設計の確定後に行う。

## Reassess When

- struct 以外の種類（Enum / TypeAlias）も typestate 状態として使うパターンが表面化した場合: typestate marker の scope を struct 外に広げる必要があるか専用 ADR で検討する。
- `TypestateMarker` の意味論（`state_name` / `transitions` の役割）が別の taxonomy 拡張作業によって変わった場合: 本 ADR は配置のみを決定しているため、意味論の変更はその拡張作業側で管理する。
- typestate パターンに struct 以外の carrier 型（例: enum variant で状態を表現する場合）が一般的になった場合: schema 全体の構造を見直す。

## Related

- `knowledge/adr/2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` — 本バグの起源となった flat variant 設計（D3 / D7）。typestate を PlainStruct にのみ付与したことで今回のバグが生じた。本 ADR は同 ADR の D3 / D7 を局所的に修正する。
