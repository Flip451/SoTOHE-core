# Catalogue Schema Reference (v5)

## Purpose

TDDD 型カタログ v5 の schema 詳細リファレンス。wire format / role 語彙 / kind・shape 表現 / lint rule kinds / JSON 例を一箇所に集約する。

生成 + 注釈 workflow (ADR `2026-07-02-1345-catalogue-generation-annotation.md` D2 / D9) では、カタログ JSON の骨格は `sotp catalog` コマンド群が生成し、type-designer は意図の入力と `$todo` 穴埋めだけを行う。schema の深い理解は執筆に不要になったが、schema 情報そのものは以下の場面で引き続き必要であり、本書がその受け皿になる (D9):

- 生成された穴埋め済みエントリの**読解**
- `$todo` 残存箇所への**記入内容の判断**
- 生成物の**保守・デバッグ** (signal 🟡/🔴 の原因調査を含む)
- 生成コマンドを経ない**手直し編集**の正確性確認

> **Authority note (D9)**: schema の権威は sotp の実装側にある。本書は記述的な参照文書 (descriptive mirror) であり、canonical SSoT はソースコード — 具体的には `libs/domain/src/tddd/catalogue_v2/` (`CatalogueDocument`, `TypeEntry`, `TraitEntry`, `FunctionEntry`, `TraitImplDeclV2`, `InherentImplDeclV2`) と `libs/infrastructure/src/tddd/catalogue_document_codec/` — である。本書の記述と実装が乖離した場合は **sotp 側を正とし**、本書を修正する。記述が疑わしいときは source を読み、Open Question として報告する。

## Scope

- Applies to: `track/items/<id>/<layer>-types.json` (schema_version 5) を読む・注釈する・手直しする・レビューするすべての場面。type-designer capability / reviewer / orchestrator / 人間の開発者。
- Does not apply to: workflow 手順そのもの (生成 + 注釈の実行手順は `.harness/capabilities/type-designer.md` が SSoT)。role / kind の**選定判断** (`knowledge/conventions/type-designer-kind-selection.md` が SSoT)。

## Document structure

Catalogue files for this workspace use **`schema_version: 5`** — a 2-axis structure that separates the architectural **role** (DDD / Clean Architecture intent) from the language-level **kind** (Rust syntactic form). The top-level document is **3 BTreeMaps** (one per item kind) plus **2 top-level arrays** that hold impl blocks as independent entries:

```json
{
  "schema_version": 5,
  "crate_name": "<this-crate>",
  "layer":       "<this-crate>",
  "types":          { "<TypeName>":     <TypeEntry>     },
  "traits":         { "<TraitName>":    <TraitEntry>    },
  "functions":      { "<FunctionPath>": <FunctionEntry> },
  "inherent_impls": [<InherentImplDeclV2>, ...],
  "trait_impls":    [<TraitImplDeclV2>,    ...]
}
```

`inherent_impls` / `trait_impls` are **top-level arrays**, not fields of `TypeEntry`. Each entry is an independent catalogue entry — it is NOT attached to the `TypeEntry` of the implementing type. For `trait_impls` (trait impl blocks, `impl Trait for Type`), the entry uses `for_type` to name the implementing type and `trait_ref` to name the trait; the symmetry lets cross-crate impls whose self type is external (e.g. `impl MyTrait for std::vec::Vec<i32>`) be declared even though no `TypeEntry` exists for the external self type. For `inherent_impls` (inherent impl blocks, `impl Type`), the entry uses `type_name` to identify the implementing struct.

