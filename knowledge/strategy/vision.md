# SoTOHE-core ビジョン v6

> **作成日**: 2026-04-13 (採用: 2026-04-15)
> **前版**:
> - v5 draft: `knowledge/strategy/TODO-PLAN-v5-draft.md` (2026-04-07, 未採用、併存継続)
> - v4 作業 draft: `tmp/vision-v4-2026-04-13.md` (本 draft の予備考察、保存)
> - v3 正式版: `tmp/archive-2026-04-13/vision-v3.md` (2026-03-22)
> **ステータス**: 採用版 (2026-04-15 に v3 から昇格)
> **変更理由**:
> 1. **SoTOHE 原点回帰**: 初版 README (コミット `3e817d8`) の名称概念を継承しつつ、v6 では **"Source of Truth Oriented Harness Engine"** として再宣言 (初版の「Single」を外し、複数 SoT の独立共存を許容)
> 2. **v5 draft の全要素を継承**: 2 信号機 / Phase 2c (domain-types.json 分離) / Behaviour Harness / Fowler Taxonomy / sotp スタンドアロン化 / Drift Detection / Reviewer Calibration / GitHub Native Harness / Harness Template 展開
> 3. **TDDD 統合** (v5 後の展開): TDDD-01/02/03 (ADR `2026-04-11-0001/0002/0003`) で導入された multilayer / シグネチャ検証 / baseline / action 宣言を vision に位置付ける
> 4. **SoT Chain**: 4 層 SoT (ADR ← spec ← 型カタログ ← 実装) の一方向参照チェーンにより、仕様と実装のドリフトを**構造的に**防止する
> 5. **ハーネス自身にも TDDD + typestate-first**: v3/v5 の「ハーネスは trait 維持、typestate 不要」方針を反転。新規コードは first、既存は段階リファクタリング

> **命名の注意**: 「TDDD (Type-Definition-Driven Development)」は仮称。本 vision では現行用語のまま記述し、後続 track で正式名称を確定する。候補の整理は §3.6 末尾を参照。

---

## 0. SoTOHE 原点回帰

SoTOHE = **S**ource **o**f **T**ruth **O**riented **H**arness **E**ngine

初版 README (2026-03-11 頃のコミット `3e817d8`) でこの名前の由来が明記されていた:

> **S**ingle Source **o**f **T**ruth **O**riented **H**arness **E**ngine
>
> Rust ベースの AI 開発オーケストレーションハーネス。仕様駆動開発（SDD）ワークフローを型安全な状態遷移と厳格な品質ゲートで管理する CLI テンプレート。

初版の哲学 (v1):

> **プログラムの真の Single Source of Truth は 3 要素で構成される**:
>
> | 要素 | 定義するもの |
> |---|---|
> | 型 (enum/struct) | 何が存在しうるか — 状態空間の制約 |
> | trait | 何ができるか — 振る舞いの契約 |
> | テスト | 具体的にどう動くか — 入出力の証拠 |
>
> **実装はこれらに従う導出物**

v1 → v2 → v3 → v4 → v5 の進化で以下が追加された:
- v2: 信号機 (🔵🟡🔴) による spec 信頼度の可視化
- v3: ハーネス自身とテンプレート出力の区別
- v4 draft: sotp CLI のスタンドアロン化 (物理分離)
- v5 draft: Fowler Taxonomy / Anthropic Three-Agent 対応 / 2 信号機 / Phase 2c (domain-types.json 分離) / Drift Detection / Harness Template

この過程で「Single Source of Truth Oriented」という一次コンセプトが薄まり、README 名称も v3 で "Spec-driven orchestration and Test-generation Oriented" に書き換えられた。

**v6 の宣言**: SoT を第一級の概念に戻す。v1 の「型 + trait + テスト = SoT」を **4 層の SoT + SoT Chain** に構造化し、ドリフト防止を核心価値に据える。

### 0.1 SoTOHE の Moat = SoT Chain

v6 では SoTOHE-core の **Moat (競争優位の中核)** を **SoT Chain** と呼ぶ。

| 版 | Moat | 構成要素 |
|---|---|---|
| v2 | 仕様品質 | spec 信号機 |
| v3 | テスト生成パイプライン | 型 + spec + テストスケルトン |
| v4 draft | sotp スタンドアロン化 | CLI 物理分離 |
| v5 draft | Behaviour Harness (Fowler) | 2 信号機 + Phase 3 テスト生成 |
| **v6** | **SoT Chain** | **4 層 SoT + 一方向参照 + TDDD + Stage 1/2 信号機** を 1 つの名前に統合 |

v5 までの Moat は個別の機能・概念の集合だったが、v6 で **統合された一つの名前 = SoT Chain** が生まれた。これが v6 の決定的な進化である。

SoT Chain は以下の 4 要素 + 1 つの動作原理で構成される:

- **4 層の独立 SoT**: ADR / spec / 型カタログ / 実装 — 各層は独立したライフサイクルを持つ (§2)
- **TDDD (仮称)**: 型カタログ層の実装手法 — TDDD-01/02/03/04 (§3)
- **Stage 1 信号機**: spec 層の CI ゲート (v5 継承)
- **Stage 2 信号機**: 型カタログ層の CI ゲート (v5 継承 + TDDD multilayer 拡張)

**動作原理**: 下流 → 上流の一方向参照 + 参照漏れを Red で merge ブロック (§1 で詳述)

**TDDD は SoT Chain の実装手法** (型カタログ層の一部) であり、名前階層の最上位ではない。上位概念 (SoT Chain) が既に名前を持っているため、TDDD は純粋に「型カタログ手法」を指す名前で十分である (§3.6 命名メモ参照)。

### 0.2 SoT Chain を実装する Phase 群

v5 では Phase 3 (テスト生成パイプライン) 単独が Moat だったが、v6 では以下の Phase 群が **1 つの Moat = SoT Chain** を構成する:

| Phase | SoT Chain における役割 |
|---|---|
| Phase 1.5 (HARN-TDDD-00〜05) | ハーネス自身を SoT Chain 規律下に置く |
| Phase 2 (Stage 1 信号機) | SoT Chain の 1 番目のリンク (spec → ADR) |
| Phase 2c (TDDD-01/02/03) | SoT Chain の 3 番目のリンク (型カタログ → 実装) |
| Phase 2d (TDDD-04) | SoT Chain の 2 番目のリンク (型カタログ → spec) |
| Phase 3 (Behaviour Harness) | SoT Chain 上でテスト生成 4 手法を提供 |

Phase 3 は v6 で「SoT Chain をテストに変換する層」として再位置付けされる。

---

## 1. SoT Chain の動作原理

§0.1 で SoT Chain を Moat の名称として宣言した。本章ではその動作原理 — **仕様と実装のドリフトを構造的に防止する 4 層一方向参照** — を詳述する。

### 1.1 仕様と実装のドリフトという根本問題

AI 支援開発の最大の落とし穴は **spec と code が独立に進化して乖離する**ことにある:

