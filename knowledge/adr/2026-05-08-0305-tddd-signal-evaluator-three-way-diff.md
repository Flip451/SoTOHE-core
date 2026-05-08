---
adr_id: 2026-05-08-0305-tddd-signal-evaluator-three-way-diff
decisions:
  - id: D1
    user_decision_ref: "chat_segment:tddd-v2-signal-evaluator-design:2026-05-08"
    candidate_selection: "from:[keep-4group,two-phase-S-D-C] chose:two-phase-S-D-C"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:tddd-v2-signal-evaluator-design:2026-05-08"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:tddd-v2-signal-evaluator-design:2026-05-08"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:tddd-v2-signal-evaluator-design:2026-05-08"
    candidate_selection: "from:[static-attach-to-B,dynamic-attach-at-matching] chose:dynamic-attach-at-matching"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:tddd-v2-signal-evaluator-design:2026-05-08"
    candidate_selection: "from:[A-plus-B-plus-C-plus-CatalogueDoc,A-plus-B-plus-C-only] chose:A-plus-B-plus-C-only"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:tddd-v2-signal-evaluator-design:2026-05-08"
    status: proposed
  - id: D7
    user_decision_ref: "chat_segment:tddd-v2-signal-evaluator-design:2026-05-08"
    status: proposed
  - id: D8
    user_decision_ref: "chat_segment:tddd-v2-signal-evaluator-design:2026-05-08"
    status: proposed
---
# TDDD Signal evaluator: S / D 構築と 3-way 評価

## Context

### §1 TDDD-02 の 4 グループ評価

ADR `2026-04-11-0001-baseline-reverse-signals.md` (TDDD-02) は baseline-aware reverse signal を導入し、現在の TypeGraph (C) の各型を宣言 (A) と baseline (B) の集合関係で 4 グループに分類して評価する algorithm を確定した。

4 グループ:

- A\B: 宣言にあり baseline にない (新規追加)
- A∩B: 宣言にも baseline にも存在 (変更 / 維持 / 削除候補)
- B\A: baseline にあり宣言にない (暗黙維持)
- ∁(A∪B)∩C: A にも B にもないが C に存在 (宣言外実装)

### §2 TDDD-03 の action フィールド導入

ADR `2026-04-11-0003-type-action-declarations.md` (TDDD-03) は型エントリに `action` フィールド (add / modify / reference / delete) を追加し、action ごとに signal 条件を変える方針を確定した。`action: delete` 宣言時は B に存在することを検証し、C に存在しなければ Blue (削除完了)、C にまだ存在すれば Yellow (削除進行中) と判定する。

### §3 V2 での TypeGraph schema 移行

ADR `2026-05-08-0258-tddd-typegraph-hybrid-and-codec.md` (ADR 2) で TypeGraph schema を `rustdoc_types::Crate` ベースに移行した。Catalogue layer schema は ADR `2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` (ADR 1) で独自軽量 schema として確定した。Catalogue → TypeGraph A の codec は ADR 2 で確定した。

### §4 Signal evaluator の re-architect

ADR 1 / ADR 2 の確定後、Signal evaluator algorithm の再設計が必要になった。既存の 4 グループ評価を「S / D 構築 (Phase 1) + S / D / C 3-way 評価 (Phase 2)」に再構成することで、TDDD のゴール「C = B ∪ A 完全一致」を直接 operationally に表現できる。S = B ∪ A の operations 結果が「あるべき C の状態」を直接表現するため、Phase 2 の 3-way diff がゴール達成度を一意に判定できる。

## Decision

### D1: Signal evaluator algorithm の 2 phase 構造への再設計

Signal evaluator algorithm を 2 phase 構造に再設計する。

- **Phase 1**: S 構築 + D 構築 (catalogue declare の整合性を早期検証)
- **Phase 2**: S / D / C の 3-way 評価で signal を出す

既存の TDDD-02 4 グループ評価 (A\B / A∩B / B\A / ∁(A∪B)∩C) は本 ADR で **refine** され、Phase 1 + Phase 2 の領域分割に組み替えられる (D7 参照)。TDDD-03 の action 別 signal は本 ADR で **継承 + 細分化** される (D8 参照)。

