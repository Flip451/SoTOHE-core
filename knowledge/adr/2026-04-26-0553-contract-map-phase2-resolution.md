# TDDD Contract Map Phase 2 — Known Limitations (L1-L4) 解消の記録と spec 精緻化

## Status

Accepted (2026-04-26)

## Related ADRs

- `knowledge/adr/2026-04-17-1528-tddd-contract-map.md` — refinement 対象の前 ADR。本 ADR は同 ADR § Known Limitations に記録された L1-L4 の解消経路と、Phase 2 実装で判明した spec 精緻化を post-merge record として残す。前 ADR は frozen (immutable record)。
- `knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md` — Reality View。Contract Map との役割分担 (§D10) を規定
- `knowledge/adr/2026-04-13-1813-tddd-taxonomy-expansion.md` — TypeDefinitionKind 13 variants taxonomy (§D1 / §D3)
- `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` — multilayer 型カタログ・layer-agnostic 不変条件 (§D6)
- `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md` — domain serde 依存除去 (hexagonal 純粋性)

## Context

### §1 前 ADR § Known Limitations の背景

`knowledge/adr/2026-04-17-1528-tddd-contract-map.md` は、Contract Map の Phase 1 / Phase 1.5 dogfood 時点で **edge を持たずに孤立表示される型が 4 カテゴリ (L1-L4)、計 9 nodes** 存在することを Known Limitations として記録した。これらはいずれも設計ミスや実装バグではなく、カタログ仕様の初期スコープ制約に由来するものとして分類されていた。

前 ADR は post-merge で immutable record となっているため、L1-L4 の解消記録は本 ADR に分離して残す。

### §2 解消された制約の概要

Phase 2 の実装により以下の能力が追加された:

1. **`unused_reference` / `declaration_only` classDef** — edge を持たない宣言型を dashed border で視覚識別する。isolation を隠すのではなく、意図的な「待機状態」や「宣言のみの型」であることを明示する可視化メカニズム。
2. **`declaration_only` 適用対象の narrowing** — 当初の設計では `declaration_only` を `action=modify` 全般に適用する想定だったが、Phase 2 実装の PR レビューで `is_method_bearing_kind()` に限定する修正が入った。`free_function` / `value_object` 等の method を持たない kind には `declaration_only` を不適切に適用しないよう絞り込まれた。
3. **field edge 描画** — `expected_members` を宣言した構造体から、参照先宣言型への field edge を描画する能力。外部型 (`String` / `bool` / `Option` / `Vec` 等) は CN-05 ルールに従い silently skip される。
4. **`Interactor` → `ApplicationService` impl edge** — `TypeCatalogueEntry::declares_application_service` フィールドを追加し、`interactor` kind の型が application service trait を impl することを catalogue で宣言可能にした。trait impl edge の描画対象を `secondary_adapter` のみから `secondary_adapter + interactor` に拡張。
5. **`TypeDefinitionKind::FreeFunction` variant** — catalogue schema に `free_function` kind を追加し、free function を catalogue entry として宣言可能にした。`expected_params` / `expected_returns` の宣言により、Phase 3 で param/return-type edge rendering を追加したときに自動的に edge が描画される基盤を確立した。

### §3 前 ADR § Known Limitations の L2 行の表現上の不正確さ

前 ADR § Known Limitations の L2 行には、`ContractMapRenderOptions` の解消経路として「free-function param edge」との表現が含まれていた。しかし Phase 2 の実際の解消経路は、`ContractMapRenderOptions` 自体を `action=modify` に変更し `expected_members` を宣言することで **field edge** を発生させる方式であった。`render_contract_map` 関数の `free_function` 種別化は L4 の対処に属し、L2b の `ContractMapRenderOptions` 解消は field edge による対処が正しい経路である。本 ADR はこの区別を明示的に記録する。

## Decision

### D1: L1 (forward-reference placeholders) の解消方針

**解消方針**: `action=reference` で宣言され、現時点で edge source / target いずれにも参加しない型を `unused_reference` classDef で dashed border 表示する。

