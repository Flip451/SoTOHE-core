# Short-name identity survey (researcher, 2026-08-21)

Read-only survey dispatched during Phase 0 of track `2026-08-21-type-identity-fully-qualified-paths` to ground the ADR rewrite. Output of the `researcher` capability, verbatim except for removal of the dispatcher trailer and replacement of work-machine absolute paths with repository-relative paths.

調査結果です。実装上、短名依存は compiler-internal fallback に限られません。特に型／トレイトの主 identity、Phase 1 の参照解決、構造比較の generic 引数が短名です。

## Q1. 短名依存の箇所

| 箇所 | 短名になる条件 | 突合相手 | 衝突時の観測可能な壊れ方 |
|---|---|---|---|
| [`build_type_trait_identity_map`](libs/infrastructure/src/tddd/signal_evaluator_v2/mod.rs:155) | 常時。local `Struct` / `Enum` / `TypeAlias` / `Trait` の `item.name` を key にする | A↔B の action 解決、S/D/C の top-level 型・trait 照合 | 同名候補は full path の辞書順で一方だけ残る。B 側で落とされた型を他の残存 item が参照すると、S に参照先が入らず Phase 1.6 の `DanglingId` になり得る。Phase 2 では片方が不可視化される。 |
| [`Phase1State::s_type_name_to_id`](libs/infrastructure/src/tddd/signal_evaluator_v2/phase1/state.rs:30)、Phase 1.45 | 常時。`name → Id`、A local `paths` も `.last()` で同じ map を引く | A→S ID remap、S/D の top-level 管理 | 同名 local item の後挿入が前者を上書きする。A の path が別 module を指していても誤った S Id に remap され、後続の closed-world／dangling-id 検査が壊れる。 |
| [`resolve_type` 等](libs/infrastructure/src/tddd/signal_evaluator_v2/resolve_type.rs:26) | unresolved な local / `crate::` / `self::` / `super::` path は常に最後の segment を取り、`s_name_to_id` を引く | A の unresolved `Type`／trait-bound ↔ S | `crate::a::Input` と `crate::b::Input` はともに `Input`。存在すれば誤解決、なければ `UnresolvedTypeRef`。誤解決後は参照先 ID が意図と異なり、Phase 1.6 で `DanglingId` または Phase 2 の不正な比較へ進む。 |
| [`EncoderState::local_name_to_id`](libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec/encoder.rs:56) と TypeRef parser | catalogue type/trait key は `TypeName` / `TraitName` の単一 identifier。`crate::module::X` は parser が最後の `X` で local lookup する | catalogue A → synthetic rustdoc crate | 同一短名の type/trait は codec が `AmbiguousIdentifier` で拒否するか、一方しか表せない。module_path は schema にあるがこの lookup key には入らない。 |
| [`build_impl_identity_map`](libs/infrastructure/src/tddd/signal_evaluator_v2/impl_identity.rs:170) | `for_` は常に `format_type` の短名。local trait は `.last()`、通常の外部 multi-segment trait も実装では `.last()`。`domain` / `usecase` だけは crate path を残す | S/D/C trait-impl identity | 異なる self type または trait が同じ key に畳まれ、map は辞書順の一件だけを保持する。現行テストも `MyError: MyTrait` へ二つの impl が縮退し一件を落とすことを明示しています。 [`tests.rs`](libs/infrastructure/src/tddd/signal_evaluator_v2/tests.rs:3522) |
| [`format_type` と generic formatter](libs/infrastructure/src/tddd/signal_evaluator_v2/format/ty_base.rs:78)、[`structural_eq`](libs/infrastructure/src/tddd/signal_evaluator_v2/structural_eq.rs:57) | `ResolvedPath`、dyn / impl trait bound、qualified path、nested generic args を再帰的に最後の segment へ縮退 | S↔C の field、variant、alias、function/method signature、impl generic args の構造比較 | `From<serde_json::Error>` と `From<std::io::Error>` がいずれも `From<Error>` となる既知の偽陽性衝突。実際の記録では Red を回避するため一件だけを宣言していました。 |
| Contract Map の [`NodeIndex::resolve`](libs/infrastructure/src/tddd/contract_map_renderer_adapter/render/node_index.rs:66) | `crate::module::Foo` と外部の module-qualified ref を crate + 最終 segment へ縮退 | catalogue TypeRef → Mermaid node | S/C/D evaluator ではなく表示系。別 module の同名 node へ edge が接続される、または node が見つからず edge が出ない。 |