### D2: Phase 1 (S 構築 + D 構築)

S と D は両方とも Phase 1 で構築する中間表現である。

- **S**: action catalogue (A) の宣言 (Add / Modify / Reference / Delete) を baseline (B) に適用した結果であり、**「C の最終目標地点」** を表現する。TDDD のゴール「C = B ∪ A 完全一致」の右辺を operationally に構築するため、Phase 2 では C が S と一致しているか (S ∩ C / S \ C / C \ (S ∪ D) の各領域) で signal を出す。
- **D**: catalogue で `action: Delete` 宣言された B 由来要素のみの集合であり、**「baseline から削除する項目の集合」** を表現する。Delete された要素は S からは除外され、D に集約される。Phase 2 では「D の各 Item が C にまだ残っているか / 既に消えたか」で削除の進捗を signal 化する (D ∩ C = 🟡 削除進行中、D \ C = 🔵 削除完了)。

schema は分離する。**S は ExtendedCrate** (ADR 2 D2 の `rustdoc_types::Crate + item_actions`) で、B 由来 Reference / A 由来 Add/Modify が混在するため action field を保持する必要がある。一方 **D は `rustdoc_types::Crate` 純粋** とし、各 Item は常に action=Delete で固定のため item_actions field は不要 (signal evaluator は D context で暗黙 Delete を仮定する)。

`ExtendedCrate.krate.index: HashMap<Id, Item>` の `Id` は ExtendedCrate スコープに閉じる per-graph 値であり (ADR 2 D3)、B (rustdoc 生出力) と A (codec 出力) の Id 空間は衝突する可能性がある。Phase 1 では Item を取り込む際に **types / traits は short name で、functions は FunctionPath (crate_name + module_path + name、ADR 2 D3) で identity** し、S / D 内で Id を新規 flat incremental に発番する。1 catalogue = 1 crate (ADR 1 D6) の原則により、同一 graph 内の type/trait short name と function FunctionPath はそれぞれ uniqueness が保証される。元 Item の Id は捨て、name / path → 新 Id mapping を再構築して child id reference (`Vec<Id>`、ADR 2 D8 の inline → id 参照変換結果も含む) を新 Id 空間に rebuild する。Phase 2 (3-way 評価) でも identity 判定は同一基準 (types/traits = short name、functions = FunctionPath) で行う。

`external_crates` も per-graph スコープで rebuild する。`S.external_crates` は **S に残存する Item (Delete 処理後) が参照する外部 crate を crate name で union して構築** する (B 由来 Item + A 由来 Add/Modify/Reference Item から抽出。重複は merge され、同 crate name の entry が複数現れることはない)。S スコープで `crate_id` 1, 2, ... を再発番する。crate_id == 0 は current crate (`crate_name`) に予約する (ADR 2 D5)。`D.external_crates` は **D 内の各 Item (Delete された要素) が参照する外部 crate のみで独立に構築** する (D スコープで crate_id を独立発番、lazy build 可)。S と D の external_crates は **subset 関係を持たない** — 削除によって外部 crate 依存が S 側で減るケースでは、その外部 crate が D 側にだけ残る (例: baseline で唯一 `std::fmt::Display` を impl していた `User` を Delete 宣言した場合、S から `std` 依存が消えて D に残る)。`Item` 内に登場する `Type::ResolvedPath` の `id` 参照のうち外部 crate を指すものは、各 graph の再発番後の crate_id に rebind する。

A の **未解決マーカー** (TypeRef parse 時点で catalogue 内に declare がなかった type 参照、ADR 2 D9) は Phase 1 で **closed-world 検証** に通す。Delete 処理後の S (= closed-world universe) のみを resolve 対象とすることで、削除済み型への参照を正しく reject する: `S.index` 内に同名 Item があれば S 由来 Item として resolve する (B 由来 Reference および A 由来 Add/Modify item が S に残存しており、Delete 済み型は S に存在しない)。**S に resolve 先が見つからない未解決マーカーは Phase 1 Error として reject** する (Delete 済み型への参照も含む)。これにより S は Phase 2 (3-way 評価) に進む時点で完全 resolve 状態となり、closed-world (Delete 後の S を universe set とする) が成立する。typo / 名前不一致 / 削除済み型への参照による declare 漏れはこの Phase 1 Error で検出される。

