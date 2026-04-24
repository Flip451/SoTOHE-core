# 型カタログ → 仕様書 signal 評価の有効化 (SoT Chain ②)

## Context

### §1 SoT Chain ② の現状 — schema 済み / signal 評価未稼働

ADR `2026-04-19-1242-plan-artifact-workflow-restructure.md` §D1.3 + §D2.2 により、型カタログ
(`<layer>-types.json`) の各エントリには `spec_refs: Vec<SpecRef>` および
`informal_grounds: Vec<InformalGroundRef>` field が追加済みで、`track-plan-decomposition-2026-04-22`
で codec 実装も完了している。

しかし同 ADR §D3.2 は「型契約 → 仕様書 signal」を暫定期 (schema 存在のみ、semantic 面は advisory) /
signal 実装後 (後続 ADR の範囲) の 2 段階で扱うと明示し、**signal 評価の実装そのものは scope 外**と
している:

> catalogue-signal の実装は本 ADR 対象外。
> `knowledge/adr/2026-04-18-1400-tddd-ci-gate-and-signals-separation.md`
> または別 ADR が担う

現状、catalogue entry の `spec_refs[]` / `informal_grounds[]` は書かれていても CI ゲートで
Blue/Yellow/Red 判定されず、merge gate でブロックする仕組みが動いていない。vision v6 §0.1 が Moat と
定義した SoT Chain の **② 番目のリンクが事実上未稼働**である。

### §2 delegate 先 ADR では代替されていない

親 ADR が delegate 先候補として挙げた `2026-04-18-1400-tddd-ci-gate-and-signals-separation` が
扱うのは以下で、catalogue-spec signal 評価は含まれない:

- 宣言ファイル `<layer>-types.json` と評価結果ファイル `<layer>-type-signals.json` の物理分離 (§D1)
- pre-commit 時の自動再計算 (§D2)
- Red/Yellow/Blue の pre-commit 判定ポリシー (§D3)
- stale 検出 (§D5、fingerprint 比較)

これらは既存 Stage 2 (type → implementation、rustdoc 構造突合) の運用基盤であり、新たに追加すべき
「catalogue entry の grounding 品質評価」(spec_refs/informal_grounds) の評価 logic は未提供。
したがって親 ADR の delegate を受ける **別 ADR が必要** である。

### §3 既存 Stage 2 signal との責務差

| 比較軸 | 既存 Stage 2 (type → implementation) | 新 catalogue-spec signal (type → spec) |
|---|---|---|
| SoT Chain | ③ | ② |
| 入力 | `<layer>-types.json` × rustdoc JSON (TypeGraph) | `<layer>-types.json` の各 entry 単独 |
| 評価対象 | 宣言と実装の構造一致 (`check_type_signals` / `check_consistency`) | `spec_refs[]` 解決 + `informal_grounds[]` 空/非空 |
| 評価単位 | type name 単位 | entry (SpecElementId 粒度) |
| 現状 | 実装済み (ADR 2026-04-08-1800 他) | **未実装** |

2 種類の signal が同じカタログに共存することになる。出力ファイル (`<layer>-type-signals.json`) での
扱い、aggregate 方法、merge gate との接続方法は本 ADR で決定する必要がある。

### §4 Phase 1 spec signal との対称性と、上流成果物の構造化に起因する非対称性

Phase 1 spec signal は `signal-eval-drift-fix-2026-04-23` (PR #110) で informal-priority rule が
確定済み:

- `informal_grounds[]` 非空 → 🟡 Yellow (他の状態に優先)
- `informal_grounds[]` 空 + `adr_refs[]` 非空 → 🔵 Blue
- 両方空 → 🔴 Red
- `convention_refs[]` は signal 評価対象外

親 ADR §D3.2 は Phase 2 catalogue signal にも同型の判定軸 (informal-priority) を定めており
(`spec_refs[]` + `informal_grounds[]` のみ、catalogue には convention 参照 field が存在しない)、
実装としては `libs/domain/src/spec.rs::evaluate_requirement_signal` と対称な純粋関数を
`libs/domain/src/tddd/signals.rs` に配置できる余地がある。

ただし **上流成果物の構造化有無に起因する本質的な非対称性** が存在する:

| 比較軸 | Phase 1 (参照先: ADR、markdown) | Phase 2 (参照先: spec.json、JSON) |
|---|---|---|
| 参照先ファイル形式 | 非構造化 (markdown / 自由形式) | 構造化 (JSON) |
| anchor の resolution 境界 | heading slug / HTML marker 等 (親 ADR §Q15 未確定、loose string validation のみ) | `SpecElementId` (§D2.1 で Q13[a] 確定、JSON subtree が一意に特定可) |
| hash 対象の canonical 化 | **未定義** (section 境界が markdown semantic に依存、§Q15 で別 ADR 送り) | **定義可** (spec 要素の canonical serialization → SHA-256、§D2.1 / §D2.3 で本 ADR 範囲内) |
| 参照構造体 field | `AdrRef { file, anchor }` (**hash なし**) | `SpecRef { file, anchor: SpecElementId, hash: ContentHash }` (**hash required**) |
| 上流改訂に対する drift 検出 | 不可 (現時点の親 ADR 範囲) | 可能 (spec 要素が変更されると SpecRef.hash と不一致になる) |

Phase 1 は参照先が markdown で hash 検証自体が存在しないため、signal 評価は「anchor 存在 +
informal_grounds 空/非空」の 2 軸のみで完結する。一方 Phase 2 は `SpecRef.hash` が required schema で
あり、catalogue entry の `spec_refs[]` に `SpecRef.hash` field が存在するため、原理的には spec 要素の
変更に対する drift 検出が可能な状態にある (schema は整っている)。ただし catalogue 側の `SpecRef` を走査
して drift を検出する CLI は現時点では未実装 — 親 ADR §D2.3 の `sotp verify plan-artifact-refs` は
spec artifact 側 (plan 参照: `adr_refs / convention_refs`) を対象とする既存 CLI であり、catalogue 内の
`spec_refs[]` を検証する機能は持たない。

したがって Phase 2 catalogue signal 評価を設計する際には、**Phase 1 と同型の informal-priority rule に
加えて、この hash 検証結果をどう signal 色 (🔵🟡🔴) にマップするか** (Decision 対象) — stale /
dangling / 未解決をどう区別/統合するか、`SignalBasis` のような内部 nuance で記録するか、
verify-plan-artifact-refs との責務分担をどう切るか — という論点が Phase 1 には存在しなかった
追加設計課題として浮上する。この非対称性への対処は本 ADR の Decision セクションで扱う。

### §5 関連する未解決 TODO — CLI-04 との整合

`knowledge/strategy/TODO.md` 登録の **CLI-04 (HIGH)** が同じ TDDD 信号機 orchestration の usecase 層
引き上げを扱う未解決課題として存在する:

- `libs/infrastructure/src/verify/merge_gate_adapter.rs::read_type_catalogue` の 2 blob 整合 +
  aggregate hydrate
- `libs/infrastructure/src/track/render.rs::sync_rendered_views` の条件付き `set_signals` / hash 検証

本 ADR の新 signal 実装が CLI-04 の責務再編と衝突または重複しないよう、本 ADR で評価関数の配置層
(domain) を明確化し、呼び出し側 orchestration の再編は CLI-04 の track に委ねる分離が必要。

### §6 関連参照

**README 原典 (cross-layer SSoT)**:

- `README.md` §SoT Chain / §参照チェーンの評価 (🔵🟡🔴 の意味論、各参照方向の評価基準を規定する
  cross-layer SSoT)
- `README.md` §ロードマップ「型契約 → 仕様書の評価実装」(計画中項目として明示、**本 ADR が完成させる
  対象**)
- `README.md` §探索的精緻化ループ (🔴/🟡 → 🔵 への強制退行、本 ADR の signal が駆動する Phase 2
  ゲート)

**親 ADR / 兄弟 ADR**:

- `knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md` §D1.3 / §D2.1 / §D2.2 /
  §D2.3 / §D3.2 / §D6.2 (親 ADR、SoT Chain と delegate 指示 + SpecRef schema + hash 検証範囲の原典)
- `knowledge/adr/2026-04-18-1400-tddd-ci-gate-and-signals-separation.md` (Stage 2 の宣言/評価結果
  分離、`<layer>-type-signals.json` の schema と pre-commit 自動再計算の基盤)
- `knowledge/adr/2026-04-08-1800-reverse-signal-integration.md` (既存 Stage 2 type→implementation
  signal の設計、TDDD 単一ゲート原則)
- `knowledge/adr/2026-03-23-1010-three-level-signals.md` (🔵🟡🔴 3 値 + `SignalBasis` 内部 nuance の
  パターン)
- `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md` (strict/interim 分離、Yellow が
  merge ブロック、domain 層に純粋 signal 関数を配置するパターン)

**Phase 1 先行事例**:

- `track/items/signal-eval-drift-fix-2026-04-23/` + PR #110 (Phase 1 spec signal の informal-priority
  確定、本 ADR はこの対称実装を Phase 2 で行う)

**Vision / TODO**:

- `knowledge/strategy/vision.md` §0.1 §1.2 §3.5 §F.5 (SoT Chain Moat 定義、業界独占差別化の 7 特徴、
  本 ADR が ② 番目のリンクを活性化)