- AI は古い spec を読んで「仕様通り」と誤判定する
- レビュアーは code の差分だけ見て spec 側の未更新に気付かない
- 新規参入者は spec と code の乖離に迷う
- レビュー回数が爆発する (歴史的教訓: 15+ rounds from skipped design)

この問題は「文書を更新しろ」という運用ルールでは解けない。**構造的に参照を強制**しなければ、どんな規律も AI 開発のスピードには勝てない。

### 1.2 SoT Chain の定義

SoTOHE v6 では、4 層の SoT を **一方向の参照チェーン** で結ぶ:

```
ADR (不変の設計決定)
  ↑ [source: ADR-...]                    Stage 1 信号機 SignalBasis (v5 既存)
spec.json / spec.md (要件の SoT)
  ↑ spec_source                          v6 新規 (TDDD-04 で必須化)
<layer>-types.json (型契約の SoT)
  ↑ rustdoc JSON + TypeGraph             v5/v6 既存 (TDDD forward/reverse check)
libs/<layer>/src/**/*.rs (実装)
```

各矢印は **「下流 → 上流」の単方向リンク**:

| 矢印 | 意味 | 検証 |
|---|---|---|
| 実装 → 型カタログ | コードの型は型カタログに宣言されている | TDDD forward (Yellow) / reverse (Red) — 既存 |
| 型カタログ → spec | カタログの各エントリは対応する spec セクションを参照する | `spec_source` 必須フィールド — **v6 新規 (TDDD-04)** |
| spec → ADR | spec の要件は ADR を source として明記する | Stage 1 信号機 SignalBasis `[source: ADR-...]` — v5 既存 |

### 1.3 ドリフト防止の 2 層ゲート

**第 1 層: コンパイラ**
- Rust の型参照整合性 (`UserId` が domain にある → usecase から参照できる)
- `check-layers` による層依存の静的検証

**第 2 層: CI 信号機 + SoT Chain**
- TDDD forward/reverse: 宣言と実装の一致
- spec_source: 型カタログと spec の紐付け
- Stage 1 信号機: spec と ADR の紐付け
- `verify spec-states`: 全層 AND 集約、Red があれば merge ブロック

**参照漏れの全パターンが CI で Red になる**:

| 状況 | どこで検出されるか |
|---|---|
| 未宣言の型を実装 | TDDD reverse check → Red |
| 宣言だけあり実装がない | TDDD forward check → Yellow (途中) / merge 時 fail |
| カタログに spec_source がない | spec_source 必須検証 → Red (新規) |
| spec_source の参照先が存在しない spec セクション | dangling 検証 → Red (新規) |
| spec_hash が stale | tamper 検出 → Red (新規) |
| spec の要件に source tag がない | Stage 1 信号機 → Red |

これにより「spec を更新せずに code を変える」or「code を変えずに spec を更新する」はどちらも構造的に不可能になる。

### 1.4 逆方向の更新フロー

読む方向は下流 → 上流だが、**更新は上流 → 下流に伝播**する:

```
ADR 追加/変更 → spec の source tag 更新 → カタログ更新 → コード実装
```

探索的 (exploratory) に実装から始めたい場合:
1. プロトタイプを別 branch で自由に書く
2. 収束したら spec セクションに起こす
3. spec を ADR の source に紐付ける (新 ADR を書くことも多い)
4. カタログに型を宣言 + `spec_source` を設定
5. コードを main の TDDD 規約下に書き直す

**exploratory branch はSoT Chainの外側で自由**。main に入れる段階で鎖に接続する。

---

## 2. 4 層の SoT + 独立ライフサイクル

### 2.1 層構造

v5 で「2 つの信号機」(仕様書 Stage 1 + 型設計書 Stage 2) が導入された。v6 ではこれを 4 層に発展させる:

| 層 | SoT ファイル | 検証 | ライフサイクル |
|---|---|---|---|
| **ADR** | `knowledge/adr/*.md` | 手動レビュー + 不変性 + dangling ref 検出 | 意思決定時に追加、原則不変 |
| **仕様書** | `spec.json` / `spec.md` (rendered view) | **Stage 1 信号機** (🔵🟡🔴, SignalBasis, v5 継承) | ヒアリング + 承認サイクル、承認後は凍結 |
| **型契約** | `<layer>-types.json` + `<layer>-types-baseline.json` | **Stage 2 信号機** (🔵🟡🔴, TDDD multilayer, シグネチャ検証, baseline, action, spec_source) | 実装進捗に応じて更新 |
| **実装** | `libs/<layer>/src/**/*.rs` | clippy / fmt / test / check-layers / usecase-purity / domain-purity | 継続的更新 |

v5 の「Stage 1 + Stage 2」は、v6 では **4 層のうち中央 2 層** に相当する。ADR 層と実装層を加えることで完全なSoT Chainが成立する。

### 2.2 独立ライフサイクルの原則

各層は独立したライフサイクルを持ち、他層の変更で不必要に触られない:

- 仕様書の **凍結** (承認後) と実装の **進捗** (日々更新) は分離
- 型契約の更新で仕様書 content hash を無効化しない
- 実装の更新で型契約 content hash を無効化しない

v5 ADR `2026-04-07-0045` で示された動機 (ライフサイクル分離) がそのまま 4 層にも適用される。

### 2.3 SSoT の 2 原則

v6 では SoT (Source of Truth) の原則を **2 つ** に拡張する:

1. **Single Authority, not Single File** (v5 継承): 各情報の正規の置き場所が 1 つであれば SoT は成立する
2. **Referenced, not Isolated** (v6 新規): 各 SoT は上流 SoT を必須参照し、下流 SoT から参照される。独立した島は SoT ではない

| 情報 | SSoT | ファイル | 上流参照 | 下流被参照 |
|---|---|---|---|---|
| 設計決定 | ADR | `knowledge/adr/*.md` | — (ルート) | spec `[source: ADR-...]` |
| 要件 | spec.json | `track/items/<id>/spec.json` | `[source: ADR-...]` | `<layer>-types.json.spec_source` |
| 型契約 | `<layer>-types.json` | `track/items/<id>/<layer>-types.json` | `spec_source` | rustdoc JSON scan |
| トラック状態 | metadata.json | `track/items/<id>/metadata.json` | — (横断管理) | plan.md view |
| タスク分解 | plan.md | metadata.json から生成される view | — | — |
| 検証結果 | verification.md | 独立ファイル | — | — |
| アーキテクチャ規則 | architecture-rules.json | workspace ルート | — | check-layers / TDDD layer discovery |
| 型ベースライン | `<layer>-types-baseline.json` | track ディレクトリ (read-only) | — (スナップショット) | TDDD check_consistency |

---

## 3. 4 つの信号機 + TDDD multilayer

### 3.1 v5 の 2 信号機アーキテクチャ (継承)

v5 draft の以下の章をそのまま継承する (ADR `2026-04-07-0045` + `2026-03-23-2120`):