前 ADR が提示していた「未使用 reference の可視マーク (例: dashed border + `(unused)` ラベル)」の方向性を採用した。ラベル付与は mermaid の表記上困難であったため、classDef による dashed border のみで識別する実装とした。

L1 に分類されていた 5 nodes (`TaskId` / `CommitHash` / `TrackBranch` / `NonEmptyString` / `ReviewGroupName`) は、dogfood 時点ですべて `unused_reference` dashed border で識別される状態になった。これらは将来の port / service 拡張で参照されたときに自動的に edge が発生する設計であり、「待機」状態の型であることが可視化されている。

### D2: L2a (declaration-only 型) の解消方針

**解消方針**: `action=modify` かつ `expected_methods` が空の型 (宣言のみで method edge の起点を持たない型) を `declaration_only` classDef で dashed border 表示する。

適用対象は `is_method_bearing_kind()` が true の kind に限定した。具体的には `value_object` / `free_function` 等 method を持たない kind には `declaration_only` を適用しない。この narrowing は Phase 2 の PR レビューで判明した spec の不正確さを修正したものであり、前 ADR § Known Limitations の L2 行が暗示していた適用範囲より限定的である。

`ValidationError` (dogfood 時点の L2a 例) は `declaration_only` dashed border で識別される状態になった。

### D3: L2b (field 参照型) の解消方針

**解消方針**: `expected_members` を catalogue entry に宣言することで、構造体の各フィールドが参照する宣言型への field edge を描画する。

前 ADR § Known Limitations の L2 行に記述された「free-function param edge」との表現は不正確であった。`ContractMapRenderOptions` の解消経路は:
- `action: reference` → `action: modify` への変更
- `expected_members` に 5 フィールド (`layers` / `kind_filter` / `signal_overlay` / `action_overlay` / `include_spec_source_edges`) を宣言

という field edge による対処であり、free-function param edge ではない。この区別を本 ADR で訂正記録する。

dogfood 検証で `ContractMapRenderOptions` から `TypeDefinitionKind` / `LayerId` への field edge 2 本が描画された。

### D4: L3 (Interactor → ApplicationService impl edge) の解消方針

**解消方針**: `TypeCatalogueEntry` に `declares_application_service: Option<String>` フィールドを追加し、`interactor` kind の型が実装する application service trait 名を catalogue で宣言可能にする。Contract Map renderer はこの宣言を読んで `-.impl.->` edge を描画する。

前 ADR § Known Limitations は「§D4 (2) を `SecondaryAdapter` + `Interactor` kind に拡張」と記述していた。実際には新規フィールド (`declares_application_service`) の追加という形で実装された。trait impl edge の描画は `secondary_adapter` (既存の `trait_impls` フィールド経由) と `interactor` (新規の `declares_application_service` フィールド経由) で異なる入力経路を持つ。

dogfood 検証で `RenderContractMapInteractor -.impl.-> RenderContractMap` の edge が描画された。

### D5: L4 (free function の戻り値型) の解消方針 — 2 段階対処

**解消方針**: L4 は段階的に対処した。

**(i) 視覚識別**: `LoadAllCataloguesError` のように `action=reference` で宣言されていても edge source / target に参加しない型は、`unused_reference` classDef で dashed border 表示される (D1 と同じメカニズム)。

**(ii) FreeFunction kind nodes として宣言基盤を確立**: `TypeDefinitionKind::FreeFunction` variant を catalogue schema に追加した。`load_all_catalogues` (infrastructure) および `render_contract_map` (domain) を `kind=free_function` + `expected_params` / `expected_returns` で catalogue に declare することで、これらの free function が Contract Map のノードとして表示される基盤を確立した。

**Phase 3 スコープに残る点**: FreeFunction kind の `expected_params` / `expected_returns` を読んで param / return-type edge を実際に描画する処理は Phase 3 のスコープである。Phase 2 時点では FreeFunction node は `unused_reference` dashed border で表示されるが、edge は描画されない。Phase 3 で param / return-type edge rendering が実装されると、`load_all_catalogues` から `LoadAllCataloguesError` への edge が自動的に描画される。