```
<!-- illustrative, non-canonical -->
S := empty ExtendedCrate (Id 0 = root module 予約、name → new Id mapping、external_crates は S 内の残存 Item が参照する外部 crate から構築・再発番)
D := empty rustdoc_types::Crate (Id 0 = root module 予約、name → new Id mapping、external_crates は D 内の Item 参照に基づき独立に構築、暗黙 Delete context)

// B 由来要素は S に Reference で attach (B 自体は不変)
// 取り込み時は types/traits = short name、functions = FunctionPath で identity し、S 内で新 Id を発番 (元 B の Id は捨てる)
for elt in B:
    new_id := S.next_id()
    S.add(elt rebuild_id_to=new_id, with action=Reference)

// A の各要素を action 別に処理 (identity 判定: types/traits = short name、functions = FunctionPath)
for elt in A:
    match elt.action:
        Reference: assert elt in S (by identity key) + 構造一致 → S 不変 (矛盾なら Error)
        Add:       assert elt not in S (by identity key) → S.add(elt rebuild_id, with action=Add) (B にあれば矛盾 → Error)
        Modify:    assert elt in S (by identity key) → S.replace_item_in_place(elt, with action=Modify) (S 内の既存 Id は維持して Item 内容を入れ替え。B になければ矛盾 → Error)
        Delete:    assert elt in S (by identity key) → D.add(S.get_by_identity(elt) rebuild_id) + S.remove(elt) (B になければ矛盾 → Error。D.add は A の catalogue element ではなく S 内の該当 Item — B 由来の baseline item — を D に移動する。D の各 Item は暗黙 Delete として扱われ、action field は持たない。)
        // identity key: types/traits = short name、functions = FunctionPath (ADR 2 D3)

// (Phase 1.5) A の未解決マーカー (ADR 2 D9) を Delete 処理後の S (closed-world universe) のみで resolve、不能なら Error
// S には B 由来 Reference 型および A 由来 Add/Modify 型が残存し、Delete 済み型は存在しない
// A.find_by_name フォールバックは使用しない (A には Delete 宣言も含まれるため、削除済み型への参照を誤 resolve する恐れがある)
for marker in A.unresolved_markers:
    if S.find_by_name(marker.name) found: S.resolve(marker, S-side Item)  // B 由来 Reference または A 由来 Add/Modify item
    else: Error("unresolved type: <name>")  // Delete 済み型への参照もここで reject される

// (Phase 1.6: dangling Id 検証)
// unresolved marker resolution 後、S 内に dangling Id (削除された Item への参照) がないかを検証する。
// Modify で参照型の field が削除 type を使わなくなった場合も、この段階で安全に validate できる。
// dangling Id が残存している場合は Phase 1 Error として reject する
// — catalogue 側で Delete 宣言する型は他の declare 型に参照されていないことが catalogue 一貫性の必要条件
```

矛盾系 (declare 整合性違反 / 未解決マーカーの resolve 不能) は Phase 1 段階で Error として reject され、Phase 2 (突合) に到達しない。catalogue 自体の declare ミス (typo / 名前不一致 / action 矛盾) を早期検出する。

### D3: Phase 2 (S / D / C の 3-way 評価) — 11 領域 × signal table