```
┌─────────────────────────────────────────────────────────────────┐
│  仕様書の信号機 (Stage 1)                                       │
│  ファイル: spec.json / spec.md                                   │
│  評価対象: 要件の出典 (source tags)                              │
│  信号: Blue / Yellow / Red (3値)                                 │
│  ゲート: red == 0 で通過                                         │
│  SSoT: spec.json → spec.md (rendered view)                      │
│  ADR: 2026-03-23-1010-three-level-signals.md                    │
│                                                                  │
│  Blue:   出典が信頼できるドキュメントに紐づく                    │
│  Yellow: 出典があるが確証が弱い (要確認)                         │
│  Red:    出典なし / プレースホルダー (ブロック)                  │
│                                                                  │
│  共有プリミティブ: ConfidenceSignal + SignalCounts               │
│  Stage 1 固有: SignalBasis (出典の理由を追跡)                    │
└─────────────────────────────────────────────────────────────────┘
        ↓ Stage 1 通過が前提条件
┌─────────────────────────────────────────────────────────────────┐
│  型設計書の信号機 (Stage 2)                                      │
│  ファイル: <layer>-types.json / <layer>-types.md                 │
│  評価対象: 型宣言と実装の一致度 (forward + reverse)              │
│  信号: Blue / Yellow / Red (v6 で Yellow 復活: WIP 許容)         │
│  ゲート: red == 0 で通過 (merge 時は yellow もブロック)          │
│  SSoT: <layer>-types.json → <layer>-types.md (rendered view)    │
│  ADR: 2026-04-07-0045, 2026-04-08-1800, 2026-04-11-0001/0002/0003│
│                                                                  │
│  Blue:   宣言と実装が一致                                        │
│  Yellow: 宣言済み未実装 (WIP) / 構造不一致 / action 不整合       │
│  Red:    未宣言の実装 / 未宣言の削除 / spec_source 漏れ          │
│                                                                  │
│  入力: TypeGraph (rustdoc JSON) + TypeBaseline + spec.json       │
│  12 variants: TypeDefinitionKind (domain + application 層)       │
│               Typestate/Enum/ValueObject/ErrorType/              │
│               SecondaryPort/ApplicationService/UseCase/          │
│               Interactor/Dto/Command/Query/Factory               │
│               (enum-first, ADR 2026-04-13-1813 で 5 → 12 拡張)   │
│  v6 拡張: multilayer / シグネチャ検証 / baseline / action /       │
│            spec_source                                           │
└─────────────────────────────────────────────────────────────────┘
        ↓ 両方通過 + ADR dangling ref ゼロが Phase 3 / merge の前提
┌─────────────────────────────────────────────────────────────────┐
│  CI ゲート (信号機ではない)                                      │
│  coverage (spec-coverage): 二値。信号機の 3 段階に馴染まない    │
│  ADR: 2026-03-24-0900-coverage-not-a-signal.md                  │
└─────────────────────────────────────────────────────────────────┘
```

**v5 からの変更点**:

- Stage 2 で Yellow を復活 (v5 では Blue/Red 2 値化の方針だったが、TDDD-02 の 4 グループ評価で Yellow が必要になった)
- ファイル名が `domain-types.json` → `<layer>-types.json` (multilayer 対応)
- 入力に `TypeBaseline` + `spec.json` が追加
- TypeDefinitionKind に `MethodDeclaration` 構造が追加 (シグネチャ検証)

### 3.2 TDDD multilayer + シグネチャ検証 (v6 新規)

v5 の Stage 2 は `domain-types.json` (単一層、メソッド名のみ) だった。v6 では TDDD-01 (ADR `2026-04-11-0002`) により以下に拡張される:

**Multilayer**:
- `architecture-rules.json` の `layers[].tddd` ブロックで任意の層に適用
- `catalogue_file` で層ごとの命名 (デフォルト: `<crate>-types.json`)
- `schema_export.targets` で多 crate 層にも対応
- `sotp track type-signals --layer <id>` で層別評価
- `verify spec-states` が全 `tddd.enabled` 層の AND 集約で判定

**シグネチャ検証**:
- `expected_methods: Vec<String>` → `Vec<MethodDeclaration>` に拡張
- JSON スキーマ: `{ name, receiver, params: [{ name, ty }], returns, async }`
- 引数型・戻り型まで完全マッチで検証 → **primitive obsession を機械的に検出**
- 例: カタログ `params[0].ty = "UserId"` vs 実装 `fn find_by_id(&self, id: i64)` → Yellow (シグネチャ不一致)

cargo は `i64` を有効な型として通すが、TDDD は設計意図との不一致として捕まえる。これが TDDD の最大の実用価値。

### 3.3 Baseline 4 グループ評価 (TDDD-02, ADR `2026-04-11-0001`)

成熟したコードベースで「本 track で変化した型」だけを検証するため、`/track:design` 時に TypeGraph のスナップショット (`<layer>-types-baseline.json`) を取得する。

4 グループ評価 (A = 宣言カタログ、B = baseline、C = 現在のコード):

| グループ | 意味 | check |
|---|---|---|
| A\B | 新規型 (宣言あり・baseline なし) | forward (Blue/Yellow) |
| A∩B | 既存型の変更 (宣言あり・baseline あり) | forward (Blue/Yellow) |
| B\A | 既存型・今回は触らない (宣言なし・baseline あり) | reverse (構造同一ならスキップ、変更/削除は Red) |
| ∁(A∪B)∩C | 未宣言の新規型 | reverse (常に Red) |

これにより、成熟コードベースでも 100+ の既存型ノイズを排除し、本 track で変化した型だけを検証できる。

### 3.4 Action 宣言 (TDDD-03, ADR `2026-04-11-0003`)

各カタログエントリに optional な `action` フィールド:

| action | 意味 | forward check |
|---|---|---|
| `"add"` (default) | 新規追加 | C に存在し宣言と一致 → Blue |
| `"modify"` | 既存変更 | 同上 |
| `"reference"` | 参照目的の転記 | 同上 |
| `"delete"` | 意図的削除 | C に **存在しない** → Blue |

これにより、意図的な削除と事故的な削除を区別でき、TDDD と既存型削除の併用が可能になる。

### 3.5 spec_source 必須化 (v6 新規・TDDD-04)

SoT Chainの **2 番目のリンク** (型カタログ → spec) を強制するため、型カタログの各エントリに必須 `spec_source` フィールドを追加する:

```json
{
  "name": "UserRepository",
  "kind": "secondary_port",
  "spec_source": {
    "spec_section": "## Domain States > UserRepository",
    "spec_hash": "sha256:3a7b..."
  },
  "action": "add",
  "expected_methods": [
    {
      "name": "find_by_id",
      "receiver": "&self",
      "params": [{ "name": "id", "ty": "UserId" }],
      "returns": "Result<Option<User>, DomainError>",
      "async": false
    }
  ]
}
```

**CI 検証 (TDDD-04 で実装)**:

| 検証 | 失敗時 |
|---|---|
| `spec_source.spec_section` が spec.json に存在 | **Red** (dangling ref) |
| `spec_source.spec_hash` と現在の spec.json content hash が一致 | **Red** (tamper / stale) |
| spec.json の Domain States セクションのうち、どの型カタログからも `spec_source` で参照されていないもの | **Red** (orphan spec) |
| 各カタログエントリが `spec_source` を持つ | **Red** (必須欠落) |

**spec_source の粒度は後続 track で詰める**:
- section header ベース? spec.json 内の要素 id ベース?
- sha256 hash の spec 全体 or セクション単位?
- Phase 2d (TDDD-04) の設計で確定

### 3.6 TDDD 命名メモ (後続 track で確定)

「TDDD (Type-Definition-Driven Development)」は仮称。**TDDD は SoT Chain の型カタログ層を実装する手法** であり、上位概念 (SoT Chain) が既に Moat 名称を持っているため、TDDD の命名は純粋に「型カタログ手法」を指す名前で十分である。

**候補 (v6 で絞り込み済み)**:

| 軸 | 候補 | 狙い |
|---|---|---|
| 型先行 | **Type-First Development (TFD)** | 最短・直感的、TDD との混同なし |
| カタログ駆動 | **Catalogue-Driven Development (CDD)** | カタログを前面、SoT Chain の部品として自然 |
| TDD アナロジー | **Type-Level TDD (TL-TDD)** | Red-Green ↔ Red-Yellow-Blue、TDD の認知度を活用 |

**候補から削除** (v6 で除外):

- ~~**SoT-Chain Driven Development (SCD)**~~ — **SoT Chain は既に Moat 名称**。TDDD をこの名前で呼ぶと上位概念と下位概念の名前が重複し、階層が崩れる
- ~~Type-Contract-Driven (TCD)~~ — Design-by-Contract との類似で曖昧化

v6 では仮称「TDDD」のまま記述する。命名確定は専用 track で実施する (候補は上記 3 つに絞り込まれた)。

---

## 4. Behaviour Harness と Fowler Taxonomy (v5 継承 + v6 拡張)

**v6 の位置付け**: Fowler の "Behaviour Harness" を、v6 では **SoT Chain として実装** する。v5 では「2 信号機 + テスト生成」だった Behaviour Harness の中身が、v6 で「SoT Chain (4 層 SoT + 一方向参照 + TDDD + 2 信号機)」に発展した。つまり Fowler 対応表における **Behaviour Harness = SoT Chain**。

v5 draft の対応表を継承し、v6 の変更点を追記:

```
Fowler: Guides (Feedforward)        → .claude/rules/, conventions, architecture-rules.json
Fowler: Sensors (Feedback)          → CI gates (computational) + Codex reviewer (inferential)
Fowler: Maintainability Harness     → clippy, fmt, deny, check-layers, usecase-purity, domain-purity ← 成熟
Fowler: Architecture Fitness        → architecture-rules.json + check-layers ← 成熟
Fowler: Behaviour Harness           → 4 層 SoT + 一方向参照 + TDDD + Phase 3 テスト生成
                                       (v5: 2 信号機 + テスト生成 から発展)
Fowler: Harnessability              → 04-coding-principles.md + TDDD + typestate-first
                                       (v5: enum-first / newtype のみ → v6 で TDDD + typestate を統合)
Fowler: Harness Template            → Phase 6 で展開予定 (トポロジー別: CLI / Web API / Event-driven / Library)
Fowler: Continuous Monitoring       → Phase 4 の DRIFT-01/02 (arch-drift / staleness)
Anthropic: Plan/Generate/Evaluate   → planner (Claude Opus) / implementer / reviewer (Codex) 分離
Anthropic: Evaluation Calibration   → Phase 5 の CALIB-01 (severity criteria + few-shot examples)
```

v6 の変更点:
- **Behaviour Harness** の定義を「2 信号機 + テスト生成」から「**4 層 SoT + 一方向参照 + TDDD + テスト生成**」に発展
- **Harnessability** に TDDD + typestate-first を明示的に含める

---

## 5. 探索的精緻化ループ (3 フェーズ)

README §「探索的精緻化ループ」と同じステートマシン図を使用する:

```
[ADR]
  ↑ ↓ ①
[仕様書]
  ↑ ↓ ②
[型契約]
  ↑ ↓ ③
[実装]
```

- **↓ フェーズ進行**: ① `/track:plan`、② `/track:design`、③ `/track:implement` (または `/track:full-cycle`) による成果物作成、または強制退行からの自動復帰
- **↑ フェーズ退行**: 🔴/🟡 を 🔵 にするため、前の成果物に戻って修正する (信号機評価に基づく強制遷移)

### 5.1 3 フェーズの流れ

| Phase | コマンド | 成果物 | 鎖のリンク |
|---|---|---|---|
| **A: 要件スケッチ** | `/track:plan <feature>` | `spec.json` / `spec.md` + Domain States + `[source: ADR-...]` tags | spec → ADR |
| **B: 型デザイン (TDDD カタログ編集)** | `/track:design` | `<layer>-types.json` (12 variants: typestate / enum / value_object / error_type / secondary_port / application_service / use_case / interactor / dto / command / query / factory) + シグネチャ検証 + `spec_source` + `action` + baseline | カタログ → spec |
| **C: 実装** | `/track:implement` / `/track:full-cycle <task>` | 実装コード。signals が 🟡 → 🔵 に遷移 | カタログ → 実装 |

**注**: v5 以前は「Phase C: 型 + テストスケルトン生成」を独立フェーズとして計画していたが、v6 では README と整合させて削除。テストスケルトン生成は将来のロードマップ項目「SoT Chain をテストコードまで拡張」として残す。

### 5.2 各フェーズのゲート

| Phase | 通過条件 |
|---|---|
| A | Stage 1 信号機 Red ゼロ (全要件が source tag を持つ) |
| B | Stage 2 信号機 Red ゼロ (全エントリが `spec_source` を持ち、spec 側 orphan がない) + baseline-capture 完了 |
| C | Stage 2 全 Blue + 既存 CI (clippy / test / etc) |

### 5.3 Planner Gate の必須化 (変更なし)

`/track:plan` は全 track で必須 (Quick/Focused/Full モード可)。`/track:design` は TDDD 対象層がある track では必須。

---

## 6. ハーネス自身にも TDDD + typestate-first (v3/v5 反転)

### 6.1 v3/v5 の方針 (アーカイブ)

> v3: ハーネス自身のコードは「実用的に良いコード」であればよい。typestate パターンは適用しない。trait ベース DI を維持。
> v5 Phase 1.5 注意書き: 「ハーネス自身の品質改善。typestate や impl Fn への移行は不要」

### 6.2 v6 の新方針

**反転の理由**:

1. **TDDD の multilayer 化** (ADR `2026-04-11-0002`) により、任意層にカタログ + シグネチャ検証が適用可能になった
2. ハーネス自身が既に複雑な状態遷移を持ち、**既存実装が事実上 typestate / enum-first になっている** (`Verdict`, `ReviewGroupState`, `GroupRoundVerdict`, `CodeHash` 等)
3. **SoT Chainを機能させる**には、ハーネス自身も同じ規律で書かれている必要がある (鎖の外側に内側を守らせることはできない)
4. `04-coding-principles.md` の enum-first / typestate パターンは既にハーネス新規コードで自然に採用されている — 明示的な方針化にすぎない

### 6.3 段階適用の原則

| 対象 | v6 方針 |
|---|---|
| **新規コード** | TDDD カタログ先行 + typestate-first で書く (`/track:design` で先に宣言) |
| **既存コード** | 後続 track で段階的に TDDD + typestate-first にリファクタリング |
| **既存の trait ベース DI** | そのまま維持 (`impl Fn` への強制移行はしない) |
| **ファイル分割** | DDD 概念で整理 + `tddd/` サブモジュール集約 |
| **multilayer 適用範囲** | domain + usecase を `enabled: true`、infrastructure / cli は `enabled: false` から段階的に開放 |

### 6.4 リファクタリング優先度

1. **優先度高**: 状態遷移が明確なモジュール (review, pr, track, verdict)
2. **優先度中**: value object が多いモジュール (id 型, hash 型, timestamp 型)
3. **優先度低**: 純粋データ型の多いモジュール (config, path)

**Phase 1.5 残タスクとの統合**: v5 の CLI-01 / ERR-09b / RVW-01 / RVW-02 等は、各 track で「型カタログ宣言 + リファクタリング」を同時実施する。結果として Phase 1.5 の成果物はすべて TDDD 対応版になる。

### 6.5 v3/v5 との両立点

- **テンプレート出力の品質最大化** (v3/v5 の方針) は維持 — むしろ TDDD で強化される
- **ハーネス vs テンプレート出力の区別** は「適用タイムライン」の差に絞られる
  - テンプレート出力: 100% TDDD + typestate (新規プロジェクト)
  - ハーネス: 新規コード 100% + 既存段階移行
- 両者は同じ TDDD + typestate-first 哲学で統一される

---

## 7. sotp スタンドアロン化 (v5 継承)

### 7.1 v5 の方針 (継承)

v4 draft → v5 draft で導入された sotp スタンドアロン化方針をそのまま継承する:

- **sotp = 独立 CLI ツール**: テンプレートに埋め込まれるのではなく、`cargo install sotp` or `cargo-binstall sotp` でインストール
- **テンプレート = sotp ユーザープロジェクトの基盤**: sotp を呼び出す側
- **Cargo workspace**: sotp グループ (libs/domain, usecase, infrastructure, apps/cli) とテンプレートグループ (apps/server) の論理境界
- **Phase 1.5 SPLIT-01/02**: 論理分離 + `bin/sotp` パス抽象化
- **Phase 4 SPLIT-03/04/05**: crate 公開準備 + バイナリ配布 + Dockerfile 化
- **Phase 6**: テンプレートリポ分割 (sotp ソースを完全除去)

### 7.2 v6 での sotp の core value

v6 で sotp が提供する core value は以下の 3 点:

1. **TDDD + SoT Chainの CI ゲート実装**: `sotp verify spec-states` + `sotp track type-signals` + `sotp track baseline-capture`
2. **Behaviour Harness のテスト生成**: `sotp <layer> export-schema` + テストスケルトン生成 (Phase 3)
3. **track workflow の orchestration**: `sotp track plan / design / implement / review / commit / pr / merge / done`

---

## 8. Harness Template 展開 + 多言語 (v5 継承)

v5 の Phase 6 TMPL-01 + 多言語計画をそのまま継承する。

### 8.1 トポロジー別 Harness Template

| # | トポロジー | 特徴 | guides / sensors の差分 |
|---|---|---|---|
| 1 | **CLI ツール** | 現在の SoTOHE-core 自身 | 既存ハーネスがそのまま使える |
| 2 | **Web API (REST/gRPC)** | async runtime, DB, HTTP | API contract test, schema drift 検出 |
| 3 | **Event-driven service** | message queue, eventual consistency | saga テスト, idempotency 検証 |
| 4 | **Library crate** | pub API stability, semver | public API diff, breaking change 検出 |

各トポロジーに対して `sotp init --topology <name>` で適切な guides + sensors + 型カタログ初期値が scaffold される。

### 8.2 多言語展開

TDDD + テスト生成パイプライン (`sotp <layer> export-schema` + `<layer>-types.json` + テストスケルトン) は、言語別の schema exporter に差し替えれば他言語にも適用可能:

- **Rust**: rustdoc JSON (既存)
- **TypeScript**: `tsc --declaration` の `.d.ts` をパース
- **Python**: `mypy` の AST または stub files
- **Go**: `go/ast` + `go/types`

`architecture-rules.json` の `tddd.schema_export.method` で言語別 exporter を指定する設計は v5 で確定済み。

検証レベル Gold/Silver/Bronze の枠組みは維持。

---

## 付録 A: v5 → v6 の変更点

| 観点 | v5 | v6 |
|---|---|---|
| **Moat 名称** | Behaviour Harness (Fowler Taxonomy) | **SoT Chain** (4 層 SoT + 一方向参照 + TDDD + 2 信号機 を 1 つの名前に統合) |
| **Moat の粒度** | Phase 3 単独が Moat | **Phase 1.5/2/2c/2d/3 全体** が SoT Chain (1 つの Moat) を構成 |
| **名前由来** | 言及なし | **Source of Truth Oriented への原点回帰** (初版の「Single」を外し、複数 SoT 許容) |
| **信号機アーキテクチャ** | 2 つ (仕様書 + 型設計書) | **SoT Chain**に発展 (ADR / spec / 型カタログ / 実装) |
| **Stage 2 の信号** | Blue/Red 2 値 (Yellow 廃止) | **Blue/Yellow/Red 3 値** (Yellow 復活、WIP 許容) |
| **型カタログのファイル** | `domain-types.json` (単一層) | `<layer>-types.json` (multilayer) |
| **型カタログの検証精度** | メソッド名のみ | **シグネチャ検証** (引数型・戻り型まで、primitive obsession 検出) |
| **既存型ノイズ対策** | 全型を対象 (成熟コードでノイズ多) | **Baseline 4 グループ評価** (per-track 増分) |
| **型削除と TDDD の併用** | 不可 (制約あり) | **action 宣言で可能** (`"delete"` を明示) |
| **型カタログと spec の紐付け** | 未定義 | **`spec_source` 必須化 (TDDD-04)** |
| **ハーネス自身の typestate 適用** | 「不要」 | **新規 first + 既存段階リファクタ** |
| **Behaviour Harness の定義** | 2 信号機 + テスト生成 | **4 SoT + SoT Chain + TDDD + テスト生成** |
| **ドリフト防止** | Drift Detection (Phase 4) | **SoT Chainによる構造的防止 + Phase 4 の Drift Detection** |
| **テスト生成手法** | 3 手法 | **4 手法** (カタログ駆動テストを追加) |
| **探索的精緻化ループ** | 3 フェーズ | **3 フェーズ** (A plan / B design / C implement、Phase B = `/track:design` を独立、スケルトン生成は削除して将来のロードマップ項目に) |