- `knowledge/strategy/TODO.md` CLI-04 (TDDD 信号機 orchestration を usecase 層へ引き上げる未解決
  課題、本 ADR 実装と整合必要)

## Decision

### D1: hash 検証は信号機とは独立の binary gate とする

catalogue-spec signal は **spec_refs / informal_grounds の「grounding 品質」** だけを判定し、
`SpecRef.hash` / `SpecRef.anchor` の検証結果は **信号機 (🔵🟡🔴) に取り込まない**。
hash / anchor 検証は OK / ERROR の二値 gate (親 ADR §D3.3 の task-coverage gate と同じ pattern) として
独立配置する。

#### D1.1: 信号機評価の規則 (Phase 1 完全対称、informal-priority)

| 条件 | catalogue signal |
|---|---|
| `informal_grounds[]` 非空 | 🟡 Yellow (informal-priority、他状態に優先) |
| `informal_grounds[]` 空 + `spec_refs[]` 非空 | 🔵 Blue |
| `informal_grounds[]` 空 + `spec_refs[]` 空 | 🔴 Red |

hash 一致 / anchor 解決の成否は信号色に影響しない (信号機はあくまで grounding の有無を示す)。

#### D1.2: binary gate は新規 CLI / 新規 gate として独立実装する

`sotp verify plan-artifact-refs` (親 ADR §D2.3) は流用せず、catalogue-spec 参照専用の **新規 CLI
サブコマンドと新規 CI gate** を新設する。

- **CLI 名**: `sotp verify catalogue-spec-refs` (参照整合性検証に特化した命名)
- **対象**: active track の全 `tddd.enabled` かつ `catalogue_spec_signal.enabled` 層の
  `<layer>-types.json` 内の全 catalogue entry の `spec_refs[]` を走査
- **検証項目**:
  - `SpecRef.anchor` (`SpecElementId`) が参照先 `spec.json` 内に存在するか (dangling 検出、全経路)
  - `SpecRef.hash` が参照先 spec 要素の canonical serialization SHA-256 と一致するか (drift 検出、全経路)
  - `<layer>-catalogue-spec-signals.json` の `catalogue_declaration_hash` が現
    `<layer>-types.json` の SHA-256 と一致するか (**CI / merge gate 経路のみ**、`--skip-stale` flag なしで
    呼ぶ場合。stale 検出、D2.2 の機構を mandatory 経路で確実に発火。pre-commit 経路は
    `--skip-stale` を付けて呼ぶため stale check をスキップ — 手順 3 で signals file を再計算する前の時点
    であり、stale 状態は正常)
- **exit code**: OK → 0 / ERROR → non-zero
- **CI 統合**: `cargo make ci` に `verify-catalogue-spec-refs` task として組み込む
- **merge gate 統合**: `check_strict_merge_gate` (ADR 2026-04-12-1200) のシーケンスに追加し、
  strict 時も interim 時も ERROR は fail-closed でブロック

**新規設計の理由**:

- `sotp verify plan-artifact-refs` は spec 側の ref field (adr_refs / convention_refs /
  related_conventions) を対象とする既存 CLI。対象 schema が異なるため bundle すると責務が混濁し、
  将来の catalogue schema 進化の変更時に影響範囲が広がる
- catalogue-spec drift は TDDD 信号機 (SoT Chain ②) の独立な関心事であり、別 gate として配置する方が
  pre-commit / CI / merge gate の各経路での抑制粒度を個別に調整できる

#### D1.3: commit / merge マトリクス (signal × hash gate の組合せ)

| 信号色 (grounding) | hash gate | commit (interim) | merge (strict) |
|---|---|---|---|
| 🔵 Blue | OK | ✅ | ✅ |
| 🔵 Blue | ERROR | ❌ | ❌ |
| 🟡 Yellow | OK | ✅ (warning) | ❌ (strict=true で block) |
| 🟡 Yellow | ERROR | ❌ | ❌ |
| 🔴 Red | OK | ❌ | ❌ |
| 🔴 Red | ERROR | ❌ | ❌ |

hash gate ERROR は全ての信号色でブロックされる (信号色は merge 時の Yellow 判断にのみ関与、
hash 不一致はどの段階でも fail-closed)。

#### D1.4: 設計上の帰結

- 信号機評価は Phase 1 `evaluate_requirement_signal` (spec.rs) と **完全対称な pure function** で
  実装可能 (domain 層、signature が同一パターン)。判定ロジックの複雑化を避ける
- hash drift は `sotp verify catalogue-spec-refs` (D1.2 で新設) が担い、同じ fail-closed 原則 (ERROR →
  non-zero exit) で統一処理。信号色内部の意味論を膨張させない
- `SignalBasis` のような内部 nuance 拡張は本 ADR では導入しない (3 値純粋、追加情報は hash gate の
  エラーメッセージで表現)

#### D1.5: エラー出力の責務と形式

binary gate が ERROR を返す際、**どの catalogue entry の、どの SpecRef が、どのように壊れているか** を
明示する責務を gate 実装側に課す。

##### 層ごとの責務

| 層 | 責務 |
|---|---|
| domain | 純粋関数 `check_catalogue_spec_ref_integrity(catalogue, spec, current_catalogue_hash, signals_opt) -> Vec<SpecRefFinding>` を提供。`current_catalogue_hash: Option<&ContentHash>` は usecase 層が `read_catalogue_for_spec_ref_check` の返値 `(TypeCatalogueDocument, String)` の String 部から渡す (stale 検出用。`None` / `signals_opt` が `None` の場合は stale check をスキップ)。違反がなければ空 Vec を返す。I/O を持たない |
| usecase | secondary port (`TrackBlobReader` / catalogue reader) 経由で入力を集め、domain の純粋関数を呼び出し、結果を aggregate |
| infrastructure / CLI | `SpecRefFinding` を human-readable に format し stderr に出力 + exit code を決定 |

##### `SpecRefFinding` データ構造

<!-- illustrative, non-canonical -->
```rust
pub enum SpecRefFindingKind {
    DanglingAnchor { anchor: SpecElementId },
    HashMismatch {
        anchor: SpecElementId,
        declared: ContentHash,
        actual: ContentHash,
    },
    /// signals file の catalogue_declaration_hash が現 catalogue の SHA-256 と不一致
    /// (pre-commit を bypass して catalogue を変更したが signals file を再生成していない)
    StaleSignals {
        declared_catalogue_hash: ContentHash,
        actual_catalogue_hash: ContentHash,
    },
}

pub struct SpecRefFinding {
    layer: LayerId,
    /// catalogue_entry が特定できない場合 (StaleSignals は layer 単位の finding) は None
    catalogue_entry: Option<String>,
    /// SpecRef index が特定できない場合 (StaleSignals) は None
    ref_index: Option<usize>,
    /// spec_file が特定できない場合 (StaleSignals) は None
    spec_file: Option<PathBuf>,
    kind: SpecRefFindingKind,
}
```

##### 出力形式 (CLI 層が format)

1 finding = 1 行、grep/awk 可能な構造を保つ:

```
[ERROR] catalogue-spec-refs: infrastructure/FsTrackStore spec_refs[0] -> DanglingAnchor anchor=IN-03
[ERROR] catalogue-spec-refs: domain/ReviewerFinding spec_refs[1] -> HashMismatch anchor=AC-02 declared=sha256:ab12... actual=sha256:cd34...
[ERROR] catalogue-spec-refs: domain (layer) -> StaleSignals declared=sha256:ef56... actual=sha256:ab12...
```

構造化出力 (`--json`) は本 ADR では要求しない。将来 CI 集計 / review briefing 経由で消費したい需要が
出たら別 track で追加する。

#### D1.6: binary gate の発火タイミング

gate は以下の **3 つの経路** で発火する。いずれの経路でも ERROR は fail-closed で処理を中断する。

| 発火タイミング | 経路 | 役割 | 参照先 |
|---|---|---|---|
| pre-commit | `/track:commit` → `dispatch_track_commit_message` (2026-04-18-1400 §D2 と同じ挿入点) | 開発者フィードバック即時性 | worktree の `<layer>-types.json` と `spec.json` |
| CI (interim) | `cargo make ci` 内の新 task `verify-catalogue-spec-refs` | pre-commit を bypass した commit / rebase で混入した commit の救済 | worktree |
| merge gate (strict) | `check_strict_merge_gate` (2026-04-12-1200) からの追加呼び出し | PR 最終判定の fail-closed | PR head ref の blob (`git show`) |

挿入位置詳細は D3.4 / D3.5 / D3.6 で定める。

#### D1.7: 親 ADR §D3.2 との関係

親 ADR §D3.2 の「spec_refs[]: 🔵 (全 SpecRef が anchor 解決 + hash 一致) / 🔴 (1 件でも失敗)」は
hash / anchor 検証と信号色を bundle した記述だが、本 ADR はこれを **2 つの独立した評価** に分離する
(signal 側は「grounding 有無」のみ、hash/anchor 側は binary gate)。

本 ADR 採用後に adr-editor back-and-forth で親 ADR §D3.2 の該当行を修正する必要がある
(`signal-eval-drift-fix-2026-04-23` Phase 1 の §D3.1 max() 削除と同じ pattern)。