短名依存ではない主要面もあります。

- free function identity は [`FunctionPath`](libs/infrastructure/src/tddd/signal_evaluator_v2/mod.rs:189) の完全 path。
- baseline graph の trait lookup は `(crate, module_path, trait_name)` で、rustdoc `paths` から組み立てます。 [`trait_index.rs`](libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/trait_index.rs:22)
- catalogue codec の external synthetic ID cache は canonical full path key です。 [`ensure_external_type_id`](libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec/encoder_state_core.rs:57)
- 旧 `code_profile_builder` の short-name `HashMap` は削除済み stub で、現在の評価経路ではありません。 [`code_profile_builder.rs`](libs/infrastructure/src/code_profile_builder.rs:1)

## Q2. fallback の実態

- `normalize_impl_trait_path` の fallback 自体は、`trait_path.id` が `krate.paths` に無い全ての impl trait path に使われます。compiler-internal 専用ではありません。 [`impl_identity.rs`](libs/infrastructure/src/tddd/signal_evaluator_v2/impl_identity.rs:336)
- ただし、現在の A codec が生成する既知 external path は `ensure_external_type_id` により synthetic ID **と同時に `paths` entry** を作ります。したがって「A codec の synthetic ID だから `paths` に無い」は、現実装では構造的必然ではありません。 [`encoder_state_core.rs`](libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec/encoder_state_core.rs:61)
- local catalogue type/trait も `local_name_to_id` と local `paths` entry を持つため、通常の A-side trait impl は fallback を必要としません。
- auto/synthetic impl は identity map の先頭で `impl_.is_synthetic` により除外されます。negative／blanket も同様です。 [`impl_identity.rs`](libs/infrastructure/src/tddd/signal_evaluator_v2/impl_identity.rs:186)
- `StructuralPartialEq`、`StructuralEq`、`TrivialClone` は、テスト上は external `core` trait・`paths` entry あり・`is_synthetic: false` の impl として identity map に到達し、そこで allowlist により除外されます。つまり確認できる compiler-internal 経路は fallback ではありません。 [`tests.rs`](libs/infrastructure/src/tddd/signal_evaluator_v2/tests.rs:1195)
- “anonymous items” は fallback の doc comment にのみあり、実装・テストに具体的な rustdoc item kind／再現 fixture の定義はありません。今回の調査では、path-less trait impl が実際の rustdoc 入力で発火する証拠は見つかりませんでした。

## Q3. 完全 path 化で現在のまま成立しないもの

- catalogue の `types` / `traits` key は `TypeName` / `TraitName` であり、`::` を許さない単一 identifier です。完全 path を key にするなら現 schema key はそのまま使えません。 [`identifiers.rs`](libs/domain/src/tddd/catalogue_v2/identifiers.rs:10)
- 各 entry は既に `module_path` を保持しますが、codec の ID pre-pass と local resolver は name 単独です。既存情報だけでは、現在の codec は `module_path + name` を識別に使いません。 [`encoder.rs`](libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec/encoder.rs:100)
- 既存 catalogue JSON は bare top-level key と entry 内の `module_path` という形です。多数の track snapshot が non-root module path を持っています。例: [`infrastructure-types.json`](track/items/codex-resolve-and-link-provisioning-2026-07-18/infrastructure-types.json:247)。
- self-crate `TraitImplDeclV2.trait_ref` は、crate-prefixed／module-qualified で書いても codec が最後の segment に縮めて local lookup します。現在のままでは full-path trait identity と対称になりません。 [`encoder_state_fn_trait_codec.rs`](libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec/encoder_state_fn_trait_codec.rs:440)
- type-signal 出力側も catalogue type/trait name 単独で kind tag map を作り、impl signal の owner を `": "` より前の文字列で集約します。full identity に変わればこの結合も現在のままでは対応しません。 [`type_signals_evaluator.rs`](libs/infrastructure/src/tddd/type_signals_evaluator.rs:284)
- raw rustdoc baseline は既に `Crate.paths` を保存しているため、baseline wire format 自体に short-name migration は確認できません。 [`baseline_rustdoc_codec.rs`](libs/infrastructure/src/tddd/baseline_rustdoc_codec.rs:72)
- evaluator／codec unit tests、fixture catalogue、短名 collision を意図的に期待するテストは影響対象です。代表例は上記 impl collision test と architecture-rules fixture の bare keys です。