| 領域 | 条件 | signal | 解釈 |
|---|---|---|---|
| S ∩ C | 構造一致 + action=Reference | skip | 維持済み (情報量削減で出力しない) |
| S ∩ C | 構造一致 + action=Add or Modify | 🔵 | 達成 |
| S ∩ C | 構造不一致 + action=Reference | 🔴 | 参照のみのはずなのに変更されている |
| S ∩ C | 構造不一致 + action=Add | 🟡 | add 進行中 (構造未完成) |
| S ∩ C | 構造不一致 + action=Modify | 🟡 | modify 進行中 (構造変更途中) |
| S \ C | action=Reference | 🔴 | 触らない契約違反、消失 |
| S \ C | action=Add | 🟡 | 新規 declare 未実装 |
| S \ C | action=Modify | 🔴 | 変更のはずなのに削除されている |
| D ∩ C | — | 🟡 | delete 進行中 (まだ削除されていない) |
| D \ C | — | 🔵 | delete 完了 |
| C \ (S ∪ D) | — | 🔴 | declare 外実装 (declare されない実装が C に存在) |

signal 種別の分布:

- 🔴 契約違反 (即修正): S ∩ C 不一致+Reference / S \ C+Reference / S \ C+Modify / C \ (S ∪ D)
- 🟡 実装中 (進捗管理): S ∩ C 不一致+Add / S ∩ C 不一致+Modify / S \ C+Add / D ∩ C
- 🔵 達成: S ∩ C 構造一致 + action=Add or Modify / D \ C
- skip: S ∩ C 構造一致 + action=Reference (B\A 構造同一は情報量削減で出力しない)

### D4: B の `action: Reference` 動的 attach

B (Baseline、`rustdoc_types::Crate` 純粋、ADR 2 D2) は永続的に action を持たない。Signal evaluator が突合時に B を読み込んで S を構築するとき「B 全要素 = `ItemAction::Reference`」を仮定して ExtendedCrate (S) を build する。Typegraph B / C の純粋性を維持する。

### D5: Signal evaluator の入力

Signal evaluator の入力は `A` (ExtendedCrate) + `B` (rustdoc_types::Crate) + `C` (rustdoc_types::Crate) の 3 つとする。CatalogueDocument は読まない。action は A の `item_actions` に既に attach されているため (ADR 2 D2)、CatalogueDocument を別途読み込む必要がない。Phase 1 で S (ExtendedCrate) と D (`rustdoc_types::Crate`) を中間表現として構築し、Phase 2 で S / D / C を比較する。

### D6: Linter は別 component

Linter は `CatalogueDocument` の role / pattern × 構造制約を enforcement する別 component とする。Signal evaluator とは入力 (CatalogueDocument vs A+B+C) と出力 (lint signal vs Blue/Yellow/Red signal) が分かれる。Linter は本 ADR の対象外 (将来別 ADR で詳細化する)。本 ADR では Architecture 全体像における位置づけのみを示す。

```
<!-- illustrative, non-canonical -->
[CatalogueDocument (role / pattern / docs / action / 構造)]
  ├─→ [Codec] ─→ [ExtendedCrate (TypeGraph A): krate + item_actions]
  │
  └─→ [Linter] (role / pattern × 構造制約 enforcement)
                                ↓
[rust code 開始時] → rustdoc → [rustdoc_types::Crate (TypeGraph B)]
[rust code 進行中] → rustdoc → [rustdoc_types::Crate (TypeGraph C)]
                                ↓
                        [Signal evaluator]
                        Phase 1: S 構築 + D 構築 (B 由来 = Reference, A 由来 = action 別、D = 暗黙 Delete)
                                 → ExtendedCrate (S), rustdoc_types::Crate (D)
                        Phase 2: S, D, C の 3-way 評価で signal 判定
                                ↓
                         🔴 / 🟡 / 🔵 / skip
```

### D7: TDDD-02 4 グループ評価との関係 (refine)

TDDD-02 の 4 グループ評価を本 ADR で以下のように refine する。

| TDDD-02 グループ | 本 ADR Phase 1 操作 | 本 ADR Phase 2 評価領域 |
|---|---|---|
| A\B | A の `Add` action で S に追加 | S \ C + Add (🟡) / S ∩ C + Add 構造一致 (🔵) / 不一致 (🟡) |
| A∩B | A の `Modify` / `Reference` action で S に置換 / 維持、または `Delete` で D に移動 | S \ C + Modify/Ref (🔴) / D ∩ C (🟡) / D \ C (🔵) など |
| B\A | B 由来要素として S に Reference で attach (declare なし → 暗黙 Reference) | S ∩ C + Reference 構造一致 (skip) / 不一致 (🔴) / S \ C + Reference (🔴) |
| ∁(A∪B)∩C | C \ (S ∪ D) に対応 | C \ (S ∪ D) (🔴) |