### D2: catalogue-spec signal は独立ファイル `<layer>-catalogue-spec-signals.json` に格納する

責務分離を優先し、新 signal (type→spec、SoT Chain ②) の評価結果は既存 `<layer>-type-signals.json`
(type→implementation、SoT Chain ③) とは別ファイルに分離する。`2026-04-18-1400` §D1 の「宣言と評価結果の
物理分離」哲学を同 SoT 層内でさらに徹底する。ただし **per-ref drift 検出の `SpecRef.hash` は親 ADR
§D2.1 に従い catalogue `<layer>-types.json` に保持する** (drift detection の運用モデルを明確に保つ
ため)。

#### D2.1: ファイル配置と命名

配置: `track/items/<id>/<layer>-catalogue-spec-signals.json`

既存の `<layer>-type-signals.json` と並置する命名パターン:

| ファイル | 生成コマンド | 内容 | 入力 | SoT Chain |
|---|---|---|---|---|
| `<layer>-types.json` (既存) | designer 手書き (`/track:type-design`) | 宣言 (SpecRef.hash 含む、per-ref drift declare) | — | — |
| `<layer>-type-signals.json` (既存) | `sotp track type-signals` | 評価結果: type → implementation | catalogue + rustdoc | ③ |
| **`<layer>-catalogue-spec-signals.json` (新規)** | `sotp track catalogue-spec-signals` (新規 CLI) | 評価結果: type → spec | catalogue のみ | ② |

3 ファイルの役割分担:

- **人が書くもの**: `<layer>-types.json` のみ (宣言、SpecRef.hash の宣言含む、review 対象)
- **機械が生成する評価結果**: 残り 2 ファイル (`review_operational` で code_hash 除外)

#### D2.2: schema v1

新規 schema として v1 から開始:

<!-- illustrative, non-canonical -->
```json
{
  "schema_version": 1,
  "catalogue_declaration_hash": "sha256:...",
  "signals": [
    { "type_name": "FsTrackStore", "signal": "blue" },
    { "type_name": "ReviewerFinding", "signal": "yellow" },
    { "type_name": "TrackId", "signal": "red" }
  ]
}
```

- `catalogue_declaration_hash`: 入力 `<layer>-types.json` の全体 SHA-256。signals 結果が現在の
  catalogue から再計算されているかの stale 検出 (2026-04-18-1400 §D5 の fingerprint-mismatch と
  同型)
- `spec_declaration_hash` は **設けない**: 親 ADR §D2.1 の `SpecRef.hash` (per-ref) で granular な
  spec drift を検出できるため、whole-file hash を別途保持すると false positive (参照していない
  spec 要素の変更でも ERROR) を生む redundant 機構になる
- `generated_at` は **設けない**: signals file を入力 catalogue の pure function に保ち、タイムスタンプ
  による無用な file 変更を排除する (clean pattern、入力不変なら複数回実行でバイト一致)
- `signals[]`: D1.1 の informal-priority rule を適用した entry 単位の評価結果

#### D2.3: signal の粒度

per-entry (type_name 単位) で signal を持つ:

<!-- illustrative, non-canonical -->
```rust
pub struct CatalogueSpecSignal {
    pub type_name: String,
    pub signal: ConfidenceSignal,
}
```

- D1.1 の informal-priority rule を適用した結果のみを保持
- hash / anchor 検証結果は含めない (D1 binary gate で別経路、signals file には格納しない)
- 1 entry の複数 `spec_refs[]` は集約して 1 signal にする
- `SignalBasis` のような内部 nuance も含めない (3 値純粋、D1.4)

#### D2.4: review scope 除外設定

`track/review-scope.json` の `review_operational` 配列に以下の glob パターンを追加:

```
track/items/<track-id>/*-catalogue-spec-signals.json
```

`<layer>` 個別の placeholder には展開非対応のため、既存 `*-type-signals.json` と同じ glob パターンで
全 tddd.enabled 層の評価結果ファイルを一括除外する (2026-04-18-1400 §D4 と同型)。`<track-id>`
placeholder は既存経路 (2026-04-18-1400 §OQ1) で展開される。

#### D2.5: rendered view への signal overlay

`<layer>-types.md` レンダラー (`libs/infrastructure/src/type_catalogue_render.rs`) は 2 種類の signal を
entry 単位で並置表示する:

- type → implementation signal (既存): 既存フォーマットを維持
- **catalogue → spec signal (新規)**: 同 entry のセクション内に追加行として表示

具体的なレイアウト (markdown 構造、列配置、色付け) は実装時の proposal で確定する (本 ADR の決定
範囲外)。Contract Map / Type Graph View への overlay (2026-04-17-1528 §D5) は別 ADR / 別 track
スコープ。

#### D2.6: 既存ファイルとの独立性

**`<layer>-types.json` の `SpecRef.hash` (親 ADR §D2.1)**:

- 本 ADR で変更なし (schema 踏襲)
- D1 binary gate が per-ref drift 検出に使用

**`<layer>-type-signals.json` (既存 Stage 2)**:

- 本 ADR で無改変
- schema_version bump 不要 (v1 のまま)
- field 追加なし
- 既存 CLI (`sotp track type-signals`) の挙動変更なし
- 既存 review scope 除外設定 (2026-04-18-1400 §D4) 変更なし

本 ADR の変更は新ファイル `<layer>-catalogue-spec-signals.json` および新 CLI の追加のみに限定され、
既存 Stage 2 (type→implementation) 経路には副作用を与えない。

### D3: CLI 仕様と pre-commit / CI / merge gate 統合の精密化

本 ADR は 2 つの新規 CLI を導入する。D1 (binary gate 分離) / D2 (独立ファイル格納) / D1.6 (発火
タイミング) の決定を踏まえ、以下で仕様を精密化する。

#### D3.1: `sotp track catalogue-spec-signals` CLI (signal 再計算)

catalogue-spec signal の評価・書き出し CLI。既存 `sotp track type-signals` と対称な命名 / 引数。

##### 仕様

```
sotp track catalogue-spec-signals [--layer <layer_id>]
```

- 対象: active track の `architecture-rules.json` 内 `tddd.enabled == true` かつ
  `catalogue_spec_signal.enabled == true` の全層 (`--layer` 省略時)
- 入力: `track/items/<track-id>/<layer>-types.json`
- 出力: `track/items/<track-id>/<layer>-catalogue-spec-signals.json` を atomic write
  - D1.1 informal-priority rule で各 entry の signal を算出
  - `catalogue_declaration_hash` に入力 `<layer>-types.json` の全体 SHA-256 を記録
  - `generated_at` field は設けない (signals file を入力 catalogue の pure function とし、
    タイムスタンプによる無用な file 変更を排除)
- exit code: 成功 → 0 / schema 違反・decode 失敗等 → non-zero
- active-track guard: 非 active track で呼び出された場合は **reject (exit non-zero + エラー
  メッセージ)**。fail-closed 原則に従い、silent skip (exit 0) は採用しない (2026-04-15-1012 §D1 の
  既存 `sotp track type-signals` 挙動と同一)
- 冪等: 入力不変なら複数回実行でバイト一致 (deterministic output)

##### 本 CLI は gate ではない

signal 再計算は「宣言入力から評価結果を persist する writer」であり、pass/fail 判定は持たない
(signal 値 🔵🟡🔴 の算出のみ)。gate 発火は D3.2 の `sotp verify catalogue-spec-refs` が担う。

#### D3.2: `sotp verify catalogue-spec-refs` CLI (hash binary gate)

D1.2 で決定した binary gate CLI の詳細仕様。

##### 仕様

```
sotp verify catalogue-spec-refs [--track <id>] [--skip-stale]
```

- 対象: active track の全 `tddd.enabled` かつ `catalogue_spec_signal.enabled` 層の
  `<layer>-types.json` 内の全 catalogue entry の `spec_refs[]` を走査
- 入力: `<layer>-types.json` (SpecRef.hash / SpecRef.anchor 源) + `spec.json` (現状の要素と subtree
  hash); `--skip-stale` なし (CI / merge gate 経路) では `<layer>-catalogue-spec-signals.json` も追加入力
  (stale 検出用)
- **`--skip-stale` flag**: stale 検出 (検証項目 3) をスキップする。pre-commit 経路で使用
  (手順 2 で呼ぶ時点はまだ signals file が再計算前のため stale が正常。手順 3 で signals file を
  再計算した後に commit が確定する)。CI / merge gate 経路はこのフラグを付けず全 3 検証を実施する
- 検証: 以下の項目を実施 (1 件でも違反 → non-zero exit)
  1. **SpecRef.anchor 解決** (全経路): `SpecRef.anchor` (`SpecElementId`) が参照先 `spec.json` 内に
     存在するか (dangling 検出)
  2. **SpecRef.hash 一致** (全経路): `SpecRef.hash` が参照先 spec 要素の canonical serialization
     SHA-256 と一致するか (drift 検出)
  3. **stale 検出** (`--skip-stale` なし / CI・merge gate 経路のみ): `<layer>-catalogue-spec-signals.json`
     の `catalogue_declaration_hash` が現 `<layer>-types.json` の SHA-256 と一致するか — signals file が
     最新 catalogue から再計算されていない場合に検出 (D2.2 の stale detection 機構を mandatory 経路で
     確実に発火させる)