## Q4. 既存記録

- 最も直接的な過去 observation は [`spec-ref-embedded-hash-removal` の observations](track/items/spec-ref-embedded-hash-removal-2026-06-11/observations.md:8) です。`From<serde_json::Error>`／`From<std::io::Error>` が `From<Error>` に縮退する既知制限を記録しています。
- [`2026-04-12-1800` ADR](knowledge/adr/2026-04-12-1800-reviewstate-v1-decommission.md:45) は、旧 `code_profile_builder` の short-name collision が baseline signal の偽 Red を生んだ履歴を記録します。ただし当該 builder は現在削除済みです。
- `DanglingId` について、過去 `observations.md`／`knowledge/research/` に今回の同名 module collision を具体的に再現した記録は見つかりませんでした。現行の track ADR には主張がありますが、保持された観測ログではありません。

推奨する次の作業は、ADR の fallback 記述を「path-less compiler-internal に限る」という前提ではなく、上記の正常 A-codec 経路・実在する short-name identity 面・未立証の anonymous-item 経路を分けて事実記述へ収束させることです。今回は read-only で、ファイル変更はしていません。


---

# Follow-up survey: catalogue-lint and other TypeRef resolution paths (researcher, 2026-08-21)

Continuation dispatched to fix the ADR scope boundary. Verbatim except for removal of the dispatcher trailer and replacement of work-machine absolute paths with repository-relative paths.

## Q5. catalogue-lint

| Rule / path | Lookup key・path の扱い | 同名時 | 種別 |
|---|---|---|---|
| `ReferencedRoleConstraint` → `resolve_type_role` | layer-qualified ref は、既知 `LayerId` を path segment から見つけ、**最後の segment の bare name** をその layer catalogue で検索。module path は照合しない。 [`catalogue_linter_eval_helpers.rs`](libs/domain/src/tddd/catalogue_linter_eval_helpers.rs:359) | target layer に同名があればそれを優先。他 layer は role が全て同じなら許容、異なる role なら `None` → lint violation。単一 catalogue 内の type/trait 同名は type を先に返す。 [`catalogue_linter_eval_helpers.rs`](libs/domain/src/tddd/catalogue_linter_eval_helpers.rs:410) | identity 解決 |
| 同 rule の prefix | `domain::models::Foo` は `domain` catalogue の `Foo`。`crate::Foo` / `self::Foo` / `super::Foo` は known layer ではないため bare-name 検索へ退化する。参照・pointer sigil は先に除去する。 [`catalogue_linter_eval_helpers.rs`](libs/domain/src/tddd/catalogue_linter_eval_helpers.rs:48) | module 同名を区別しない。 | identity 解決 |
| `KindLayerConstraint` | TypeRef を解決しない。対象 entry 自身の `catalogue.layer()` が permitted layers に入るかだけを検査。 [`catalogue_linter_eval.rs`](libs/domain/src/tddd/catalogue_linter_eval.rs:190) | 該当なし。 | entry 属性検査 |
| `FieldElementUniqueAcrossEntries` / `NoExternalReferenceInMethods` | 前者は TypeRef の `split("::").last()`、後者は bare tail の文字列境界検索を使う。 [`catalogue_linter_eval.rs`](libs/domain/src/tddd/catalogue_linter_eval.rs:507) | path の違う同名型を同一視して violation を出し得る。 | identity 風の lint 補助 |

`ReferencedRoleConstraint` の delete entry は lookup 対象外です。未解決は expected role 不一致として violation になります。 [`catalogue_linter.rs`](libs/domain/src/tddd/catalogue_linter.rs:5410)

## Q6. 追加の解決経路