## 付録 B: v5 からの継承項目

v5 draft の以下の要素は v6 でそのまま継承する:

- **sotp スタンドアロン化** (SPLIT-01〜05, Phase 1.5 + 4 + 6)
- **Phase 2c** (domain-types.json 分離の動機 + 12 variants enum-first taxonomy)
- **Behaviour Harness** の位置付け (Fowler Taxonomy)
- **Drift Detection** (Phase 4 の DRIFT-01/02: arch-drift 定期スキャン + 依存 staleness)
- **Reviewer Calibration** (Phase 5 の CALIB-01: severity criteria + few-shot examples)
- **GitHub Native Harness** (Phase 5 の GH-01/02: Issue intake + label projection + Scorecard)
- **Harness Template 展開** (Phase 6 の TMPL-01: トポロジー別)
- **Anthropic Three-Agent 対応** (planner / implementer / reviewer 分離)

**v6 は v5 の拡張であり、否定ではない**。v5 の全要素は有効であり、v6 で TDDD multilayer + SoT Chain + ハーネス TDDD + SoT 原点回帰を追加するのみ。

## 付録 C: 投資比率 (v5 → v6)

```
v1:  guardrails 70% + テスト支援 10% + 仕様品質 20%
v2:  guardrails 20% + テスト支援 40% + 仕様品質 40%
v3:  ハーネス保守 20% + テスト生成ツール 40% + テンプレート品質 40%
v5:  ハーネス保守 + sotp 分離 25% + テスト生成 + Fowler 対応 40% + テンプレート品質 35%
v6:  ハーネス TDDD リファクタ 30% + SoT Chain + TDDD multilayer 35% + テスト生成 20% + テンプレート品質 15%
```

v6 では「ハーネス TDDD リファクタリング」に重点が移るが、これは **SoT Chainを機能させるための投資**である。テンプレート品質は相対的に減るが、ハーネスの TDDD 化の成果物がそのままテンプレートの参照実装になるため、実質的な投資総量は維持される。

## 付録 D: TDDD 実装状態 (2026-04-13 時点)

| Step | ADR | track | 状態 |
|---|---|---|---|
| **Phase 2c 起点** | `2026-04-07-0045-domain-types-separation.md` | `spec-domain-types-v2-2026-04-07` | 完了 |
| **reverse signal 導入** | `2026-04-08-1800-reverse-signal-integration.md` | (先行 track) | 完了 |
| **TDDD-02: baseline + 4 グループ評価** | `2026-04-11-0001-baseline-reverse-signals.md` | (先行 track) | Accepted |
| **TDDD-03: action 宣言** | `2026-04-11-0003-type-action-declarations.md` | (未着手) | Proposed |
| **TDDD-01: multilayer + シグネチャ検証 + リネーム** | `2026-04-11-0002-tddd-multilayer-extension.md` | `track/tddd-01-multilayer-2026-04-12` | in_progress (Phase 1, T001-T003 完了, T004-T007 進行中) |
| **tddd-02: taxonomy 拡張 (5 → 12 variants) + usecase 層取り込み** | `2026-04-13-1813-tddd-taxonomy-expansion.md` | `tddd-02-usecase-wiring-2026-04-14` | **Accepted (2026-04-14 実装済み)** |
| **TDDD-04: spec_source 必須化 (SoT Chain)** | — (v6 で新規 ADR 作成) | 未計画 | **v6 で新規追加** |
| **命名確定** | — | 未計画 | 仮称「TDDD」のまま |
| **ハーネス TDDD リファクタリング** | — (v6 で方針明記) | 各実装 track で同時進行 | **v6 で新規方針** |

## 付録 E: 参考文献

- `knowledge/strategy/tddd-implementation-plan.md` — TDDD 3 ステップの実装計画
- `knowledge/strategy/TODO-PLAN-v5-draft.md` — v5 draft (Fowler / sotp 分離 / 2 信号機、v6 採用後も併存継続)
- `knowledge/research/2026-04-07-1234-harness-engineering-landscape.md` — Fowler Taxonomy + Anthropic Three-Agent 調査
- `knowledge/research/2026-04-05-harness-engineering-startup-analysis.md` — 外部観測面の課題分析
- `knowledge/research/2026-04-05-github-native-harness-design.md` — GitHub Native Harness 設計メモ
- `knowledge/adr/2026-04-07-0045-domain-types-separation.md` — 2 信号機 + 型カテゴリ
- `knowledge/adr/2026-04-08-1800-reverse-signal-integration.md` — reverse signal 導入
- `knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md` — baseline 4 グループ評価
- `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` — multilayer + シグネチャ検証
- `knowledge/adr/2026-04-11-0003-type-action-declarations.md` — action 宣言
- `knowledge/adr/2026-03-23-2120-two-stage-signal-architecture.md` — 2 段階ゲート
- `knowledge/adr/2026-03-23-1010-three-level-signals.md` — Blue/Yellow/Red 3 値
- `knowledge/adr/2026-03-24-0900-coverage-not-a-signal.md` — coverage は CI ゲート
- `.claude/rules/04-coding-principles.md` — typestate / enum-first の判断基準
- `tmp/vision-v4-2026-04-13.md` — v4 作業 draft (v6 の予備考察、リファレンスとして保存)
- `tmp/README-v4-2026-04-13.md` — README v4 draft (同上)
- `tmp/archive-2026-04-13/vision-v3.md` — v3 正式版 (参照のみ)

---

## 付録 F: 業界比較 — SDD 既存ツールとの比較分析 (2026-04 時点)

v6 草案作成時 (2026-04-13) に実施した WEB 調査に基づき、既存のハーネス / SDD ツールと SoT Chain の比較をまとめる。詳細な調査ソースは §F.6 を参照。

### F.1 SDD 主要 4 ツール比較表 (Spec Kit / Kiro / Tessl / tsumiki)