dogfood 検証でノード表示を確認:
```
L14_infrastructure_load__all__catalogues[load_all_catalogues]:::free_function
class L14_infrastructure_load__all__catalogues unused_reference
L6_domain_render__contract__map[render_contract_map]:::free_function
class L6_domain_render__contract__map unused_reference
```

## Rejected Alternatives

### A1: 前 ADR § Known Limitations に直接 "Resolved" 注記を追記する

前 ADR のコメントには「Phase 2 完了時に各 L# を 'Resolved in <ADR / 実装 track の識別子>' 注記で埋めて記録を残す」とあった。この方針は `knowledge/conventions/adr.md` の post-merge immutability rule (許容される編集は typo 修正・broken cross-reference 修正・newer ADR への back-reference 追加のみ) に抵触する semantic amendment であるため、実施されなかった (一度試みられたが revert 済み)。本 ADR を新規作成して記録を残すことが正式な対処である。

### A2: L4 を完全解消してから Phase 3 scope なしで記録する

L4 の free function param / return-type edge rendering を Phase 2 内で完全に実装してから記録することも考えられたが、Phase 2 のスコープ (isolated nodes の edge coverage 改善) は FreeFunction kind の宣言基盤確立で達成されており、edge rendering の追加は Phase 3 の独立したスコープとして分離する方が scope creep を防げる。

## Consequences

### 利点

1. **完全 unidentified nodes がゼロになった** — Phase 1.5 時点の「edge も dashed-border 識別も無い」9 nodes が、edge 描画 (L2b / L3) または dashed border 視覚識別 (L1 / L2a / L4) によって対処済みの状態になった。L4 FreeFunction nodes はまだ edge を持たないが `unused_reference` dashed border で意図的な宣言であることが明示されており、D5 に記載の通り Phase 3 で param/return-type edge rendering が追加されると完全解消になる。
2. **「待機中の型」と「未宣言の問題」が視覚的に区別できる** — `unused_reference` は意図的な forward-reference、`declaration_only` は method を持たない宣言型を識別し、genuine な問題を隠すことなく設計意図を伝える。
3. **FreeFunction kind が catalogue schema に入った** — 自由関数を TDDD の管理対象として宣言できるようになった。Phase 3 で edge rendering が追加されると、関数の型依存関係が Contract Map 上で可視化される。
4. **Interactor → ApplicationService 関係が Contract Map に表現できるようになった** — hexagonal アーキテクチャの interactor / application service 間の実装関係が 1 枚の図で把握できる。

### コスト / リスク

1. **`declaration_only` narrowing (D2) は型カタログ authoring のガイダンス更新が必要** — `is_method_bearing_kind()` の適用範囲を type-designer / spec-designer が把握している必要がある。
2. **L4 の Phase 3 スコープ残項が「未完了」に見える懸念** — FreeFunction node は `unused_reference` として表示されるため、Phase 3 実装前は孤立ノードと区別がつかない。Phase 3 で edge rendering が追加された段階で初めて完全解消になる。
3. **field edge は外部型を silently skip する** — CN-05 ルール (カタログ未宣言の型は edge 対象外) により、`bool` / `String` / `Option<T>` 等のフィールドは edge が発生しない。これは意図的な設計だが、フィールドの全体像が図から読めない場合がある。

## Reassess When

- **Phase 3 で FreeFunction edge rendering が実装された場合** — L4 が完全解消となるため、本 ADR の D5 (Phase 3 スコープに残る点) を参照 ADR として新 ADR に記録する。
- **`declaration_only` narrowing (D2) が運用上不便であることが判明した場合** — `is_method_bearing_kind()` の適用範囲に関する決定を見直す新 ADR を検討する。
- **field edge の silently skip 範囲 (CN-05) を変更する場合** — 外部型への edge 描画を許容する設計変更が発生した場合、D3 との整合を確認する。