| 経路 | key / 衝突時 | identity か表示か |
|---|---|---|
| `sotp catalog import --type` | `crate::module::Type` を `(crate,module,name)` に分解し、rustdoc schema の name と module segments を**完全一致**で選ぶ。short-name fallback はない。catalogue への書込み key は最終 `name` なので、別 module の同名を二つ import しようとすると section 内 duplicate entry で失敗。 [`import_shape.rs`](libs/infrastructure/src/tddd/catalog_gen/import_shape.rs:152) | rustdoc item identity |
| `sotp catalog add` | section + bare `CatalogEntryName` で挿入し、同 section の同 key は拒否。module_path は scaffold の入力穴で、insert key ではない。 [`fs_access.rs`](libs/infrastructure/src/tddd/catalog_gen/fs_access.rs:320) | catalogue entry identity |
| `sotp catalog cite` | `--entry` は bare key を `types/traits/functions` 全 section から探す。同名が複数 section にあれば fail-closed、存在しなければエラー。module path selector はない。 [`fs_access.rs`](libs/infrastructure/src/tddd/catalog_gen/fs_access.rs:349) | catalogue entry identity |
| `catalog check` / catalogue-spec refs | TypeRef 解決はしない。spec anchor は `SpecElementId` set として照合する。catalogue entry の内部 hash は `section + entry_key` を使い、type/trait 同名は区別する。 [`validate.rs`](libs/infrastructure/src/tddd/catalog_gen/validate.rs:72), [`catalogue_spec_signals.rs`](libs/usecase/src/catalogue_spec_signals.rs:215) | anchor / entry identity |
| `test-obligation derive` の trait impl | trait role index は `(catalogue crate_name, TraitName)`。`TraitRefScope::Workspace` も crate + bare trait name のみで、module path は使わない。external trait は zero obligations。 [`derive/mod.rs`](libs/usecase/src/test_obligation/derive/mod.rs:248) | catalogue trait identity |
| test-obligation の anchor | entry / method の `spec_refs` をそのまま `(file_path, anchor)` に変換する。TypeRef を spec entry に解決しない。 [`derive/mod.rs`](libs/usecase/src/test_obligation/derive/mod.rs:594) | anchor identity |
| `task-contract` / pre-review gate | `(layer, CatalogueEntryKey)`。`CatalogueEntryKey` は non-empty string だけを検証し、type/trait section や module path を持たない。signal document の `type_name` と文字列一致する。 [`task_contract.rs`](libs/domain/src/task_contract.rs:20), [`pre_review_gate.rs`](libs/usecase/src/pre_review_gate.rs:467) | catalogue entry / signal identity |
| baseline graph | `rustdoc.paths` の crate + module path + name、または `Path.id → ItemSummary` を使う。path-less item は edge を出さず skip。短名 key ではない。 [`trait_index.rs`](libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/trait_index.rs:22), [`edge_collector.rs`](libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/edge_collector.rs:519) | renderer の edge identity |
| contract map | 前回報告どおり `crate::bare-name` index と module-tail fallback。表示だけでなく edge target 解決にも使う。 [`node_index.rs`](libs/infrastructure/src/tddd/contract_map_renderer_adapter/render/node_index.rs:66) | renderer の edge identity |
| `dup-check` / `find-similar` | CodeFragment の embedding と source path を対象にする semantic-dup 機能。catalogue TypeRef／type name を entry へ解決する経路は見つからなかった。 | 該当なし |

`spec.json` 側の `catalogue_refs` / catalogue-entry reverse reference は、`libs/` と `apps/` の model・codec を検索した範囲では見つかりませんでした。現存する参照方向は catalogue `spec_refs[]` → spec anchor です。

## Q7. `domain` / `usecase` 特例

これは workspace 外部 crate として見える `domain`・`usecase` の同名 trait を区別するためです。実装コメントは `ObligationFulfillmentCachePort` の二つの別 trait が短名で衝突する例を明記し、テストも両方の crate-qualified key が残ることを検証しています。 [`impl_identity.rs`](libs/infrastructure/src/tddd/signal_evaluator_v2/impl_identity.rs:305), [`tests.rs`](libs/infrastructure/src/tddd/signal_evaluator_v2/tests.rs:1058)

残る不確実性は、test-obligation の trait-role index が同一 crate 名の catalogue を複数受け取った場合に最初の一致を取る点です。通常の「1 crate = 1 catalogue」前提外なので、実運用で到達するかはこの調査だけでは確認できません。

推奨する次の作業は、ADR scope に少なくとも catalogue-lint、test-obligation trait resolution、task-contract/type-signal keying を明記し、`catalog import` と baseline graph は既に full-path identity である対照例として除外することです。変更はしていません。