TDDD-02 が B\A で「構造同一はスキップ / 構造変更 or 削除は Red」と分類していた部分は、本 ADR では `S ∩ C + Reference` の構造一致 (skip) / 不一致 (🔴) と `S \ C + Reference` (🔴) に組み替えられる。意味論は維持しつつ、領域分類を S/D/C 3-way diff という統一 framework に再構成する。

### D8: TDDD-03 action 別 signal との関係 (継承 + 細分化)

TDDD-03 の action 別 signal table (add / modify / reference / delete × C の状態 → Blue/Yellow/Red) を本 ADR で継承し、Phase 2 評価で構造一致 / 不一致を更に区別することで signal 精度を上げる。

- `S ∩ C + 不一致 + action=Reference` → 🔴 (TDDD-03 で明示的にカバーされていなかった「reference 契約のはずなのに構造変更」を新規に捕捉)
- `S \ C + action=Modify` → 🔴 (TDDD-03 の `modify` の Yellow 条件「C に存在しない = 変更途中 WIP」を本 ADR では 🔴 に変更。「変更を declare したのに削除された」と解釈し、削除の declare が必要)

これらの細分化は signal 解像度を向上させ、契約違反と実装途中をより正確に区別する。

## Rejected Alternatives

A. **TDDD-02 の 4 グループ評価をそのまま維持** — 既存実装と互換性は保たれるが、(1) Phase 1 段階での declare 整合性検証 (矛盾系 Error reject) ができず、(2) action 文脈での細分化 (reference 構造変更 など) が組み込みにくい、(3) TDDD のゴール「C = B ∪ A 完全一致」を operationally に表現していない。本 ADR の再設計で改善する。

B. **S vs C の単純 2-way diff (D を持たない)** — D を作らずに S だけで突合する案。問題: TDDD 開始時に元 delete 対象が C にまだ存在する (delete 進行中) とき、S にはこの要素が存在しないため `C \ S` 領域に該当して 🔴 になる。本来は 🟡 (delete 進行中) として扱うべき。D を別集合として保持する D2 案を採用。

C. **SourceTag enum を別途定義 (FromBaseline / FromAdd / FromModify)** — S 内の要素の出自を `SourceTag` enum で区別する案。`ItemAction` (Add / Modify / Reference / Delete、ADR 2 D2) が既に存在しており、B 由来は暗黙 `Reference`、catalogue 由来は declare の action そのまま、で ItemAction だけで完結する。SourceTag を別途定義する必要がない。ItemAction で統一する D2 案を採用。

D. **Signal evaluator が CatalogueDocument を読む (A の代替)** — TypeGraph A から TDDD-specific 情報を drop し、signal evaluator が CatalogueDocument を別途読み込む案。問題: A の id と CatalogueDocument の name の対応付けが複雑化し、突合 algorithm が 4 入力 (A + B + C + CatalogueDocument) になる。「A 構築時に action を落とすと S や D を構築できない」という問題から却下。ADR 2 D2 で ExtendedCrate (`rustdoc_types::Crate + item_actions`) を採用したことでこの問題は解消された。

E. **`S ∩ C` の構造不一致を一律 🟡 (action 別に細分化しない)** — 構造不一致を action に関わらず 🟡 (実装中) と扱う案。問題: `action=Reference` で構造変更されているケース (= 触らない契約違反) を捕捉できず、🟡 (進行中) と誤分類されて修正優先度が下がる。action 文脈で細分化する D3 を採用。

F. **`S \ C + action=Modify` を 🟡 (新規未実装と同じ扱い)** — modify 宣言した型が C に存在しない場合を「変更途中 WIP」と解釈する (TDDD-03 baseline)。問題: modify 宣言は「既存型を変更する」契約であり、C から消失したのは「削除」と等価。delete を declare していないのに削除した状態は契約違反 (= 🔴) と扱うべき。🟡 から 🔴 に変更した D3 を採用。