- 出力: 違反あれば stderr に `SpecRefFinding` 形式で 1 行ずつ列挙 (D1.5)
- exit code: 違反ゼロ → 0 / 1 件でも違反 → non-zero (fail-closed)
- active-track guard: 非 active track で呼び出された場合は **reject (exit non-zero + エラー
  メッセージ)**、silent skip は採用しない
- plan/* branch: gate 発火 path (pre-commit / merge gate) のみ skip、CLI 手動実行は非 skip

#### D3.3: 経路別発火モデル (binary gate / signal 再計算 / signal 検証 の役割分担)

3 経路での発火モデルは以下の通り (経路によって「再計算」と「検証」の役割が異なる):

| 経路 | binary gate | signal 再計算 | signal 検証 |
|---|---|---|---|
| pre-commit | ✅ `sotp verify catalogue-spec-refs --skip-stale` (dangling + drift のみ、stale check は `--skip-stale` でスキップ) → ERROR で block | ✅ `sotp track catalogue-spec-signals` (再計算して persist) | ✅ `cargo make ci` (D3.4 手順 4) 経由で `check_catalogue_spec_signals(strict=false)` が実行される (🔴 Red → block、🟡 Yellow → warning) |
| CI | ✅ `sotp verify catalogue-spec-refs` (dangling + drift + stale 検出) → ERROR で block | ❌ (pre-commit 経路で担保) | ✅ `check_catalogue_spec_signals(strict=false)` (🔴 Red → block、🟡 Yellow → warning) |
| merge gate | ✅ `sotp verify catalogue-spec-refs` (dangling + drift + stale 検出、blob 経由) → ERROR で block | ❌ (PR head の commit 済み file を使用) | ✅ `check_catalogue_spec_signals(strict=true)` (🟡 Yellow / 🔴 Red → block) |

**binary gate が先行する根拠 — signal の意味 semantic が先行条件に依存**:

catalogue-spec signal (🔵🟡🔴) は「catalogue entry の grounding 品質」を表現する。この signal 値が
意味を持つ前提条件は **spec_refs[] の参照先が現実に整合している** こと (anchor 解決 + hash 一致)。

- 参照先に drift がある場合、catalogue entry の grounding は意味を失う (「宣言では AC-01 に依拠して
  いると言っているが、AC-01 の内容は書き換わっている」状態)
- この状況で signal を 🔵 Blue と評価して persist しても誤誘導 (実際には re-verification が必要なのに
  「問題なし」と表示する)
- binary gate で drift ゼロが確認できて初めて signal 評価に意味が生じるため、gate を先に走らせる

pre-commit 経路では binary gate が ERROR なら後段の signal 再計算はスキップ (commit は既に block
されており、drifted catalogue から signals file を更新する意味がない)。

#### D3.4: pre-commit 挿入位置 (`dispatch_track_commit_message`)

挿入順序 (D3.3 反映、binary gate → signal 再計算):

1. (既存) Stage 2 type-signals 自動再計算 (2026-04-18-1400 §D2): `sotp track type-signals` 相当
2. **(新規) catalogue-spec refs binary gate**: `sotp verify catalogue-spec-refs --skip-stale` — stale
   check をスキップ (手順 3 の signals 再計算前のため)。ERROR なら commit block
3. **(新規) catalogue-spec signal 再計算**: `sotp track catalogue-spec-signals`
4. (既存) `cargo make ci`
5. (既存) Review guard (`sotp review check-approved`)
6. (既存) Commit from file
7. (既存) `.commit_hash` 永続化

**挿入順序の根拠 — SoT Chain bottom-up 検証**:

SoT Chain は「実装 → 型カタログ → 仕様書 → ADR」の一方向参照で、下流 (実装 / 型カタログ) は上流
(仕様書 / ADR) の決定に依拠する。drift を正す作業は **参照の根本 (下流) から始めて上流へ順次遡る** の
が自然で、本 pre-commit 順序はこれを反映する:

- 手順 1 (既存): type-signals = Chain ③ の「実装 → 型カタログ」整合性検証 (最下流、実装が catalogue に
  追随)
- 手順 2-3 (新規): catalogue-spec refs + signals = Chain ② の「型カタログ → 仕様書」整合性検証
  (ひと上流、catalogue が spec に追随)

この根本 → 上流の順で fix していくことで、(a) 下流の問題が先に露見し、(b) 上流検証時には下流整合が
前提として成立する。gate 失敗時は後続経路に進まず即 abort するため、CI 時間も浪費しない。

#### D3.5: CI (`cargo make ci`) 統合

CI に追加する 2 つの task:

1. **`verify-catalogue-spec-refs`** — 既存 `verify-plan-artifact-refs` / `verify-track-metadata` と
   並列実行可能。D3.2 仕様に従い dangling / drift / **stale 検出** の全項目を実施 (stale 検出は
   CI / merge gate 経路のみ有効、D3.2 検証項目 3)。pre-commit bypass で signals file が更新されなか
   った場合、CI の stale 検出で必ず catch する (fail-closed)

2. **`check-catalogue-spec-signals`** — `verify-catalogue-spec-refs` 成功後に実行 (または並列でも可、
   stale の場合は signals file が信頼できないため gate 1 が先に ERROR を返す)。
   `check_catalogue_spec_signals(strict=false)` を呼び出し、D4.1 の interim 挙動を適用:
   - 🔴 Red → `Finding::error` (BLOCKED)
   - 🟡 Yellow → `Finding::warning` (PASS + ログ出力)

- `sotp track catalogue-spec-signals` (signal 再計算) は CI task には組み込まない:
  - 再計算は pre-commit 経路で担保済み (D3.4 の手順 3)
  - stale (pre-commit bypass による未更新) は `verify-catalogue-spec-refs` の stale 検出で CI が検出する
- `ci-local` 内 task の依存関係: 既存 verify 群と同列、並列実行可能

#### D3.6: merge gate (`check_strict_merge_gate`) 統合 — Option B (SoT Chain bottom-up reorder)

D3.4 の SoT Chain bottom-up 検証方針と整合させるため、既存 merge gate の順序を reorder する:

1. (既存) branch validation / plan/* skip
2. (既存) `read_type_catalogue` + `check_type_signals` (Chain ③; 既存 API、rename なし)
3. **(新規) catalogue-spec refs binary gate** — `read_catalogue_for_spec_ref_check` (catalogue blob) +
   `read_spec_document` (spec blob、anchor/hash 比較のため) + `read_catalogue_spec_signals_document`
   (signals blob、stale 検出のため) + `check_catalogue_spec_ref_integrity` (Chain ②、dangling / drift /
   stale の全 3 検証を実施)
4. **(新規) catalogue-spec signal check** — `read_catalogue_spec_signals_document` (step 3 で取得済み
   の結果を再利用可) + `check_catalogue_spec_signals(strict=true)` (Chain ②、🟡/🔴 を BLOCKED に)
5. (既存) `read_spec_document` + `check_spec_doc_signals` (Chain ①、spec blob は step 3 で取得済みの
   結果を再利用可)

##### `TrackBlobReader` 拡張

secondary port に新メソッドを追加 (既存 `read_spec_document` / `read_type_catalogue` と同経路):

<!-- illustrative, non-canonical -->
```rust
pub trait TrackBlobReader {
    // (既存 省略)

    /// Step 3: binary gate 用 — catalogue entry の SpecRef.hash / anchor 検証および stale 検出のために使用
    /// Returns (decoded document, SHA-256 of raw file bytes) — same tuple pattern as read_type_catalogue
    fn read_catalogue_for_spec_ref_check(
        &self,
        branch: &str,
        track_id: &str,
        layer_id: &str,
    ) -> BlobFetchResult<(TypeCatalogueDocument, String)>;

    /// Step 4: signal check 用 — 事前に pre-commit / CI で生成済みの signals file を読む
    fn read_catalogue_spec_signals_document(
        &self,
        branch: &str,
        track_id: &str,
        layer_id: &str,
    ) -> BlobFetchResult<CatalogueSpecSignalsDocument>;
}
```

infrastructure: `GitShowTrackBlobReader` が `git show origin/<branch>:track/items/<id>/<layer>-types.json`
および `git show origin/<branch>:track/items/<id>/<layer>-catalogue-spec-signals.json`
経由で blob 取得 (既存 symlink guard / LANG=C fixation / BlobResult / path-not-found stderr 解析
すべて流用、`2026-04-12-1200` §D4 系の fail-closed 原則踏襲)。

##### merge gate 失敗条件

binary gate (step 3):

- strict mode / interim mode 問わず: 1 件でも以下のいずれかに該当 → `Finding::error` (BLOCKED)
  - drift (hash mismatch): `SpecRef.hash` が参照先 spec 要素の SHA-256 と不一致
  - dangling anchor: `SpecRef.anchor` が参照先 `spec.json` に存在しない
  - stale signals: `<layer>-catalogue-spec-signals.json` の `catalogue_declaration_hash` が現
    `<layer>-types.json` の SHA-256 と不一致 (CI / merge gate 経路で必須検出、D3.2 検証項目 3)

signal check (step 4):

- strict mode: signal が 🟡 Yellow または 🔴 Red → `Finding::error` (BLOCKED)
- interim mode: signal が 🔴 Red → `Finding::error` (BLOCKED) / 🟡 Yellow → `Finding::warning` (PASS)
  (D4.1 の strict/interim table と同一、merge gate は `strict=true` で呼ぶ)

##### 2026-04-12-1200 §D5 への amendment 必要性

既存 merge gate は Chain ① → Chain ③ の順。本 ADR は bottom-up (Chain ③ → ② → ①) に reorder する。
2026-04-12-1200 §D5 の該当 orchestration 記述を adr-editor back-and-forth で amendment する必要がある
(`signal-eval-drift-fix-2026-04-23` Phase 1 の §D3.1 max() 削除と同じ pattern)。

#### D3.7: 評価関数の層配置 (CLI-04 との整合)

TODO.md の **CLI-04 (HIGH)** は既存 TDDD 信号機 orchestration を infrastructure adapter から usecase 層へ
引き上げる未解決課題。本 ADR で新設する実装は **最初から正しい層に配置** することで CLI-04 の負債に
加担しない:

| 層 | 責務 | 新規追加する artifact |
|---|---|---|
| domain | signal 評価の純粋関数 / drift 検出の純粋関数 / `SpecRefFinding` 型 | `evaluate_catalogue_entry_signal` / `check_catalogue_spec_ref_integrity` / `SpecRefFinding` (新規 module: `libs/domain/src/tddd/catalogue_spec_signal.rs` 想定) |
| usecase | orchestration (port 経由 I/O + domain 純粋関数呼び出し) | `RefreshCatalogueSpecSignals` / `VerifyCatalogueSpecRefs` interactor、`TrackBlobReader::read_catalogue_for_spec_ref_check` / `read_catalogue_spec_signals_document` port 追加 |
| infrastructure | port 実装 (file I/O / git show 経由 blob 取得) | 既存 secondary adapter (`GitShowTrackBlobReader` 等) への method 追加、および catalogue-spec-signals ファイル用の `FsCatalogueSpecSignalsStore` |
| cli | CLI コマンドの薄い wrapper (引数 parse + usecase interactor dispatch) | `apps/cli/src/commands/track/catalogue_spec_signals.rs` / `apps/cli/src/commands/verify/catalogue_spec_refs.rs` |

CLI-04 の既存 orchestration 引き上げは本 ADR scope 外 (別 track で実施、D6 で境界明記)。

### D4: strict / interim モードの挙動

`2026-04-12-1200-strict-spec-signal-gate-v2` §D2 が確立した **strict / interim 分離** の pattern を本
ADR の 2 系統 (catalogue-spec signal / binary gate) に適用する。

背景 (既存 pattern):

| モード | caller | Yellow の扱い | Red の扱い |
|---|---|---|---|
| **interim** (strict=false) | CI (`cargo make ci`) | `Finding::warning` (PASS + ログ可視化) | `Finding::error` (BLOCKED) |
| **strict** (strict=true) | merge gate (`check_strict_merge_gate`) | `Finding::error` (BLOCKED) | `Finding::error` (BLOCKED) |

pre-commit 経路は既存 `/track:commit` フロー内で `cargo make ci` を呼ぶため interim 扱い。merge gate は
strict 扱い。本 ADR の新 gate もこの二元分類を踏襲する。

#### D4.1: catalogue-spec signal (🔵🟡🔴) の strict / interim 挙動

Phase 1 spec signal / Stage 2 type signal と完全対称に運用する:

| signal | strict (merge gate) | interim (CI / pre-commit) |
|---|---|---|
| 🔵 Blue | PASS | PASS |
| 🟡 Yellow | `Finding::error` (BLOCKED) | `Finding::warning` (PASS + ログ出力) |
| 🔴 Red | `Finding::error` (BLOCKED) | `Finding::error` (BLOCKED) |

実装: domain 層の純粋関数 (D3.7) として

<!-- illustrative, non-canonical -->
```rust
pub fn check_catalogue_spec_signals(
    signals: &CatalogueSpecSignalsDocument,
    strict: bool,
) -> VerifyOutcome;
```

を追加する。既存 `check_spec_doc_signals` / `check_type_signals` (2026-04-12-1200 §D2) と同型の
signature で strict bool を受け取り、`VerifyOutcome` を返す。

caller 側の strict 値は既存 pattern を踏襲:
- `check_strict_merge_gate` (usecase): `strict=true`
- `verify_from_spec_json` 相当の CI 経路: `strict=false`

#### D4.2: binary gate (hash / anchor drift) の strict / interim 挙動

binary gate は **strict / interim に関わらず常に `Finding::error`** で BLOCK する。

| drift 事象 | strict (merge gate) | interim (CI / pre-commit) |
|---|---|---|
| hash mismatch | `Finding::error` (BLOCKED) | `Finding::error` (BLOCKED) |
| dangling anchor | `Finding::error` (BLOCKED) | `Finding::error` (BLOCKED) |

根拠 (2026-04-18-1400 §D5 の stale detection 原則踏襲):

> stale は機械的に解消可能な状態であり性質が異なる。Yellow の warning ポリシーは stale には適用しない。

hash/anchor drift は `sotp track catalogue-spec-signals` を再実行しても解消しない (drift は author
行動が必要: catalogue の SpecRef.hash を現 spec subtree hash に合わせる or spec 側を戻す)。しかし
「再確認の必要が明示的」な状態であり、warning で通過させる合理性がない。

- pre-commit 経路: D3.4 の手順 2 で binary gate が ERROR を返せば commit ブロック、手順 3 以降に
  進まない
- CI 経路: `verify-catalogue-spec-refs` task が non-zero exit、`cargo make ci` 全体が失敗
- merge gate 経路: D3.6 の手順 3 で ERROR → BLOCKED

#### D4.3: signal / binary gate の独立性 (D1 再掲)

signal 評価 (D4.1) と binary gate (D4.2) は **独立した評価軸** であり、合成しない:

| signal (grounding 品質) | binary gate (drift 検出) | interim 総合判定 | strict 総合判定 |
|---|---|---|---|
| 🔵 | OK | PASS | PASS |
| 🔵 | ERROR | BLOCKED | BLOCKED |
| 🟡 | OK | PASS + warning | BLOCKED |
| 🟡 | ERROR | BLOCKED | BLOCKED |
| 🔴 | OK | BLOCKED | BLOCKED |
| 🔴 | ERROR | BLOCKED | BLOCKED |

interim 経路 (CI / pre-commit) で BLOCK になる条件:

- signal が 🔴 (grounding 不在)
- binary gate が ERROR (drift)

strict 経路 (merge gate) で BLOCK になる条件:

- signal が 🟡 or 🔴
- binary gate が ERROR

この合成ロジックは既存の `check_spec_doc_signals` / `check_type_signals` の結果と **AND 集約** で
最終 verdict を決める (2026-04-12-1200 §D5 の pattern 踏襲)。

#### D4.4: fail-closed 不変条件

signal / binary gate いずれも fail-closed を維持する:

- catalogue / spec 読み取り不能 (I/O / decode) → ERROR (2026-04-12-1200 §Context §Fail-closed 原則)
- symlink / submodule 検出 → ERROR (2026-04-12-1200 §D4.3 symlink guard 流用)
- catalogue / spec の schema 違反 → ERROR
- branch ref validation 失敗 → ERROR (`validate_branch_ref`)

全ての fail-closed 判定は 2026-04-12-1200 §D4 系の既存 guard を流用し、新規 guard は導入しない。

### D5: advisory → enforced への移行 + template 採用者向け opt-in flag

catalogue-spec signal の gate 発火は `architecture-rules.json` の `tddd` block に追加する
`catalogue_spec_signal.enabled` flag で **layer 単位に制御** する。この flag は **暫定 toggle ではなく
恒久配備**: SoTOHE-core をテンプレートとして採用する各プロジェクトが、layer 単位に catalogue-spec
signal を導入するかを選択できる設計自由度を残す。既存 `tddd.enabled` と同型の template 採用者向け
opt-in flag (`2026-04-11-0002-tddd-multilayer-extension` §D1 の pattern)。

#### D5.1: スコープ

| track 状態 | 挙動 |
|---|---|
| 非 active (branch なし / archive 済み) | 既存 active-track guard (2026-04-15-1012 §D1) で除外、本 ADR は触らない |
| active track | `layers[].tddd.catalogue_spec_signal.enabled` flag に従う |

`knowledge/conventions/no-backward-compat.md` の原則 (「非 active は write で protect、active は
新 rule 適用」) は維持される。

#### D5.2: architecture-rules.json schema 拡張

既存 `tddd` block に `catalogue_spec_signal` サブ block を追加 (optional):

<!-- illustrative, non-canonical -->
```json
{
  "version": 2,
  "layers": [
    {
      "crate": "domain",
      "path": "libs/domain",
      "tddd": {
        "enabled": true,
        "catalogue_file": "domain-types.json",
        "schema_export": { "method": "rustdoc", "targets": ["domain"] },
        "catalogue_spec_signal": { "enabled": true }
      }
    }
  ]
}
```

- 配置: `layers[].tddd.catalogue_spec_signal.enabled`
- 型: `bool`
- **default**: `false` (block 省略時 or `enabled` field 省略時、advisory 継続)
- **`tddd.enabled: false` 時**: `catalogue_spec_signal` block は無意味 (tddd 全体が無効のため skip)
- `enabled: false` 時の挙動:
  - signal 再計算 CLI / binary gate CLI はその layer を skip (他層は flag に従う)
  - pre-commit / CI / merge gate は該当 layer を skip
  - catalogue の `spec_refs[]` schema / codec は常に有効 (advisory 記録として書ける)
- `enabled: true` 時の挙動:
  - 全経路 (pre-commit D3.4 / CI D3.5 / merge gate D3.6) で gate 発火
  - D4 の strict / interim pattern 適用

`architecture-rules.json` schema version は **v2 のまま** (既存 `tddd` block に optional field 追加、
破壊的変更ではない — 2026-04-11-0002 §D1 と同じ pattern)。

#### D5.3: SoTOHE-core 自身での default

SoTOHE-core リポジトリ自身は、**最終的に全 `tddd.enabled == true` 層で `catalogue_spec_signal.enabled:
true`** を採用することを目標とする (dogfood の立場、vision v6 §6 の「ハーネス自身にも TDDD +
typestate-first」方針と整合)。

ただし実装 track 完了 (merge) 時点で全層が `enabled: true` である必要はない。D5.4 で定める段階的な
rollout フローに従い、本実装 track では「当該 track の対象 layer」を先行して `enabled: true` に設定し、
他の `tddd.enabled` 層で `spec_refs[]` 記入作業が未完なら当該 layer は `false` のまま merge しても良い。
残りの layer は後続 track で layer 別に記入 + flag flip する (段階適用原則)。

テンプレート採用者は自プロジェクトの `architecture-rules.json` で layer 単位に `true` / `false` を
選択できる (判断は各プロジェクトに委ねる、§6.3 の段階適用原則と同型)。

#### D5.4: 実装 track での flag 設定フロー

本 ADR 実装 track では以下の手順を取る:

1. 全実装 (CLI / domain pure fn / gate 接続) を追加
2. `architecture-rules.json` で当該実装 track の対象 layer を `catalogue_spec_signal.enabled: true` に
   設定 (初回 dogfood)
3. 当該 layer の catalogue の各 entry の `spec_refs[]` に **参照する spec 要素を記入** (未記入の
   entry があれば `/track:type-design` の通常 workflow で記入)
4. `cargo make ci` pass を確認 (signal / gate が想定通り動作)
5. verification.md に dogfood 結果を記録
6. 問題なければ flag `true` のまま merge (rollback 不要)

他の tddd.enabled 層で `spec_refs[]` 記入作業が未完なら、その layer は `false` のまま merge しても
良い (後続 track で layer 別に記入 + flag flip 可能、§D5.3 の段階適用)。

#### D5.5: 既存 active track の適用方針

本 ADR 実装 track merge 時点で、SoTOHE-core 自身の他 active track がある場合:

- 対象 layer の catalogue の `spec_refs[]` が記入済み: 新 gate を通過 (追加作業なし)
- 記入漏れ: 次 commit 時に 🔴 Red で block → author が記入作業を実施 (type-designer agent の通常
  workflow、`/track:type-design` の責務)

テンプレート採用プロジェクトでは、各プロジェクトが flag を `true` に flip するタイミングで同様の
記入作業を行う。

### D6: CLI-04 との scope 境界

`knowledge/strategy/TODO.md` 登録の **CLI-04 (HIGH)** は、**既存 Stage 2 (type→implementation) の
signal orchestration を infrastructure 層から usecase 層へ引き上げる** 未解決課題:

1. `libs/infrastructure/src/verify/merge_gate_adapter.rs::read_type_catalogue`
   (`<layer>-types.json` + `<layer>-type-signals.json` の 2 blob 整合 + `doc.set_signals(...)` 呼び
   出しが infrastructure 内に漏れている)
2. `libs/infrastructure/src/track/render.rs::sync_rendered_views` + `render_contract_map_view`
   (signal file 読取 / hash 検証 / 条件付き `set_signals` が render 合成内にインライン実装されている)

本 ADR で新設する catalogue-spec signal 経路は **最初から正しい層配置で実装** する (D3.7) ことで
CLI-04 の既存負債に加担しない。以下、両 track の scope 境界を明示する。

#### D6.1: 本 ADR scope 内

- 新 signal (catalogue-spec、Chain ②) の domain 純粋関数 (`evaluate_catalogue_entry_signal`、
  `check_catalogue_spec_ref_integrity`、`check_catalogue_spec_signals`、`SpecRefFinding`)
- 新 signal 用の usecase orchestration (`RefreshCatalogueSpecSignals` / `VerifyCatalogueSpecRefs`
  interactor)
- 新 signal 用の secondary port method (`TrackBlobReader::read_catalogue_for_spec_ref_check` 等)
- 新 signal 用の infrastructure adapter method (既存 `GitShowTrackBlobReader` への追加)
- 新 signal 用の CLI wrapper (`apps/cli/src/commands/track/catalogue_spec_signals.rs` /
  `apps/cli/src/commands/verify/catalogue_spec_refs.rs`)
- 新 signal ファイル `<layer>-catalogue-spec-signals.json` の codec (infrastructure)
- architecture-rules.json schema 拡張 (`catalogue_spec_signal.enabled` flag、D5.2)
- 新 signal を review_operational glob に追加 (D2.4)
- 関連 unit / integration test

#### D6.2: 本 ADR scope 外 (CLI-04 別 track で実施)

- 既存 `merge_gate_adapter.rs::read_type_catalogue` の usecase 層引き上げ
- 既存 `render.rs::sync_rendered_views` の usecase 層引き上げ (`usecase::render_track_views`
  interactor 新設)
- 既存 `spec_states::evaluate_layer_catalogue` (2026-04-18-1400 §D5) と render 経路の stale 検出
  helper 共通化
- 既存 `<layer>-type-signals.json` の schema / codec / pre-commit 経路の変更

#### D6.3: 2 track の進行順序に関する非依存性

本 ADR 実装 track と CLI-04 実装 track は **どちらが先でも独立に進行可能**:

##### CLI-04 が先に完了する場合

- 既存 Stage 2 orchestration が usecase 層に引き上げ済み
- 本 ADR の新 signal orchestration は最初から usecase 層 → 既存 Stage 2 と対称な綺麗な構造に収束
- `TrackBlobReader` port は両者が method を追加する形で共存

##### 本 ADR が先に完了する場合

- 既存 Stage 2 orchestration は infrastructure 層に残る (負債のまま)
- 本 ADR の新 signal orchestration は usecase 層 (clean)
- 結果: 新 Stage 2 (catalogue-spec) は綺麗、既存 Stage 2 (type-impl) は CLI-04 待ち の非対称状態
- この非対称は **一時的で許容** (両方を同一 track で済ませると scope 肥大化)
- CLI-04 が後続 track で完了すれば対称が回復

##### 並行進行の場合

- `TrackBlobReader` port への method 追加で conflict が出る可能性があるが、method 追加は単調操作
  なので後着 track が rebase で吸収可能
- 各 track の作業範囲は disjoint (新 signal 系 vs 既存 Stage 2 系) でコード衝突は少ない

#### D6.4: 推奨進行順

- 本 ADR 実装 track を **先行** することを推奨
- 理由: 本 ADR が vision v6 §0.2 Roadmap「計画中」項目 (SoT Chain ② のリンク活性化) の直接実装で、
  SoTOHE-core の Moat 確立に直結する。CLI-04 は既存負債のリファクタリングで緊急度は相対的に低い
- 本 ADR 完了後、CLI-04 は次段階の sizable track として独立に着手 (layer 別 rollout、D5.3 Phase 1 の
  完了状況を待ってから着手しても良い)

#### D6.5: 将来統合の余地

CLI-04 完了後、新 signal (本 ADR) と既存 Stage 2 (type-impl) の usecase orchestration が同一 pattern
(port 経由 I/O + domain pure fn + interactor) に収束する。その時点で共通化余地 (shared helper /
abstract interactor) があれば別 track で抽出する。本 ADR scope では共通化前の個別実装で OK
(premature abstraction を避ける)。

## Rejected Alternatives

### A. hash 検証結果を信号機に合成する (max() 合成 / SignalBasis 内部 nuance)

親 ADR §D3.2 の原記述「spec_refs[]: 🔵 (全 SpecRef が anchor 解決 + hash 一致) / 🔴 (1 件でも失敗)」の
ように、hash drift 検出結果を informal-priority rule と合成して 1 つの信号色に bundle する案、または
`SignalBasis` 相当の enum で内部的に distinguish する案。

却下理由:
- 信号機の責務 (grounding 品質の表現) と drift 検出 (機械的に解消可能な不整合) は性質が異なる
- bundle すると signal 色の semantic が曖昧化、review gate での解釈が複雑化
- hash drift は「機械的に解消可能な状態」(2026-04-18-1400 §D5) で、grounding の 🟡 warning と異なる
  fail-closed policy を適用すべき
- D1 で binary gate として独立配置する方が責務分離が clean

### B. SpecRef.hash を `<layer>-catalogue-spec-signals.json` に移す (catalogue 純粋宣言化)

catalogue を純粋宣言 (`{ file, anchor }` のみ) に保ち、hash は評価結果 artifact (signals file) に
集約する案。2026-04-18-1400 §D1 の「宣言 / 評価結果の物理分離」哲学をさらに徹底。

却下理由:
- 親 ADR §D2.1 の `SpecRef { file, anchor, hash }` schema が意図的に hash を含めた設計 (parent ADR
  amendment 要)
- pre-commit auto-refresh と drift detection の競合 (signals file を毎回再生成すると hash が常に
  最新化 → drift ゼロ → 検出機能破綻)
- 維持には ack machinery / refresh flag 等の追加機構が必要
- author intent の git diff audit が `review_operational` 除外で効かなくなる

### C. `<layer>-type-signals.json` に catalogue-spec signal を統合 (schema v2 bump)

既存 `<layer>-type-signals.json` に `catalogue_spec_signals` field を追加し 1 ファイルに集約する案。
pre-commit 再計算を 1 経路で統一、review scope 除外設定も既存エントリ継続で運用コスト低。

却下理由:
- 1 ファイルに 2 種類の signal (type→impl / type→spec) が混載、ファイル名 `type-signals` と内容の
  semantic 乖離
- schema v2 bump (破壊的変更) で codec / 既存運用の影響範囲が広がる
- 責務分離 (D2) を優先し独立ファイル化

### D. catalogue entry に CatalogueEntryId を新設

catalogue entry に explicit `id` field を追加し、`CatalogueSpecSignal` は `{id, signal}` で参照する案
(親 ADR §D2.1 の SpecElementId と対称化)。type rename 時の signal history 安定性向上。

却下理由:
- 本 ADR scope 外 (catalogue entry schema 変更は codec / baseline / render / 既存 Stage 2 整合へ波及)
- 現状 type_name は track 内で十分に安定 (rename は稀で git 履歴から追跡可)
- cross-artifact 参照要件 (Contract Map / impl-plan ref) が実証されたら別 track で段階導入
  (Reassess When)

### E. `generated_at` timestamp を `<layer>-catalogue-spec-signals.json` に記録

2026-04-18-1400 §D1 の既存 `<layer>-type-signals.json` と同じく ISO 8601 UTC 生成時刻を記録する案。

却下理由:
- signals file を入力 catalogue の pure function に保ち、入力不変時の無用な file 変更 (timestamp
  更新だけで git diff 発生) を排除する
- determinism / reproducibility を優先
- audit trail は git commit 履歴側で確保可能

### F. signal 再計算 → binary gate の順序 (逆順)

最初の draft では writer (signal 再計算) を先、validator (binary gate) を後にする順序を提案。

却下理由:
- signal (🔵🟡🔴) は grounding の意味 semantic を表現し、参照先に drift がある状態で算出すると
  misleading (🔵 Blue を persist するが実際には re-verification 必要)
- 先に hash drift を fail-closed で検出し、clean な参照状態でのみ signal 評価を走らせるのが semantic
  上正しい

### G. merge gate を既存順のまま新 gate を末尾挿入 (Option A)

`check_strict_merge_gate` の既存順 (Chain ① → Chain ③) を保ち、新 gate (Chain ②) を末尾に追加する案。
2026-04-12-1200 §D5 behavior を変更せず scope 最小。

却下理由:
- pre-commit 経路の SoT Chain bottom-up 順序 (Chain ③ → ②) と非対称
- SoT Chain 原則の一貫性を全 gate 経路で保つため、merge gate も bottom-up に reorder (Option B)
- 2026-04-12-1200 §D5 の adr-editor back-and-forth amendment が追加で必要になるが、Phase 1 の §D3.1
  max() 削除と同じ pattern で吸収可能

### H. advisory → enforced 即時 cutover (opt-in flag なし)

本 ADR 実装 merge 直後から全 tddd.enabled 層で一斉適用する案。`knowledge/conventions/no-backward-compat.md`
の素直な適用、簡潔な移行。

却下理由:
- template 採用者 (別プロジェクト) の layer 単位の選択自由度を奪う
- SoTOHE-core 自身の in-progress active track に disruption の可能性
- `architecture-rules.json` の既存 `tddd.enabled` pattern と対称な opt-in flag が自然な設計
  (2026-04-11-0002 §D1 の template-adopter friendly 方針と整合)

### I. CLI-04 (既存 Stage 2 orchestration 引き上げ) を本 ADR に吸収

既存 type-signals orchestration の infrastructure → usecase 層引き上げ (TODO CLI-04) を本 ADR 実装
track に含めて一括対応する案。

却下理由:
- 2 つの異なる関心事 (新 signal 追加 / 既存 orchestration リファクタ) を 1 track に含めると scope
  肥大化、review 負担大
- 両 track は独立に進行可能 (D6.3)、新 ADR は最初から正しい層配置で実装して CLI-04 負債に加担しない
- layer rollout (D5.3) の段階的性と整合

### J. flag を暫定 toggle として導入し全層 enabled 後に撤去

`catalogue_spec_signal.enabled` を migration 専用の暫定 toggle とし、全層 `true` 確定後に別 ADR で
撤去する案。

却下理由:
- flag は template 採用者 (別プロジェクト) の layer 単位の恒久的選択肢として残すべき (vision v6 §8.1
  「トポロジー別 Harness Template」と整合)
- 撤去すると採用プロジェクトが一律 enforce を強制される、柔軟性を損なう
- 既存 `tddd.enabled` 自体が同型の恒久 opt-in flag (2026-04-11-0002 §D1)

## Consequences

### Positive

- **SoT Chain ② のリンク活性化**: vision v6 §0.1 で Moat の中核として定義された 4 層 SoT Chain の
  中央 (型契約 → 仕様書) の評価機構が稼働し、Moat の完成度が Phase 1 (spec → ADR) + Phase 2 (type
  → impl、既存 Stage 2) + **Phase 2' (type → spec、本 ADR)** + (将来 impl 検証) の 3 リンク体制へ
- **Phase 1 spec signal と対称な informal-priority rule** により、catalogue 品質評価の一貫性が
  担保される。`spec_refs[]` / `informal_grounds[]` の非空状態が 🔵/🟡/🔴 にマップされる仕組みが
  Phase 1 と同型で、評価 logic の学習コストが低い
- **hash drift 検出の binary gate 分離** により、grounding 品質 (信号機 3 値) と drift 検出 (OK/ERROR
  二値) の関心が明確に分離。review の論点が混濁せず、fail-closed 原則 (hash drift は strict/interim
  問わず常に ERROR) が自然に働く
- **template 採用者の選択自由度** が `catalogue_spec_signal.enabled` flag で layer 単位に確保される
  (vision v6 §8.1 の「トポロジー別 Harness Template」方針と整合)
- **正しい層配置での新規実装** (domain pure fn + usecase orchestration + infrastructure adapter +
  CLI wrapper の 4 層) により、CLI-04 の既存負債に加担せず、将来 CLI-04 完了後の対称構造に自然に
  収束
- **SoT Chain bottom-up 検証原則が全 gate 経路で一貫**: pre-commit / merge gate の両方で Chain ③ →
  Chain ② → Chain ① の順序 (D3.4 / D3.6) で、根本 (実装) から上流 (ADR) に向かって順次検証する
  一貫性
- **既存 Stage 2 (`<layer>-type-signals.json`) 無改変** による backward-compatible な独立ファイル
  分離で、既存 behavior に副作用なし
- **業界比較での独占特徴強化**: vision v6 §F.5 で列挙された「信号機 (🔵🟡🔴) + spec_source 必須化」の
  特徴が schema レベルから実動作レベルに引き上げられ、既存 SDD ツール (Spec Kit / Kiro / Tessl /
  tsumiki) との差別化がさらに具体化

### Negative

- **親 ADR 2 本への adr-editor back-and-forth amendment 必要**:
  - `2026-04-19-1242-plan-artifact-workflow-restructure` §D3.2 の「spec_refs[]: 🔵 (anchor 解決 +
    hash 一致)」記述を D1 (hash 分離) に整合させる修正
  - `2026-04-12-1200-strict-spec-signal-gate-v2` §D5 の `check_strict_merge_gate` 既存順序を D3.6
    に整合させる修正
  - いずれも `signal-eval-drift-fix-2026-04-23` Phase 1 の §D3.1 max() 削除と同じ pattern で吸収
    可能だが、2 つの親 ADR amendment を連動させる必要
- **新 CLI 2 本の追加** (`sotp track catalogue-spec-signals` / `sotp verify catalogue-spec-refs`) で
  CLI surface area が拡大、documentation / help text / test の保守コスト
- **`architecture-rules.json` schema v2 の optional field 追加** — 破壊的変更ではないが parser /
  validator / existing projects (template 採用者) への周知が必要
- **pre-commit フローに 2 step 追加** (`dispatch_track_commit_message` の配線に新 signal 再計算 +
  binary gate を差し込み) → commit 体感速度への微影響 (< 0.1 秒/層)
- **merge gate の既存順序 reorder** (Chain ① → ③ から ③ → ② → ①) により、2026-04-12-1200 で規定
  された既存挙動の subtle な変更。AND 集約による最終 verdict は不変だが、エラー出力順序が変わる
- **dogfood 時に SoTOHE-core 自身の active track で `spec_refs[]` 記入漏れがあれば記入作業が必要**
  (本 ADR 実装 track の verification 段階で事前対応)
- **CLI-04 との 2 track 並行進行の場合**、`TrackBlobReader` port への method 追加で軽い rebase
  coord が必要 (method 追加は単調操作なので吸収可能)
- **advisory → enforced の flag 管理**: SoTOHE-core 自身は全 layer `enabled: true` で運用するが、
  template 採用者は `false` default から layer 別 flip を判断する運用負担 (既存 `tddd.enabled` と
  同型なので学習コストは低い)

### Neutral

- **既存 Stage 2 (`<layer>-type-signals.json`) は無改変**: schema / CLI / 挙動とも本 ADR で触らない。
  `generated_at` 削除を既存ファイルにも適用するかは将来の判断
- **non-active track は既存 active-track guard (2026-04-15-1012 §D1) で untouched**: 本 ADR が
  non-active track に及ぼす影響はない
- **`<layer>-catalogue-spec-signals.json` の `generated_at` 削除** は将来的に既存
  `<layer>-type-signals.json` も同様に clean pattern 化するか別 track の判断 (本 ADR では既存を
  触らない)
- **`CatalogueEntryId` 導入は本 ADR scope 外**、cross-artifact 参照要件 (Contract Map / impl-plan
  ref 等) が実証されたら後続 track で段階導入
- **Contract Map / Type Graph View への catalogue-spec signal overlay** は別 track / 別 ADR scope
  (2026-04-17-1528 §D5 signal_overlay の枠組みで処理)
- **CLI-04 との関係**: 本 ADR 先行 / CLI-04 後続を推奨するが、どちらが先でも独立進行可能 (D6.3)。
  どちらを先行するかは運用判断

## Reassess When

- **CatalogueEntryId 導入要件の発生**: Contract Map (2026-04-17-1528) / impl-plan / 他 track 等から
  catalogue entry への cross-artifact 参照要件が実証された場合、または type rename による signal
  history 断裂が実害として観測された場合 → 本 ADR D2.3 の type_name ベース signal を再検討し、
  catalogue entry への explicit `id` 付与と合わせて別 track で schema 拡張
- **`generated_at` clean pattern の既存 signal file 波及**: `<layer>-catalogue-spec-signals.json` の
  pure-function / no-timestamp 方針が運用で有用と実証された場合、既存 `<layer>-type-signals.json`
  (2026-04-18-1400 §D1) に同 pattern を適用する別 track を検討
- **CLI-04 完了**: 既存 Stage 2 orchestration が usecase 層に引き上げられた後、本 ADR の新 signal
  orchestration と既存 Stage 2 の共通化余地 (shared helper / abstract interactor) が見えた場合 →
  別 track で pattern 抽出 (premature abstraction を避けるため本 ADR では個別実装で OK、D6.5)
- **Contract Map / Type Graph View の signal overlay 統合**: 2026-04-17-1528 §D5 signal_overlay が
  catalogue-spec signal を同時表示する要件が固まった場合 → 別 ADR で overlay レイアウトを規定
- **親 ADR §Q15 の markdown anchor semantic 厳密化**: `AdrAnchor` / `ConventionAnchor` の厳密化と
  `AdrRef.hash` / `ConventionRef.hash` 追加 (親 ADR 2026-04-19-1242 §Q15) が別 ADR で確定した場合 →
  Phase 1 spec signal も hash 対応に拡張可能になり、Phase 1 / Phase 2 の評価 logic 非対称 (D4) が
  解消される。本 ADR の hash binary gate と integration 手順を再評価
- **`informal_grounds[]` の非空 → 🟡 Yellow rule が実運用で不適切と判明**: track 完了時に
  `informal_grounds[]` を全解消する運用が負担過大、または adhoc な使い方で spam Yellow が発生する
  場合 → 3 値の境界 rule を再考 (informal-priority 以外の mapping policy)
- **Cross-layer 型参照の catalogue 明示** (親 ADR 2026-04-11-0002 §D5 Phase 2 roadmap): usecase
  catalogue が domain catalogue の型を `spec_refs` 相当で明示参照する要件が固まった場合 →
  catalogue-spec signal を cross-layer にも拡張するか、別 signal として新設するか判断
- **template 採用プロジェクトからの flag 運用 feedback**: `catalogue_spec_signal.enabled` の layer
  単位 opt-in が実用的でないと判明した場合 (例: 全 layer 一括 opt-in がほとんど、または部分 opt-in
  で混乱が頻発) → flag 粒度 (global / per-layer / per-track) を再評価
- **hash binary gate の false positive / false negative 観測**: `SpecRef.hash` の比較 semantic
  (canonical serialization 規則等) が実装経験で不適切と判明した場合、または drift 検出漏れ / 過剰
  検出が発生した場合 → canonical 化アルゴリズムと gate の判定条件を再評価
- **merge gate reorder (D3.6) の副作用観測**: SoT Chain bottom-up 順序への reorder で既存のエラー
  報告フローやレビューワ期待と食い違う事象が発生した場合 → 順序復元または別の調整案を検討

## Related

### 親 ADR / 直接の参照先

- `knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md` §D1.3 / §D2.1 / §D2.2 /
  §D2.3 / §D3.2 / §D6.2 — 親 ADR (SoT Chain ② の schema + evaluation 定義 + 実装 delegate 指示)
- `knowledge/adr/2026-04-18-1400-tddd-ci-gate-and-signals-separation.md` §D1 / §D2 / §D4 / §D5 —
  `<layer>-type-signals.json` 物理分離 + pre-commit 自動再計算 + review_operational 除外 + stale
  検出の先行 pattern
- `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md` §D2 / §D4 / §D5 — strict/interim
  分離 + fail-closed 原則 + ヘキサゴナル層分離の先行 pattern (本 ADR の D4 / D3.6 の基盤)

### 信号機 / TDDD 系 ADR

- `knowledge/adr/2026-03-23-1010-three-level-signals.md` — 🔵/🟡/🔴 3 値 + SignalBasis 内部 nuance の
  先行定義 (本 ADR は 3 値純粋を維持、SignalBasis は採用せず)
- `knowledge/adr/2026-03-23-2120-two-stage-signal-architecture.md` — Stage 1 / Stage 2 の独立 gate
  構成の原典
- `knowledge/adr/2026-04-08-1800-reverse-signal-integration.md` — 既存 Stage 2 (type→implementation)
  signal の設計、TDDD 単一ゲート原則
- `knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md` — TDDD-02 baseline 4 グループ評価
- `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` §D1 / §D6 — architecture-rules.json の
  `tddd` block + layer-agnostic 原則 (本 ADR の `catalogue_spec_signal.enabled` flag の配置母体)
- `knowledge/adr/2026-04-11-0003-type-action-declarations.md` — `action: add/modify/reference/delete`
- `knowledge/adr/2026-04-13-1813-tddd-taxonomy-expansion.md` — 5→12 variants (TypeDefinitionKind)
- `knowledge/adr/2026-04-15-1012-catalogue-active-guard-fix.md` §D1 — active-track guard (本 ADR の
  非 active track reject の基盤)
- `knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md` — 13 variants (SecondaryAdapter 追加)
- `knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md` — rustdoc ベース Reality View (別 artifact)
- `knowledge/adr/2026-04-17-1528-tddd-contract-map.md` §D5 — catalogue ベース Contract Map と signal
  overlay の将来統合先 (本 ADR Reassess When 参照)

### Phase 1 先行事例

- `track/items/signal-eval-drift-fix-2026-04-23/` + PR #110 — Phase 1 spec signal の informal-priority
  rule 確定、親 ADR §D3.1 amendment の adr-editor back-and-forth 事例。本 ADR の D1 / D3.6 で同
  pattern を Phase 2 に適用

### Convention

- `knowledge/conventions/pre-track-adr-authoring.md` — ADR の事前整備 / adr-editor auto-edit 判定
- `knowledge/conventions/workflow-ceremony-minimization.md` — 人工状態 (approved/Status) 廃止、file
  存在ベース gate (本 ADR の architecture-rules.json flag も同原則)
- `knowledge/conventions/no-backward-compat.md` — 非 active track は write で protect、active は
  新 rule 適用 (本 ADR の D5.1 / Rejected §H の根拠)
- `knowledge/conventions/enforce-by-mechanism.md` — 機構による強制 (本 ADR の CI gate / pre-commit /
  merge gate 経由の機械的強制の上位原則)
- `knowledge/conventions/hexagonal-architecture.md` — 層依存方向の原則 (本 ADR の D3.7 / D6.1 層配置の
  根拠)
- `.claude/rules/04-coding-principles.md` — enum-first / typestate / newtype (本 ADR の
  `SpecRefFinding` enum kind 分離等で適用)

### 戦略文書

- `knowledge/strategy/vision.md` §0.1 / §1.2 / §3.5 / §6 / §8.1 / §F.5 — SoT Chain Moat 定義 +
  spec_source 必須化ロードマップ + ハーネス TDDD + Harness Template 採用者 opt-in 原則 + 業界独占
  7 特徴
- `knowledge/strategy/TODO.md` CLI-04 — 既存 Stage 2 orchestration の usecase 層引き上げ (本 ADR の
  D6 で scope 境界を明記)

### README

- `README.md` §SoT Chain / §参照チェーンの評価 / §ロードマップ — 評価表の cross-layer SSoT、「型契約
  → 仕様書の評価実装」計画中項目 (本 ADR が完成させる対象) / §探索的精緻化ループ