`<this-crate>` is one of the crate names listed in `architecture-rules.json` (e.g. one of this workspace's layered crates). By convention `crate_name == layer` for tracked workspace catalogues.

**`sotp catalog init` 直後の skeleton**: 生成直後のカタログには 6 top-level キーのみが並ぶ — `schema_version` / `crate_name` / `layer` / `types` / `traits` / `functions` (section map は空)。`inherent_impls` / `trait_impls` の 2 top-level array は「空なら省略」規約に従い、entry が追加されるまで**キー自体が現れない**。これは正常な形であり、手で空配列を補う必要はない。

## TypeEntry (under `types: { ... }`)

```json
{
  "action": "add" | "modify" | "reference" | "delete",
  "role":   { "<DataRoleVariant>": { <payload fields if any> } },
  "kind":   { "kind": "<struct|enum|type_alias>", ... },
  "methods":           [<MethodDeclaration>, ...],
  "module_path":       "<path::segments>",
  "docs":              "<optional docstring>" | null,
  "spec_refs":         [<SpecRef>, ...],
  "informal_grounds":  [<InformalGroundRef>, ...]
}
```

`role` MUST be one of the **17 type-section role values**, written in **discriminated-object form** because `DataRole` is a data-carrying enum:

| Variant | Wire form | Notes |
|---|---|---|
| `ValueObject` | `{ "ValueObject": { "invariants": [<InvariantDecl>...] } }` | `invariants` is `#[serde(default)]` → may be omitted ⇔ `{ "ValueObject": {} }` |
| `Entity` | `{ "Entity": { "identity": <IdentityAccessor>, "invariants": [...] } }` | `identity` is **required** (no default) |
| `AggregateRoot` | `{ "AggregateRoot": { "identity": <IdentityAccessor>, "invariants": [...], "exclusive_members": ["<TypeRef>"...], "shared_value_objects": ["<TypeRef>"...], "emits": ["<TypeRef>"...] } }` | `identity` required; other Vec fields default to `[]` |
| `DomainService` | `{ "DomainService": { "emits": ["<TypeRef>"...] } }` | `emits` defaults to `[]` ⇔ `{ "DomainService": {} }` |
| `UseCase` | `{ "UseCase": { "handles": ["<TypeRef>"...] } }` | `handles` defaults to `[]` ⇔ `{ "UseCase": {} }` |
| `EventPolicy` | `{ "EventPolicy": { "reacts_to": ["<TypeRef>", ...] } }` | `reacts_to` is **required and must be non-empty** (`NonEmptyVec` invariant — empty array is a decode error) |
| `DomainEvent` | `{ "DomainEvent": {} }` | unit variant — payload-free event role (Stage 2) |
| `Specification` / `Factory` / `Interactor` / `Command` / `Query` / `Dto` / `ErrorType` / `SecondaryAdapter` / `CompositionRoot` / `PrimaryAdapter` | `{ "<Variant>": {} }` | unit variants — always write the empty object payload. `CompositionRoot` is permitted only in `cli_composition`; `PrimaryAdapter` is permitted only in `cli_driver` (ADR 2026-06-21-1420 D2 / D3) |

Using a trait-section or function-section role here is a parse-time error.

`IdentityAccessor` shape: `{ "method_name": "<MethodName>" }` (a public getter method name; public field identity is forbidden — D5). The Rust type is a single-field struct holding a `MethodName`.

`InvariantDecl` shape: `{ "name": "<InvariantName>", "predicate": { "SelfMethod": "<MethodName>" } }`. `InvariantName` is a `String`-backed newtype (non-empty, identifier-validated). `InvariantPredicate` is an enum whose only current variant is `SelfMethod(MethodName)`; future predicate kinds add new variants.

`NonEmptyVec<T>` (used by `EventPolicy.reacts_to` and several linter rule kinds): a domain newtype around `Vec<T>` that rejects empty arrays at construction. The codec decode for `reacts_to: []` returns `InvalidEntry`.

`RoleKind` (payload-free discriminant): an enum that covers every `DataRole`, `ContractRole`, and `FunctionRole` variant (17 + 4 + 2 = 23 variants — `FunctionRole::{FreeFunction, UseCaseFunction}` were added in ADR 2026-06-21-1420 T001 so that the lint framework's `KindLayerConstraint` can scan function entries as well as type / trait entries). It is used by linter rule kinds whose payload references roles (`forbidden_roles`, `expected_role`) without needing the data-carrying payload. `RuleTarget` is a struct that holds `target_roles: Vec<RoleKind>` and selects which catalogue entries a `CatalogueLinterRule` applies to.

**The plain-string role form (`"role": "ValueObject"` etc.) is no longer accepted** — the codec rejects it as a parse-time error. The discriminated-object form above is mandatory.

**For `LintRuleSpec` authors**: field-vector / type-ref rule kinds that use the carry precheck (`FieldEmpty`, `FieldNonEmpty`, `ReferencedRoleConstraint`, `FieldElementUniqueAcrossEntries`, `NoExternalReferenceInMethods`) are valid only when **every** selected `target_role` actually carries the rule's `target_field` in its payload. For example `FieldNonEmpty { target_field: "emits" }` with `target_roles: ["Entity"]` is `InvalidRuleConfig` — `Entity` does not carry `emits`. `MethodReferenceSignature` only supports `target_field: "invariants"` and checks entries whose role carries invariants; `AccessorSignatureRequired` only supports `target_field: "identity"` and checks entries whose role carries identity. The carry-relationship is fixed by the role wire-form table above: `invariants` → `ValueObject` / `Entity` / `AggregateRoot`; `identity` → `Entity` / `AggregateRoot`; `exclusive_members` / `shared_value_objects` → `AggregateRoot`; `emits` → `AggregateRoot` / `DomainService`; `handles` → `UseCase`; `reacts_to` → `EventPolicy`; `aggregate` → `Repository`.

### TraitImplDeclV2 (each element of the top-level `trait_impls` array)

```json
{
  "action":    "add" | "modify" | "reference" | "delete",
  "trait_ref": "<TypeRef>",
  "for_type":  "<TypeRef>"
}
```

or with impl-block-level generics:

```json
{
  "action":                "add" | "modify" | "reference" | "delete",
  "trait_ref":             "<TypeRef>",
  "for_type":              "<TypeRef>",
  "impl_generics":         [<MethodGenericParam>, ...],
  "impl_where_predicates": [<WherePredicateDecl>, ...]
}
```

- `action` — the TDDD operation for this impl entry (`"add"` / `"modify"` / `"reference"` / `"delete"`). **Defaults to `"add"`** (the codec uses `#[serde(default = "default_action")]`), so it may be omitted when `Add` is intended (the common case for new impls). Every `trait_impls` entry carries its own `action` — as a top-level independent entry with no parent `TypeEntry`, the action is not inherited.
- `trait_ref` — the trait reference as a TypeRef string, **including** the generic args if any (e.g. `"core::convert::From<MyError>"`, `"std::fmt::Display"`, `"FnOnce<(A,), B>"`). Self-crate traits use the bare short name (`"MyTrait"`); external crate traits use a crate-prefixed fully-qualified path. The crate-prefix convention is the same as for any TypeRef (external crate items carry a crate prefix; self-crate items do not), so the A-codec resolves the trait crate via the standard `external_crates` auto-build.
- `for_type` — the self type of the impl (the `Type` in `impl Trait for Type`) as a TypeRef string. Self-crate types use the bare short name (e.g. `"SelfType"`); external crate types use a crate-prefixed fully-qualified path (e.g. `"std::vec::Vec<i32>"`). Because the impl is a top-level entry (not attached to a `TypeEntry`), an external self type needs no `TypeEntry` to be declared.
- `impl_generics` — optional array of impl-block-level generic type parameters (`impl<L, R> Trait for Foo<L, R>` → entries for `L`, `R`). **Omit when empty** (DTO uses `#[serde(default, skip_serializing_if = "Vec::is_empty")]`).
- `impl_where_predicates` — optional array of impl-block-level where-clause predicates on `impl_generics`. **Omit when empty.**

### InherentImplDeclV2 (each element of the top-level `inherent_impls` array)

```json
{
  "type_name":  "<TypeName>",
  "methods":    [<MethodDeclaration>, ...]
}
```

or with impl-block-level generics:

```json
{
  "type_name":             "<TypeName>",
  "impl_generics":         [<MethodGenericParam>, ...],
  "impl_where_predicates": [<WherePredicateDecl>, ...],
  "methods":               [<MethodDeclaration>, ...]
}
```

- `type_name` — the name of the type this impl block belongs to. Multiple `InherentImplDeclV2` entries sharing the same `type_name` represent multiple inherent `impl` blocks for one struct in the source.
- `methods` — method declarations inside this impl block. **Omit or set to `[]` when empty.**
- `impl_generics` — optional impl-block-level generic type parameters. **Omit when empty.**
- `impl_where_predicates` — optional impl-block-level where-clause predicates. **Omit when empty.**

**Key difference from `trait_impls`**: `InherentImplDeclV2` has **no `action` field**. The DTO uses `#[serde(deny_unknown_fields)]`, so writing `"action": "add"` on an `inherent_impls` entry will be rejected by the codec. Do not add `action` to inherent impl entries.

## TraitEntry (under `traits: { ... }`)

```json
{
  "action":           "add" | "modify" | "reference" | "delete",
  "role":             { "<ContractRoleVariant>": { <payload fields if any> } },
  "methods":          [<MethodDeclaration>, ...],
  "supertrait_bounds":["<TypeRef>", ...],
  "module_path":      "<path::segments>",
  "docs":             "<optional docstring>" | null,
  "spec_refs":        [<SpecRef>, ...],
  "informal_grounds": [<InformalGroundRef>, ...]
}
```

`role` MUST be one of the **4 trait-section role values**, written in **discriminated-object form** because `ContractRole` is a data-carrying enum:

| Variant | Wire form | Notes |
|---|---|---|
| `SpecificationPort` | `{ "SpecificationPort": {} }` | unit — always empty object payload |
| `ApplicationService` | `{ "ApplicationService": {} }` | unit |
| `SecondaryPort` | `{ "SecondaryPort": {} }` | unit (non-Repository secondary port) |
| `Repository` | `{ "Repository": { "aggregate": "<TypeRef>" } }` | `aggregate` is **required** — names the AggregateRoot type this Repository persists; no default (a Repository without an aggregate is an illegal state — D10) |

Using a type-section or function-section role here is a parse-time error. The plain-string form (`"role": "SpecificationPort"` etc.) is no longer accepted — the codec rejects it as a parse-time error.

## FunctionEntry (under `functions: { ... }`)

```json
{
  "action":            "add" | "modify" | "reference" | "delete",
  "role":              "<function-section role value>",
  "params":            [{ "name": "<ParamName>", "ty": "<TypeRef>" }, ...],
  "returns":           "<TypeRef>",
  "is_async":          true | false,
  "generics":          [{ "name": "<ParamName>", "bounds": ["<TypeRef>", ...] }, ...],
  "where_predicates":  [{ "lhs": "<TypeRef>", "rhs": ["<TypeRef>", ...], "operator": "Bound" | "Equal" }, ...],
  "docs":              "<optional docstring>" | null,
  "spec_refs":         [<SpecRef>, ...],
  "informal_grounds":  [<InformalGroundRef>, ...]
}
```

`role` MUST be one of the **2 function-section role values**: `FreeFunction` | `UseCaseFunction`.

The BTreeMap key is a function path with format `<this-crate>::[<module_path>::]<function_name>` (module segments optional; e.g. `"<this-crate>::register_user"` at crate root, `"<this-crate>::merge_gate::check_strict_merge_gate"` with module). **`<this-crate>` MUST equal the document's own `crate_name`** — the codec rejects any function path key that does not start with `{crate_name}::` (D4).

## The `kind` field (3 top-level discriminators: `struct` / `enum` / `type_alias`)

A struct's Rust-level form (unit / tuple / plain) is carried in a nested `shape`; its typestate membership is an **orthogonal** sibling (`typestate`), so **any** struct shape can be a typestate state. The old `unit_struct` / `tuple_struct` / `plain_struct` wire tags are **removed** (CN-02) — the codec (`deny_unknown_fields`) rejects them; always write `"kind": "struct"` and put the form in `shape`.

```json
// 1. Struct — always `"kind": "struct"`; the `shape` (unit | tuple | plain) is nested.
//    `typestate` is an OPTIONAL sibling of `shape` (omit unless this struct is a typestate state).
"kind": { "kind": "struct", "shape": { "kind": "unit" } }                                                          // pub struct Foo;
"kind": { "kind": "struct", "shape": { "kind": "tuple", "fields": ["<TypeRef>"], "has_stripped_fields": false } }  // pub struct Foo(Bar);
"kind": {                                                                                                          // pub struct Foo { bar: Bar }
  "kind": "struct",
  "shape": { "kind": "plain", "fields": [{ "name": "<FieldName>", "ty": "<TypeRef>" }], "has_stripped_fields": false },
  "typestate": { "state_name": "<TypestateMachineName>", "transition_methods": ["<MethodName>"] }
}

// 2. Enum — `pub enum Foo { Bar, Baz(T), Qux { field: T } }`
"kind": {
  "kind": "enum",
  "variants": [
    { "name": "Bar", "payload": { "kind": "unit" } },          // canonical wire format for Unit variant
    { "name": "Baz", "payload": { "kind": "tuple",  "fields": ["<TypeRef>"] } },
    { "name": "Qux", "payload": { "kind": "struct", "fields": [{ "name": "<FieldName>", "ty": "<TypeRef>" }] } }
  ]
}

// 3. Type alias — `pub type Foo = Bar<Baz>;`
"kind": { "kind": "type_alias", "target": "<TypeRef>" }
```

A `unit` shape carries no `fields` payload at the schema level, so a unit struct with fields is structurally impossible to express. `typestate` and `has_stripped_fields` default to absent/`false` (the codec omits them when unset); write them explicitly only when they apply. The canonical wire format for a Unit enum variant includes `"payload": {"kind": "unit"}`; omitting `payload` is accepted by the decoder (defaults to Unit) but is non-canonical.

### `has_stripped_fields`: private (non-`pub`) fields

rustdoc **omits private fields** from the public API JSON and sets `has_stripped_fields: true` on the C-side struct shape. The catalogue (A-side) MUST mirror this, or the type → source signal stays 🟡 **forever — even after the type is fully implemented** — because the structural-equality evaluator returns `Mismatch` the instant the flag differs (`structural_eq.rs`: `if asf != bsf { return false; }`):

- In `fields`, list **only the `pub` fields** — private fields are absent on both sides, so never list them.
- Set `"has_stripped_fields": true` **iff the struct has ≥1 private field**. Leaving it `false` on a struct that actually has a private field is a permanent 🟡 — the single most common interactor / service-wrapper miss.
- **`tuple` shape caveat**: the codec encodes `has_stripped_fields: true` for a tuple shape by appending a single trailing `None` placeholder to the field vector. Because the catalogue does not record the exact position of each private field, the trailing-`None` representation will mismatch rustdoc's actual `None`-slot layout whenever any private field is not at the trailing position — producing a permanent 🟡. A dependency-holding struct must therefore use a `plain` shape, not a tuple.
- **Never declare the same inherent method in both `TypeEntry.methods` and a top-level `inherent_impls` entry** — the contract-map renderer aggregates inherent methods from both, so a method present in both double-renders. Declare each inherent method once; for interactors / service-wrappers, put the constructor in a top-level `inherent_impls` entry (consistent with generic interactors, whose `impl_generics` can only be expressed via `inherent_impls`).

**Interactor / service-wrapper (the canonical `has_stripped_fields: true` case)** — a struct whose only field is a private injected dependency (`std::sync::Arc<dyn …Port>`, an inner service) has **all** fields private: declare `fields: []` + `has_stripped_fields: true` with `methods: []`, declare the constructor in a top-level `inherent_impls` entry, and declare the implemented ApplicationService as a top-level `trait_impls` entry:

```json
"ActiveTrackResolveInteractor": {
  "action":  "add",
  "role":    { "Interactor": {} },
  "kind":    { "kind": "struct", "shape": { "kind": "plain", "fields": [], "has_stripped_fields": true } },
  "methods": [],
  "module_path": "track_resolution", "docs": null, "spec_refs": [], "informal_grounds": []
}
// + top-level arrays:
//   "inherent_impls": [ { "type_name": "ActiveTrackResolveInteractor", "methods": [
//     { "name": "new", "receiver": null, "params": [{ "name": "branch_reader", "ty": "std::sync::Arc<dyn BranchReaderPort>" }], "returns": "Self", "is_async": false, "generics": [], "has_default_impl": false, "where_predicates": [] } ] } ]
//   "trait_impls":    [ { "trait_ref": "ActiveTrackResolveService", "for_type": "ActiveTrackResolveInteractor" } ]
```

## MethodDeclaration shape

```json
{
  "name": "<MethodName>",
  "receiver": "&self" | "&mut self" | "self" | null,
  "params":   [{ "name": "<ParamName>", "ty": "<TypeRef>" }, ...],
  "returns":  "<TypeRef>",
  "is_async": true | false,
  "generics": [{ "name": "<ParamName>", "bounds": ["<TypeRef>", ...] }, ...],
  "has_default_impl": true | false,
  "where_predicates": [{ "lhs": "<TypeRef>", "rhs": ["<TypeRef>", ...], "operator": "Bound" | "Equal" }, ...],
  "docs": "<optional docstring>" | null
}
```

- `receiver: null` = associated function (no `self`); the valid `receiver` tokens are `"self"`, `"&self"`, `"&mut self"`, and `null` (the codec also accepts `""` as equivalent to `null`). Prefer `null` over `""` for the absence case
- `has_default_impl: true` = trait method has a default body (`fn foo(&self) { ... }`); used by A-codec to set the rustdoc `has_body` flag correctly
- `where_predicates` captures `where Vec<T>: Clone` patterns whose LHS cannot be expressed in `generics[].bounds`. Fields: `lhs` (the constrained type), `rhs` (non-empty bound list), `operator` (`"Bound"` for `T: Trait`, `"Equal"` for `T = Type`; defaults to `"Bound"` when omitted). The legacy `"type"` / `"bounds"` field names are accepted on read for backward compatibility only — always write `lhs` / `rhs` in new entries

## TypeRef rules (`ty` / `returns` / `bounds`)

- **Prefer last-segment names for in-crate types**: e.g. `TrackId` (not `<this-crate>::track::TrackId`) when `TrackId` is defined in the same catalogue's crate. Paths with a `crate::`, `self::`, or `super::` prefix are also resolved as in-crate by the A-codec (it strips the prefix and looks up the last segment). Multi-segment paths that lack these prefixes are treated as cross-crate FQNs — an in-crate type written as a multi-segment path produces an unresolved cross-crate reference instead of resolving locally. The A-codec auto-resolves only a small set of common names; standard-library types such as `String`, `bool`, and `Option` are recognised, but most other types (including types from `std::path`, `std::sync`, etc.) must be referenced by their full path when used across crate boundaries.
- **Use FQN with `::` for cross-crate references**: e.g. `<other-crate>::module::TypeName` for an entry that references a type owned by a different workspace crate. The crate name segment is the catalogue's `crate_name` of the owning crate, as listed in `architecture-rules.json`. For standard-library types not in the auto-resolve set, use the fully-qualified path (e.g. `std::path::PathBuf`). The A-codec's `external_crates` auto-build resolves the FQN to the appropriate `ExternalCrate` entry.
- **Use concrete generics**: `Result<T, E>`, not bare `Result` — bare `Result` passes the codec but loses type information needed for forward-check signal evaluation

## Catalogue Lint Rule Kinds (reference)

The linter (ADR D15 / D17) validates catalogue entries via 12 `CatalogueLinterRuleKind` variants. The type-designer does not author lint configs (that's the user's `.harness/catalogue-lint/config.json`), but knowing which rule kinds exist explains why certain fields are required when a lint is opt-in.

- `FieldEmpty { target_field }` — payload field must be empty
- `FieldNonEmpty { target_field }` — payload field must be non-empty
- `KindLayerConstraint { permitted_layers }` — entry must live in one of the listed layers (used to enforce e.g. EventPolicy is domain-only)
- `ReferencedRoleConstraint { target_field, expected_role }` — every `TypeRef` in the named field resolves to an entry whose role is `expected_role`
- `TraitImplRequired { required_traits }` — `trait_impls` must contain every listed trait reference
- `NoRoleInMethodSignature { forbidden_roles }` — no method param / return may reference a type whose role is in the forbidden list
- `MethodReferenceSignature { target_field }` — the method named in `target_field` exists and matches a receiver / params / returns shape
- `AccessorSignatureRequired { target_field }` — identity getter (or similar) exists with `&self` / no params / non-`()` return
- `FieldElementUniqueAcrossEntries { target_field: "exclusive_members" }` — the same element does not appear in multiple AggregateRoot entries (target_field is fixed to `exclusive_members` per D6/D11)
- `NoExternalReferenceInMethods { target_field: "exclusive_members" }` — types listed in `exclusive_members` must not appear in non-aggregate methods (fixed target_field)
- `NoPublicField` — `StructShape::Plain` / `Tuple` entries must not declare public fields
- `ForbiddenMethodReceiver { forbidden_receiver }` — methods must not declare the listed receiver; canonical values: `"self"` / `"&self"` / `"&mut self"` (anything else is rejected by `CatalogueLinterRule::new` as `CatalogueLinterRuleError::InvalidRuleConfig`)

**Evaluation surface**: method-checking rules (`NoRoleInMethodSignature`, `MethodReferenceSignature`, `AccessorSignatureRequired`, `NoExternalReferenceInMethods`, `ForbiddenMethodReceiver`) walk both `TypeEntry.methods` and matching `inherent_impls` declarations for the same `type_name`. Any entry with `action: delete` is filtered out of role / trait / method lookups before evaluation (fail-closed for rule cross-references).

**Errors**: `CatalogueLinterError::InvalidRuleConfig(String)` is returned for unsupported `target_field` names, or for carry-prechecked rule kinds when any selected `target_role` does not carry the field. `CatalogueLinterRuleError::InvalidRuleConfig(String)` is returned by `CatalogueLinterRule::new` when `ForbiddenMethodReceiver.forbidden_receiver` does not match the canonical receiver set. `MethodReferenceSignature` and `AccessorSignatureRequired` reject only unsupported field names (`invariants` / `identity`, respectively) and skip entries whose role does not carry that accepted field. `CatalogueLinterError::UnknownLayer { layer_id }` is returned when `target_layer_id` is not present in the catalogue map.

## Distribution & Config (ADR D15 / D19)

The lint configuration mechanism is separate from `<layer>-types.json` but uses related types. A type-designer cataloguing the `lint` machinery must know these files exist:

- **`.harness/catalogue-lint/presets/ddd-strict.json`** — the canonical *distributed preset*. Contains `{ "schema_version": 1, "rules": [...] }` with the minimum-core rules derived deterministically from ADR D4–D11 / D16 / D18. The user copies this file (or its rule list) into their `config.json`; there is no Rust `ddd_strict_preset()` API (D15 amend).
- **`.harness/catalogue-lint/config.json`** — the per-project lint config. Same `{ "schema_version": 1, "rules": [...] }` shape. `sotp track lint` resolves rules with the precedence **CLI `--rules-file` > `config.json` > fail-closed error**. There is no silent preset fallback (D19).

Types introduced by D19 that the type-designer may need to catalogue:

- `LintConfig` (usecase layer, `role: ValueObject`) — holds the parsed `rules: Vec<LintRuleSpec>` with a private field, exposes `new(rules)` / `rules() -> &[LintRuleSpec]`.
- `LintConfigLoader` (usecase layer, `role: SecondaryPort`) — `Send + Sync` trait with `fn load(&self) -> Result<LintConfig, LintConfigLoaderError>` (no path parameter; the path is baked into the adapter at construction).
- `LintConfigLoaderError` (usecase layer, `role: ErrorType`) — variants `MissingFile { path: PathBuf }` / `ParseError { path: PathBuf, reason: String }` / `SchemaVersionMismatch { expected: u32, actual: u32 }`.
- `FsLintConfigLoader` (infrastructure layer, `role: SecondaryAdapter`) — single private field `path: PathBuf`; constructor `new(path)`. Implements `LintConfigLoader` over the workspace JSON file.

Codec error names worth knowing for catalogue work:

- `CatalogueDocumentCodecError::SchemaVersionRequiresMigration { from, to, reason }` — returned when the codec sees `schema_version: 4` (or any other version that needs migration). Older versions return `UnsupportedSchemaVersion`.

## Catalogue Pattern Cookbook (v5)

Concrete catalogue shapes. In the generate + annotate workflow the skeletons are produced by `sotp catalog add` / `sotp catalog import` — use these patterns as **reading references**: to understand a generated entry, to judge what to fill into a `$todo` hole, and to verify hand-adjustments after generation. They also remain the target shapes the generated output converges to.

> **Schema Reference takes precedence.** The cookbook examples below are written in the normative **v5 wire format**: `role` uses the discriminated-object form for type-section and trait-section entries (e.g. `"role": { "ValueObject": {} }`, `"role": { "SecondaryPort": {} }`), while function-section entries keep the plain-string form (`"role": "UseCaseFunction"` — `FunctionRole` is a fieldless enum, wired as `role: String` in the codec DTO). If a cookbook literal ever diverges from the schema reference sections above, the schema reference sections win.

> **Layer-name disclaimer.** The cookbook examples below use the layer / crate name placeholders `<core-crate>` (a layer that may host roles like `"ValueObject"` / `"SecondaryPort"`) and `<adapter-crate>` (a layer that may host roles like `"SecondaryAdapter"`). For *this* workspace, the actual names are listed in `architecture-rules.json` and the legal role × layer combinations are specified in `knowledge/conventions/type-designer-kind-selection.md` § R1. Substitute the placeholders for the real names — do not copy the placeholders verbatim into the JSON. The catalogue file name follows the pattern `<layer>-types.json` (e.g. `<core-crate>-types.json`); locate the legal layer names from the SSoT pair.
>
> For a worked example in a real catalogue, consult the latest tracks under `track/items/<id>/` — each track ships `<layer>-types.json` files that show how the layer names from `architecture-rules.json` are substituted in.

Patterns 1 and 3 show complete documents with `"schema_version": 5`. Patterns 2, 4–8 show partial BTreeMap sections (e.g. `"types": { ... }`) extracted from a full document for conciseness; they use `jsonc` fences because some contain `//` annotation comments. The codec accepts only `"schema_version": 5` — versions 1–4 are rejected fail-closed (v4 with a migration prompt).

### Pattern 1: Typestate cluster + enum wrapper (state machine + heterogeneous Vec)

ADR decision lifecycle `Proposed → Accepted → Implemented → Superseded | Deprecated`. One struct per state with its `typestate` marker set (orthogonal to `shape`) + one `Enum` wrapper.

```json
{
  "schema_version": 5,
  "crate_name": "<core-crate>",
  "layer":       "<core-crate>",
  "types": {
    "ProposedDecision": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": {
        "kind": "struct",
        "shape": {
          "kind": "plain",
          "fields": [
            { "name": "common", "ty": "AdrDecisionCommon" }
          ],
          "has_stripped_fields": false
        },
        "typestate": { "state_name": "AdrDecisionLifecycle", "transition_methods": ["accept"] }
      },
      "methods": [
        {
          "name": "accept",
          "receiver": "self",
          "params": [],
          "returns": "AcceptedDecision",
          "is_async": false,
          "generics": [],
          "has_default_impl": false,
          "where_predicates": []
        }
      ],
      "module_path": "adr",
      "docs": "Typestate for a newly drafted decision awaiting review.",
      "spec_refs": [],
      "informal_grounds": []
    },
    "AcceptedDecision": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": {
        "kind": "struct",
        "shape": {
          "kind": "plain",
          "fields": [{ "name": "common", "ty": "AdrDecisionCommon" }],
          "has_stripped_fields": false
        },
        "typestate": { "state_name": "AdrDecisionLifecycle", "transition_methods": ["implement"] }
      },
      "methods": [
        {
          "name": "implement",
          "receiver": "self",
          "params": [{ "name": "implemented_in", "ty": "String" }],
          "returns": "ImplementedDecision",
          "is_async": false,
          "generics": [],
          "has_default_impl": false,
          "where_predicates": []
        }
      ],
      "module_path": "adr",
      "docs": "Typestate for a decision that has been accepted.",
      "spec_refs": [], "informal_grounds": []
    },
    "ImplementedDecision": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": {
        "kind": "struct",
        "shape": {
          "kind": "plain",
          "fields": [
            { "name": "common",         "ty": "AdrDecisionCommon" },
            { "name": "implemented_in", "ty": "String" }
          ],
          "has_stripped_fields": false
        },
        "typestate": { "state_name": "AdrDecisionLifecycle", "transition_methods": [] }
      },
      "methods": [],
      "module_path": "adr",
      "docs": "Typestate for a decision that has been implemented.",
      "spec_refs": [], "informal_grounds": []
    },
    "SupersededDecision": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": {
        "kind": "struct",
        "shape": {
          "kind": "plain",
          "fields": [
            { "name": "common",        "ty": "AdrDecisionCommon" },
            { "name": "superseded_by", "ty": "String" }
          ],
          "has_stripped_fields": false
        },
        "typestate": { "state_name": "AdrDecisionLifecycle", "transition_methods": [] }
      },
      "methods": [],
      "module_path": "adr",
      "docs": "Terminal typestate for a decision replaced by a later decision.",
      "spec_refs": [], "informal_grounds": []
    },
    "DeprecatedDecision": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": {
        "kind": "struct",
        "shape": {
          "kind": "plain",
          "fields": [{ "name": "common", "ty": "AdrDecisionCommon" }],
          "has_stripped_fields": false
        },
        "typestate": { "state_name": "AdrDecisionLifecycle", "transition_methods": [] }
      },
      "methods": [],
      "module_path": "adr",
      "docs": "Terminal typestate for a deprecated decision.",
      "spec_refs": [], "informal_grounds": []
    },
    "AdrDecisionEntry": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": {
        "kind": "enum",
        "variants": [
          { "name": "Proposed",     "payload": { "kind": "tuple", "fields": ["ProposedDecision"] } },
          { "name": "Accepted",     "payload": { "kind": "tuple", "fields": ["AcceptedDecision"] } },
          { "name": "Implemented",  "payload": { "kind": "tuple", "fields": ["ImplementedDecision"] } },
          { "name": "Superseded",   "payload": { "kind": "tuple", "fields": ["SupersededDecision"] } },
          { "name": "Deprecated",   "payload": { "kind": "tuple", "fields": ["DeprecatedDecision"] } }
        ]
      },
      "methods": [],
      "module_path": "adr",
      "docs": "Enum wrapper for heterogeneous Vec<AdrDecisionEntry> membership.",
      "spec_refs": [], "informal_grounds": []
    }
  },
  "traits": {},
  "functions": {}
}
```

Anti-pattern: a flat `Enum` `DecisionStatus { Proposed, Accepted, ... }` plus a plain-shape struct `{ status: DecisionStatus, implemented_in: Option<String>, superseded_by: Option<String> }`. That shape permits `Proposed { superseded_by: Some(...) }` — runtime invariants only. Per `knowledge/conventions/prefer-type-safe-abstractions.md` § Enum-first / § Typestate, use a typestate cluster instead.

### Pattern 2: Pure enum with variant payloads (finite values, no transitions)

```jsonc
"types": {
  "FailureDetail": {
    "action": "add",
    "role": { "ValueObject": {} },
    "kind": {
      "kind": "struct",
      "shape": { "kind": "plain", "fields": [{ "name": "message", "ty": "String" }], "has_stripped_fields": false }
    },
    "methods": [],
    "module_path": "result", "docs": null, "spec_refs": [], "informal_grounds": []
  },
  "SomeResult": {
    "action": "add",
    "role": { "ValueObject": {} },
    "kind": {
      "kind": "enum",
      "variants": [
        { "name": "Success", "payload": { "kind": "unit" } },
        { "name": "Failure", "payload": { "kind": "tuple", "fields": ["FailureDetail"] } }
      ]
    },
    "methods": [],
    "module_path": "result", "docs": null, "spec_refs": [], "informal_grounds": []
  }
}
```

### Pattern 3: Hexagonal port + adapter pair (cross-crate references)

The core-tier crate declares the port + error type; an adapter-tier crate declares the adapter that implements it. The adapter side puts a **top-level `trait_impls` entry** whose `trait_ref` references the port via a crate-prefixed fully-qualified path and whose `for_type` names the adapter, so the cross-crate edge is resolvable.

```jsonc
// <core-crate>-types.json
{
  "schema_version": 5,
  "crate_name": "<core-crate>",
  "layer":       "<core-crate>",
  "types": {
    "AdrFilePortError": {
      "action": "add",
      "role": { "ErrorType": {} },
      "kind": {
        "kind": "enum",
        "variants": [
          { "name": "ListPaths", "payload": { "kind": "tuple", "fields": ["String"] } },
          { "name": "ReadFile",  "payload": { "kind": "tuple", "fields": ["std::path::PathBuf", "String"] } }
        ]
      },
      "methods": [],
      "module_path": "adr::port", "docs": null, "spec_refs": [], "informal_grounds": []
    }
  },
  "traits": {
    "AdrFilePort": {
      "action": "add",
      "role": { "SecondaryPort": {} },
      "methods": [
        {
          "name": "read_adr_frontmatter",
          "receiver": "&self",
          "params":   [{ "name": "path", "ty": "std::path::PathBuf" }],
          "returns":  "Result<AdrFrontMatter, AdrFilePortError>",
          "is_async": false,
          "generics": [],
          "has_default_impl": false,
          "where_predicates": []
        }
      ],
      "supertrait_bounds": [],
      "module_path": "adr::port",
      "docs": "Secondary port for ADR file enumeration and front-matter parsing.",
      "spec_refs": [], "informal_grounds": []
    }
  },
  "functions": {}
}
```

```jsonc
// <adapter-crate>-types.json — adapter side; the impl is a top-level trait_impls entry
{
  "schema_version": 5,
  "crate_name": "<adapter-crate>",
  "layer":       "<adapter-crate>",
  "types": {
    "FsAdrFileAdapter": {
      "action": "add",
      "role": { "SecondaryAdapter": {} },
      "kind": {
        "kind": "struct",
        "shape": { "kind": "plain", "fields": [{ "name": "adr_dir", "ty": "std::path::PathBuf" }], "has_stripped_fields": false }
      },
      "methods": [],
      "module_path": "adr::fs",
      "docs": "Filesystem adapter implementing AdrFilePort.",
      "spec_refs": [], "informal_grounds": []
    }
  },
  "traits": {},
  "functions": {},
  "trait_impls": [
    {
      "trait_ref": "<core-crate>::adr::port::AdrFilePort",
      "for_type":  "FsAdrFileAdapter"
    }
  ]
}
```

Notes:
- Cross-crate references in `params[].ty` / `returns` use **FQN** (e.g. `<core-crate>::adr::port::AdrFilePort`). The A-codec's `external_crates` auto-build resolves the prefix to an `ExternalCrate` entry.
- `trait_impls` is a **top-level array** (not a `TypeEntry` field). Each entry uses `action` (defaults to `"add"` when omitted) + `trait_ref` (the trait reference as a TypeRef — a crate-prefixed FQN for a cross-crate port, e.g. `"<core-crate>::adr::port::AdrFilePort"`; a bare short name for a self-crate trait) + `for_type` (the implementing self type — a bare short name for a self-crate type, e.g. `"FsAdrFileAdapter"`).
- In-crate references (within the same `crate_name`) use **last-segment names** (e.g. `AdrFrontMatter`). Standard-library types not in the auto-resolve set (e.g. `std::path::PathBuf`) use their full path.
- Object-safety: prefer owned types (`std::path::PathBuf`) over unsized borrowed types (`&std::path::Path`) in port method signatures so `Arc<dyn Port>` works without lifetime gymnastics.

### Pattern 4: `modify` trait with all methods + cross-crate FQN

When a trait is `modify`-ed, the declaration must enumerate every method. Partial enumeration triggers `Mismatch_Modify` → 🟡.

```jsonc
"traits": {
  "TrackBlobReader": {
    "action": "modify",
    "role":   { "SecondaryPort": {} },
    "methods": [
      {
        "name": "read_spec_document",
        "receiver": "&self",
        "params":   [{ "name": "track_id", "ty": "TrackId" }],
        "returns":  "Result<<core-crate>::spec::SpecDocument, TrackBlobReaderError>",
        "is_async": false,
        "generics": [],
        "has_default_impl": false,
        "where_predicates": []
      },
      {
        "name": "read_type_catalogue",
        "receiver": "&self",
        "params":   [
          { "name": "track_id", "ty": "TrackId" },
          { "name": "layer",    "ty": "<core-crate>::tddd::LayerId" }
        ],
        "returns":  "Result<Option<String>, TrackBlobReaderError>",
        "is_async": false,
        "generics": [],
        "has_default_impl": true,
        "where_predicates": []
      }
      // ... every other method of the trait, in declared order
    ],
    "supertrait_bounds": ["Send", "Sync"],
    "module_path": "track::blob",
    "docs": null,
    "spec_refs":         [{ "file": "track/items/<id>/spec.json", "anchor": "IN-…" }],
    "informal_grounds":  []
  }
}
```

### Pattern 5: `add` free function with generics + where_predicates

This example is from `<orchestration-crate>-types.json` (so `crate_name: "<orchestration-crate>"`). The function path key MUST start with the document's own `crate_name::` (the codec rejects cross-crate function paths per D4).

```jsonc
// In <orchestration-crate>-types.json — crate_name is "<orchestration-crate>"
"functions": {
  "<orchestration-crate>::merge_gate::check_strict_merge_gate": {
    "action":   "add",
    "role":     "UseCaseFunction",  // function-section roles keep the plain-string form (FunctionRole is fieldless)
    "params":   [{ "name": "registry", "ty": "R" }],
    "returns":  "Result<<core-crate>::verify::VerifyOutcome, MergeGateError>",
    "is_async": false,
    "generics": [
      { "name": "R", "bounds": ["TrackRegistry", "Send", "Sync"] }
    ],
    "where_predicates": [],
    "docs": "Strict variant of the merge-gate that requires all required scopes to be Approved.",
    "spec_refs":        [],
    "informal_grounds": []
  }
}
```

For LHS forms that the inline `bounds` field cannot express (e.g. `where Vec<T>: Clone`, `where T::Item: Send`), use `where_predicates`:

```jsonc
"generics":         [{ "name": "T", "bounds": [] }],
"where_predicates": [
  { "lhs": "Vec<T>", "rhs": ["Clone"] }
]
```

### Pattern 6: Type alias entry

A `type_alias` entry is for a genuine Rust `pub type` declaration — a named alias for an existing type, with no validation or newtype semantics. **Do not use `type_alias` for validated IDs or newtypes**: those must use a `tuple` shape (single-field newtype with a validating constructor) or a `plain` shape with a `value()` accessor.

```jsonc
"types": {
  "TrackResult": {
    "action": "add",
    "role":   { "Dto": {} },
    "kind":   { "kind": "type_alias", "target": "Result<TrackId, TrackError>" },
    "methods": [],
    "module_path": "track", "docs": null, "spec_refs": [], "informal_grounds": []
  }
}
```

### Pattern 7: `delete` entry (excluded from S during Phase 1)

`sotp catalog import --action delete --type <crate>::...::<Type> --anchor <spec-anchor>` writes an identity-only tombstone. A delete entry records the removed type's name, optional `module_path`, `spec_refs`, and `informal_grounds`; it carries no live shape fields (`role`, `kind`, `methods`, or `docs`). Do not hand-add those fields after generation: the entry is excluded from S and routed to the deleted set D, so adding a stale baseline shape makes the tombstone misleading instead of more complete.

```jsonc
"types": {
  "LegacyConfig": {
    "action": "delete",
    "module_path": "legacy",
    "spec_refs": [{ "file": "track/items/<id>/spec.json", "anchor": "IN-..." }],
    "informal_grounds": []
  }
}
```

### Pattern 8: `reference` entry (carried for edge exposure)

A `reference` entry is for a **pre-existing workspace type already in baseline** that this track does not modify. It is included only so that edges that reference it (`trait_impls`, `params[].ty`, etc.) appear in the contract-map / baseline-graph rendering.

A `reference` entry's methods / fields do not drive Phase 2 structural equality: Phase 2 compares the baseline item (B) against the current source (C), not the catalogue declaration (A). They are still required for the authoring contract. `sotp catalog import --action reference` carries the rustdoc shape unchanged; keep that baseline shape intact, including the full baseline method list for referenced ports. Do not shrink a reference trait to `methods: []` unless the baseline trait actually has no methods: R7 requires baseline port methods to be enumerated, and capability step 12c requires every reference entry to be confirmed baseline-identical.

```jsonc
"traits": {
  "UserRepository": {
    "action": "reference",
    "role":   { "Repository": { "aggregate": "User" } },
    "methods": [
      {
        "name": "find_by_id",
        "receiver": "&self",
        "params": [{ "name": "id", "ty": "UserId" }],
        "returns": "Result<Option<User>, UserRepositoryError>",
        "is_async": false,
        "generics": [],
        "has_default_impl": false,
        "where_predicates": []
      }
      // ... all other methods of the trait, in declared order
    ],
    "supertrait_bounds": ["Send", "Sync"],
    "module_path": "user::port",
    "docs": "Carried so that `PgUserRepository: UserRepository` edges are visible in the contract-map.",
    "spec_refs": [], "informal_grounds": []
  }
}
```

## Exceptions

- 本書は記述的参照であり、拘束ルールは持たない。schema の拘束は sotp の codec / check / linter が fail-closed に実施する (乖離時は sotp が正 — 冒頭の Authority note を参照)。

## Review Checklist

- 本書の記述を根拠に catalogue entry を評価する際は、疑わしい記述を必ず `libs/domain/src/tddd/catalogue_v2/` / `libs/infrastructure/src/tddd/catalogue_document_codec/` の実装と突き合わせる (本書は mirror であり authority ではない)。
- 本書を更新する変更が入った場合、schema 実装側の変更に由来するか (追随更新) を確認する — 本書単独での schema 変更提案は無効。

## Related Documents

- `.harness/capabilities/type-designer.md` — 生成 + 注釈 workflow の手順書 (capability operational SSoT)
- `knowledge/conventions/type-designer-kind-selection.md` — role / kind 選定の拘束ルール (R1-R10)
- `knowledge/adr/2026-07-02-1345-catalogue-generation-annotation.md` — 生成 + 注釈への移行決定 (D2) と本書の設置決定 (D9)
- `libs/domain/src/tddd/catalogue_v2/` / `libs/infrastructure/src/tddd/catalogue_document_codec/` — schema authority (canonical SSoT)