G. **`D ∩ C` を 🔴 (delete declare 違反扱い)** — delete 宣言した要素が C にまだ存在する場合を契約違反として 🔴 と扱う案。問題: delete 宣言自体は適切な契約であり、C にまだ存在するのは実装が進行中というだけ。TDDD-03 baseline の Yellow 判定と整合的に 🟡 (delete 進行中) と扱う D3 を採用。

H. **`D \ C` を skip (情報量削減のため出力しない)** — delete 完了状態を skip と扱う案 (B\A 構造同一の skip 方針と整合する)。🔵 (delete 完了を明示報告) を採用し、達成感と reporting の観点で skip より明示報告を優先。

I. **B に `action: Reference` を永続的に attach** — Baseline 生成時に rustdoc 出力に action を埋め込む案。問題: Typegraph B が `rustdoc_types::Crate` 純粋でなくなり、ADR 2 D2 と矛盾する。突合時動的 attach (D4) を採用。

J. **Id を graph 横断で永続化 (B の Id をそのまま S が引き継ぐ + A の codec 時に B の Id 続きから発番)** — S 構築時に B の Id をそのまま使い、A の codec が B に割り当て済みの Id 範囲を把握してその続きから発番する案。B の Id を引き継ぐことで S 内の Id 一意性は保てるが、A の codec 入力が `CatalogueDocument` のみ (ADR 2 D5) という制約に違反し、codec が B の Id 空間に依存することで codec 設計が崩れる。A codec は B に依存せず独立してビルドできる必要があり (ADR 2 D2 / D5)、S 構築時に Id を新規発番 + rebuild する D2 案を採用。

## Consequences

### 良い影響

- **declare 整合性の早期検出**: Phase 1 で矛盾系 (`add` で B にあり / `modify` / `reference` / `delete` で B になし) は Error reject され、Phase 2 (突合) に到達しない。catalogue declare ミスを早期検出できる。
- **signal table が明確**: 11 領域 × signal の table が action 文脈で一意に決まる。reference 契約違反 (構造変更) や modify 削除 (削除 declare 漏れ) のような細かいケースも明示的に捕捉できる。
- **TDDD のゴール直接表現**: S = B ∪ A の operations 結果が「あるべき C の状態」を直接表現する。`C = B ∪ A` 完全一致を Phase 2 の 3-way diff で operationally に検証できる。
- **TypeGraph 純粋性維持**: B / C は `rustdoc_types::Crate` 純粋のまま (ADR 2 D2)、突合時に動的 action attach (D4) を行うので Typegraph の永続表現が変わらない。
- **Signal evaluator の入力が 3 つに統一**: A + B + C の TypeGraph 3 つだけで signal を出せる (CatalogueDocument 別読み不要、D5)。Architecture が整理される。
- **TDDD-02 / TDDD-03 の意味論を継承**: 既存の 4 グループ評価と action 別 signal の意味論は維持しつつ、より統一された framework に再構成される (D7 / D8)。

### 悪い影響

- **既存 4 グループ評価 implementation の書き換え**: TDDD-02 の `check_consistency` (4 グループ評価) を本 ADR の Phase 1 + Phase 2 構造に書き換える必要がある。`libs/domain/src/tddd/consistency.rs` (または後継) の全面 refactor。
- **S / D 構築のメモリコスト**: 突合のたびに ExtendedCrate (S) と `rustdoc_types::Crate` (D) を構築するメモリコスト。S は B 由来要素 + A 由来要素を全部保持するため、規模に応じて B のサイズ + A のサイズ程度になる。D は Delete declare された baseline 要素のみで通常は S より小さい。大きな workspace では memory pressure に注意が必要。
- **Phase 1 構造一致確認の比較コスト**: `Reference` action での構造一致確認 (B との完全比較) は ItemAction 別比較ロジックを必要とする。`StructurallyEqual` のような equality 比較が catalogue 全要素 × B 全要素で必要。
- **既存 baseline schema との非互換**: TDDD-02 では baseline schema (`domain-types-baseline.json`) が独自 schema (`TypeBaseline`) で保存されている。ADR 2 D2 で baseline を `rustdoc_types::Crate` 純粋に変更したため、本 ADR の Phase 1 入力も rustdoc 流に変わる。既存 baseline 資産の migration が必要。
- **error reporting の表現負担**: Phase 1 の矛盾系 Error をユーザーに分かりやすく示す必要がある (例:「`add` 宣言したが baseline に既に存在する型: User」のようなメッセージ)。CLI / IDE 連携での error 表示 UX 設計が新たに必要。