| 機能 | GitHub Spec Kit | Amazon Kiro | Tessl | tsumiki | **SoTOHE v6** |
|---|---|---|---|---|---|
| **drift 防止機構** | `/speckit.analyze` + Spec Sync extension + Reconcile Extension + CI Guard extension | executable specs + steering files + file-save hooks + property-based testing | **spec-as-source** + `// GENERATED FROM SPEC - DO NOT EDIT` + 1:1 sync + `[@generate]` | ファイルパス / 命名規約の強制 | **SoT Chain** (4 層 SoT + 一方向参照 + CI Red ブロック) |
| **型レベル契約** | ✗ (contracts/api-spec.json は API レベルのみ) | ✗ (明示的言及なし) | ✗ | ✗ | **✓ TDDD シグネチャ検証** (引数型・戻り型・receiver・async の完全マッチ) |
| **primitive obsession 検出** | ✗ | ✗ | ✗ | ✗ | **✓ 機械的検出** (`id: UserId` vs `id: i64` → Yellow) |
| **信号機 (🔵🟡🔴)** | ✗ (checklist はあるが Fowler 評: 「agent が従わないことが多い」) | ✗ | ✗ | ✗ | **✓ Stage 1 + Stage 2 の 2 信号機** |
| **多層カタログ** | ✗ | ✗ | ✗ (1 spec = 1 code の **1:1 mapping**) | ✗ | **✓ `architecture-rules.json` の `layers[].tddd`** で任意層対応 |
| **baseline 増分評価** | ✗ | ✗ | ✗ | ✗ | **✓ 4 グループ評価** (A\B, A∩B, B\A, ∁(A∪B)∩C) で既存型ノイズ排除 |
| **action 宣言** (add/modify/reference/delete) | ✗ | ✗ | ✗ | ✗ | **✓ TDDD-03** で意図的削除と事故的削除を区別 |
| **SoT 層数** | 1 層 (spec) | 1 層 (spec) | **2 層** (spec ↔ code) | 1 層 (spec) | **4 層** (ADR / spec / 型カタログ / 実装) |
| **ハーネス自身への自己参照適用** | ✗ (ツール側で不要) | ✗ | ✗ | ✗ | **✓ v6 新方針** (ハーネスが自らを TDDD で検証) |
| **CI ゲート (merge ブロック)** | ✓ (CI Guard extension) | ハック的 (hooks) | 不明 | ✗ | ✓ (`verify spec-states` + multilayer AND 集約) |
| **コードモデル** | hand-written | hand-written | **Generated** (人間は編集しない) | hand-written | **hand-written + 契約検証** |

**Fowler の SDD 3 tools 比較記事 (Birgitta Böckeler, 2026)** における評価:

> Tessl は 3 ツールの中で唯一、explicit な drift 防止機構を持つ。Kiro と Spec Kit は comparable な drift 防止機構を欠く。型契約・型レベル検証について、3 ツールいずれも対応していない。

SoTOHE v6 は **4 ツールいずれも対応していない型レベル契約 + 信号機 + multilayer + baseline + action** を統合して提供する。

### F.2 Fowler Taxonomy (2026-04 更新) の業界ギャップと SoT Chain の対応

Fowler 記事 (`martinfowler.com/articles/harness-engineering.html`) は harness engineering の業界ギャップとして以下の 4 点を指摘している:

| # | Fowler が指摘するギャップ | SoTOHE v6 の対応 |
|---|---|---|
| 1 | **"Behaviour harness underdeveloped"** — 現状は AI 生成テストに依存、品質不十分 | **SoT Chain** が Behaviour Harness の具体実装。Fowler は概念を定義したが実装形態を未提示。SoTOHE v6 は「4 層 SoT + 一方向参照 + TDDD + 2 信号機」という具体形を提示 |
| 2 | **"No harness coherence tooling"** — guides と sensors が分散、同期メカニズムなし | **`architecture-rules.json`** を guides + sensors の単一 SSoT にする。Phase 1.5/2/2c/2d/3/4/5/6 の依存関係を戦略サマリーで統合管理 |
| 3 | **"Test quality insufficiency"** — mutation testing, coverage gaps が未解決 | **Phase 3-14 カタログ駆動テスト** — TDDD カタログから proptest / 契約テスト / exhaustive match テストを自動生成し、「宣言 = テスト」の哲学で品質保証 |
| 4 | **"Missing evaluation frameworks"** — harness 自体の有効性を測る指標なし | **Stage 1/2 信号機** (spec 品質 + 型契約品質の定量化) + **Harness Scorecard** (v5 継承: workflow success rate, review rounds per track, human rescue rate) |

Fowler は業界全体を俯瞰する記述だが、SoTOHE v6 はこの 4 ギャップ全てに具体的な実装で応える唯一のハーネスになる (調査範囲内)。

### F.3 Tessl との哲学的差異

Tessl は Fowler の SDD 3 ツール比較で **唯一の explicit drift 防止機構** を持つと評価されており、概念的に SoTOHE に最も近い。ただし根本的な哲学が異なる:

| 観点 | Tessl | SoTOHE v6 |
|---|---|---|
| **コアメタファ** | spec-as-source (spec が唯一の正本) | 4 層独立 SoT (各層が独立したライフサイクル) |
| **コードの扱い** | **Generated** (`// GENERATED FROM SPEC - DO NOT EDIT`、人間は編集しない) | **Hand-written** (契約検証のみ、人間 + AI が書く) |
| **mapping** | **1:1** (1 spec file = 1 code file) | **多対多** (1 spec section → 複数の型カタログエントリ → 複数の実装ファイル) |
| **spec の粒度** | API + capabilities + linked tests の 3 部構成 | 要件 + Domain States + Given/When/Then + source tags |
| **型レベル契約** | 無し (capabilities で API を定義するが型は検証しない) | **TDDD シグネチャ検証** (引数型・戻り型・receiver・async まで完全マッチ) |
| **多層対応** | 無し (1:1 mapping が限界) | **architecture-rules.json** で任意層 (domain/usecase/infra/cli) |
| **既存コードベースへの適用** | Generated 前提で既存コードの移行コストが高い | **Hand-written ベース** + baseline 4 グループ評価で per-track 増分適用 |
| **人間と AI の境界** | AI が code を生成、人間は spec のみ編集 | 両方を人間 + AI が協働、契約 (型カタログ) で整合を保証 |

**根本的な違い**:

- **Tessl**: spec を source にすれば drift は発生しない (**generation 型**の解)
- **SoTOHE v6**: hand-written code でも drift を構造的に防ぐ (**verification 型**の解)

generation 型の解は理論的に美しいが、現実の既存コードベース (成熟した大規模プロジェクト) には適用しにくい。SoTOHE v6 の verification 型は **既存コードベースへの段階的適用** (baseline per-track + action 宣言 + ハーネス自身の段階リファクタリング) が可能であり、実用性で優る。

### F.4 arxiv 2602.00180 "Spec-Driven Development: From Code to Contract" との関係

論文 (`arxiv.org/abs/2602.00180`) は SDD を 3 レベルで分類する:

| レベル | 定義 | drift 対応 |
|---|---|---|
| **Spec-First** | コード前に仕様を書くが、後の保守は任意 | drift を放置 |
| **Spec-Anchored** | 仕様とコードを並行保守 | 手動 drift 管理 |
| **Spec-as-Source** (Tessl モデル) | 仕様のみが編集対象、コードは派生物 | drift を排除するが制約大 |

**SoTOHE v6 はこの 3 レベルのどこに位置するか?**

SoTOHE v6 は **第 4 のレベル "Spec-Chained"** と呼べる位置にある:

- 仕様もコードも独立 SoT (各層の独立ライフサイクル)
- 参照の強制で drift を構造的に防止 (SoT Chain)
- 型レベル契約で設計意図を機械的に検証 (TDDD シグネチャ検証)
- baseline + action で既存コードベースにも適用可能

Spec-Anchored の「並行保守」と Spec-as-Source の「generation」の中間として、**「参照チェーンによる整合保証」** を提供する。論文はこの第 4 レベルに言及していない。

論文自体の重要な認め:

> "AI models are excellent at pattern completion but poor at mind reading"
> "By providing AI with unambiguous, executable contracts, SDD enhances the reliability of coding agents."
>
> — arxiv 2602.00180, Section I-A

論文は **型レベル検証には触れていない** (formal verification は embedded / safety-critical 文脈でのみ言及)。SoTOHE v6 の TDDD シグネチャ検証は、**AI 汎用開発における型レベル検証のパイオニア実装**となる可能性がある。

### F.5 SoTOHE v6 が独占する特徴 (2026-04 時点の調査範囲)

以下の 7 点は、調査した既存ハーネス / SDD ツールのいずれにも存在しない特徴である:

1. **4 層 SoT の独立ライフサイクル** — 既存ツールは最大 2 層 (Tessl の spec↔code)
2. **型レベル契約 (TDDD シグネチャ検証)** — SDD 論文も認める未開領域
3. **信号機 (🔵🟡🔴) による段階的品質** — 業界で同等機構なし
4. **multilayer 型カタログ** — Tessl の 1:1 mapping を構造的に超える
5. **baseline 4 グループ評価** — per-track 増分評価の具体実装は業界初
6. **action 宣言 (add/modify/reference/delete)** — 意図の明示化は既存ツールになし
7. **ハーネス自身への自己参照的 TDDD 適用** — v6 新方針、業界で唯一

この 7 点が SoTOHE v6 の **Moat (競争優位の中核)** = **SoT Chain** を構成する差別化要素である。

### F.6 調査ソース (2026-04-13 時点)

**ツール公式 / 記事**:

- GitHub Spec Kit: `https://github.com/github/spec-kit`
- Amazon Kiro: `https://kiro.dev/`
- Tessl: `https://tessl.io/blog/tessl-launches-spec-driven-framework-and-registry/`
- tsumiki (Zenn): `https://zenn.dev/hidechannu/articles/20260314-spec-driven-development-tsumiki`

**業界分析**:

- Martin Fowler "Harness engineering for coding agent users": `https://martinfowler.com/articles/harness-engineering.html`
- Birgitta Böckeler "Understanding Spec-Driven Development: Kiro, spec-kit, and Tessl": `https://martinfowler.com/articles/exploring-gen-ai/sdd-3-tools.html`
- InfoQ "Spec Driven Development: When Architecture Becomes Executable": `https://www.infoq.com/articles/spec-driven-development/`
- Red Hat Developer "Harness engineering: Structured workflows for AI-assisted development": `https://developers.redhat.com/articles/2026/04/07/harness-engineering-structured-workflows-ai-assisted-development`
- SSRN "Harness Engineering: A Governance Framework for AI-Driven Software Engineering" (Kim & Hwang): `https://papers.ssrn.com/sol3/papers.cfm?abstract_id=6372119`

**学術論文**:

- arxiv 2602.00180 "Spec-Driven Development: From Code to Contract in the Age of AI Coding Assistants": `https://arxiv.org/abs/2602.00180` — 3 レベルの SDD 分類、型レベル検証ギャップを認める記述

**関連エコシステム**:

- awesome-harness-engineering (ai-boost): `https://github.com/ai-boost/awesome-harness-engineering`
- Latent Space podcast "Extreme Harness Engineering for Token Billionaires" (OpenAI Codex team): `https://www.latent.space/p/harness-eng`

**内部記録**:

- `knowledge/research/2026-04-07-1234-harness-engineering-landscape.md` (v5 時点の Fowler / Anthropic 調査)
- `knowledge/research/2026-04-05-harness-engineering-startup-analysis.md`
- `knowledge/research/2026-04-05-github-native-harness-design.md`

---

## レビュー観点 (ユーザー確認用 — 歴史的記録)

> **注**: 本節は v6 草案作成時点 (2026-04-13) のレビューチェックリストである。v6 昇格は 2026-04-15 に完了しており、以下は昇格前レビューの記録として保持する。

本 v6 draft を昇格する前に、以下の観点で確認した:

1. **§0 原点回帰**: 初版 README の概念を継承しつつ "Source of Truth Oriented" を一次コンセプトに据える方針が意図通りか
2. **§1 SoT Chain**: 4 層 SoT の参照チェーンとドリフト防止の説明粒度が妥当か
3. **§2 4 層 SoT**: ADR / spec / 型カタログ / 実装の 4 層分類と独立ライフサイクルが正しく切れているか
4. **§3.1 Stage 2 で Yellow 復活**: v5 の「Yellow 廃止」方針を反転することに問題がないか (TDDD-02 の 4 グループ評価で Yellow が必要)
5. **§3.5 spec_source 必須化**: TDDD-04 として新規 ADR を書き下す方針が妥当か
6. **§4 Fowler Taxonomy**: v5 の対応表に「SoT Chain」「TDDD + typestate」を追記する粒度が正しいか
7. **§5 3 フェーズ**: Phase B (TDDD カタログ編集) を `/track:design` として明示し、Phase C (スケルトン生成) を削除した判断が妥当か
8. **§6 ハーネス TDDD 反転**: v3/v5 の方針反転の理由付けが十分か
9. **§7 sotp スタンドアロン化**: v5 の方針をそのまま継承する方針で齟齬がないか
10. **TDDD 命名**: 仮称のまま記述し、後続 track で確定する方針で良いか (§3.6 で「SoT-Chain Driven Development」を Moat 名称との重複を理由に候補から除外した判断を含む)
11. **TODO-PLAN v6 との整合**: 本 vision v6 と同時に作成する `tmp/TODO-PLAN-v6-draft.md` との整合が取れているか
12. **README v6 との整合**: 本 vision v6 のエッセンスが `tmp/README-v6-draft.md` に適切に反映されているか

合意後、以下を実施する想定:

- `knowledge/strategy/vision.md` を本 draft で置き換え (v5 は採用されなかったため、現行 v3 → 本 v6 へ直接昇格)
- `knowledge/strategy/TODO-PLAN.md` を `tmp/TODO-PLAN-v6-draft.md` で置き換え
- `README.md` を `tmp/README-v6-draft.md` で置き換え
- v5 draft (`knowledge/strategy/TODO-PLAN-v5-draft.md`) は `knowledge/strategy/` に併存継続 (検討履歴として保持)
- v4 作業 draft (`tmp/vision-v4-2026-04-13.md`, `tmp/README-v4-2026-04-13.md`) は `tmp/` に保存 (ユーザー指示 3(b))
- TDDD-04 (spec_source 必須化) の ADR を書き下す (別 track で実施)
- TDDD 命名確定 track の計画 (別 track で実施)