## Reassess When

- 新たな action の追加要求が来た場合 (例: `Rename` で旧名と新名のペア宣言、`Move` で layer 間の型移動): Phase 1 の操作 table と Phase 2 の signal table の両方に新 action 用の列を追加する必要があり、algorithm 全体の整合性を再評価する必要がある。
- signal の細分化要求 (例: 構造一致だが docs だけ違う場合に専用 signal を出す): 本 ADR の signal table が拡張される。
- TDDD のゴール「C = B ∪ A 完全一致」の解釈が変わった場合 (例: skip 領域 (B\A 構造同一) の扱いが変わる、または部分一致を許容する): D3 の signal table と skip 規則を再評価する必要がある。
- `ItemAction` の値域が変わった場合 (ADR 1 D4 の Add / Modify / Reference / Delete から拡張): 本 ADR の Phase 1 / Phase 2 全体に影響する。
- TDDD-02 / TDDD-03 の baseline / action declaration 機構が大きく refactor された場合 (例: per-layer baseline を per-crate baseline に統合するなど): 本 ADR の入力 schema (B / C の `rustdoc_types::Crate`) が変わる可能性があり、Phase 1 の処理を再設計する必要がある。
- 外部 crate 依存の意図的トラックが必要になった場合 (例: catalogue で「`X` crate を新規依存として追加する」を明示宣言したい / 外部依存変更を ADR 由来で意図的に管理したい場合): 現状 (α 案) では `S.external_crates` は S 内の残存 Item (Delete 後) が参照する外部 crate から自動構築しており、catalogue 側で外部 crate を明示 declare する schema は持たない。意図的トラックが必要なら、`CatalogueDocument` に `external_crates: Vec<ExternalCrateDecl>` (Action 付き、差分のみ宣言) を追加する schema 拡張 (β 案) への移行を要評価。ADR 1 D6 / 本 ADR D2 (Phase 1 構築) / ADR 2 D5 の挙動変更を伴うため専用 ADR で判断する。

## Related

- 本 ADR の前提:
  - `knowledge/adr/2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` — ADR 1 (Catalogue layer schema)。`ItemAction` の 4 値 (Add / Modify / Reference / Delete) を本 ADR の Phase 1 操作で使用。
  - `knowledge/adr/2026-05-08-0258-tddd-typegraph-hybrid-and-codec.md` — ADR 2 (TypeGraph hybrid + codec)。ExtendedCrate / Baseline / Current schema を本 ADR の入力として使用。
- 既存 ADR との関係:
  - `knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md` — TDDD-02 (4 グループ評価)。本 ADR で refine され、Phase 1 + Phase 2 の領域分割に再構成される (D7)。
  - `knowledge/adr/2026-04-11-0003-type-action-declarations.md` — TDDD-03 (action 別 signal)。本 ADR で継承 + 細分化され、構造一致/不一致 × action の組み合わせで signal 精度が向上する (D8)。
  - `knowledge/adr/2026-04-26-0855-tddd-feature-extension-with-verification.md` — Core invariant ADR (catalogue / TypeGraph / baseline / serde codec の 4 点同時更新)。本 ADR は signal evaluator を再定義する形で Core invariant を維持。
- 後続 ADR:
  - Linter ADR (本 ADR の対象外、CatalogueDocument の role / pattern × 構造制約の enforcement)
- `knowledge/adr/README.md` — ADR 索引
