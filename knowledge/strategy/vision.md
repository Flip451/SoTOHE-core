# SoTOHE-core ビジョン v3

> **作成日**: 2026-03-22
> **前版**: `tmp/archive-2026-03-20/vision-2026-03-20.md` (v2)
> **変更理由**: SoTOHE-core 自身のコードと、テンプレートが生成するプロジェクトのコードを明確に区別

---

## 0. 最も重要な区別

SoTOHE-core には 2 つの側面がある：

| 側面 | 説明 | 設計哲学の適用度 |
|---|---|---|
| **ハーネス実装** | sotp CLI, track workflow, hooks, verify — テンプレートの基盤コード | 実用的に良いコードであればよい。typestate 等の理想的パターンの適用は不要 |
| **テンプレート出力** | このテンプレートを使って生成されるプロジェクトの spec, テスト, 実装コード | **ここに設計哲学のすべてを反映する** |

v1-v2 ではこの区別が曖昧で、ハーネス自身のリファクタリングに設計哲学を全力投入していた。
v3 では「テンプレートの出力品質を最大化する」ことに集中する。

---

## 1. 核心: テスト生成パイプライン

```
要件 → テスト（大量・矛盾なし・網羅的） → 実装（テストを通すだけ）
      ↑ テンプレートが提供する Moat
```

SoTOHE-core が提供する価値は「プロジェクトのテストを効率的に・大量に・矛盾なく生成する仕組み」。

---

## 2. テスト数を減らす手段としての型

Make Illegal States Unrepresentable は**テスト生成のコスト削減策**。

```
String:  テストで全パターン網羅 → 無限の入力空間
enum:    コンパイラが排除済み → 有限パターン
typestate: 不正遷移は関数が存在しない → テスト不要（コンパイラが保証）
```

### typestate パターン（生成プロジェクト向け）

テンプレートが生成するプロジェクトでは、有効な遷移だけ関数として定義する：

```rust
// 有効遷移: 関数が存在する = 遷移表の埋まっている部分
impl InProgress {
    pub fn pass_fast(self, hash: CodeHash) -> FastPassed { ... }
}

// 無効遷移: 関数が存在しない = 遷移表の - の部分
// NotStarted に pass_fast は存在しない → コンパイルエラー

// 永続化用 enum（serde 互換）
pub enum ReviewStatus { NotStarted, InProgress, FastPassed, Approved, Invalidated }
impl ReviewStatus {
    pub fn as_in_progress(self) -> Option<InProgress> { ... }
}
```

**関数の存在がテスト対象**。syn で抽出した関数一覧 = 遷移表。

SoTOHE-core 自身のコードにこのパターンを適用する必要はない。

---

## 3. 投資比率

```
v1:  guardrails 70% + テスト支援 10% + 仕様品質 20%
v2:  guardrails 20% + テスト支援 40% + 仕様品質 40%
v3:  ハーネス保守 20% + テスト生成ツール 40% + テンプレート品質 40%
```

---

## 4. SSoT の原則: Single Authority, not Single File

各情報の正規の置き場所が 1 つであれば SSoT は成立する。

| 情報 | SSoT | ファイル |
|---|---|---|
| トラック状態 | `metadata.json` | 1 トラック 1 ファイル |
| 要件 | `spec.md` | 1 トラック 1 ファイル |
| タスク分解 | `plan.md` | metadata.json から生成される view |
| 検証結果 | `verification.md` | 独立ファイル |

---

## 5. 探索的精緻化ループ

```
[ざっくり要件] ⇆ [spec.md + 信号機] ⇆ [型スケルトン + テストスケルトン]
      ↑                                          ↓
      └────────────── 矛盾・漏れの発見 ───────────┘
```

### `/track:plan` の 3 フェーズ

```
Phase A: 要件スケッチ
  → spec.md に Domain States + State Transitions + 信号機 + 具体例 (Given/When/Then)

Phase B: 型 + テストによる検証ループ
  → typestate スケルトン生成（有効遷移の関数のみ）
  → テストスケルトン生成（存在する関数に対してのみ）
  → cargo check で整合性確認
  → 矛盾発見 → spec にフィードバック

Phase C: タスク分解
  → 型 + テストスケルトンが既にある状態で実装タスクを分解
```

---

## 6. テンプレートが生成するプロジェクトの層構造

| 層 | 責務 | テスト戦略 |
|---|---|---|
| **domain types** | struct, enum, typestate 型。構築メソッド | コンパイラが保証（テスト最小） |
| **domain impl** | 状態遷移関数の実装。バリデーション | proptest + spec 例変換 |
| **usecase** | domain 関数を `impl Fn` で受け取り、順番に呼ぶだけ | モック自動生成（クロージャ） |
| **infrastructure** | I/O 具象実装 | 統合テスト（最小限） |
| **CLI / API** | DI 組み立て + usecase 呼び出し | ほぼ不要 |

### usecase の `impl Fn` パターン

テスト生成の自動化に最適化：

```rust
pub fn record_round(
    load: impl Fn(&TrackId) -> Result<Doc>,
    apply: impl Fn(Doc, &RoundInput) -> Result<Doc>,
    save: impl Fn(&Doc) -> Result<()>,
    input: RoundInput,
) -> Result<()> {
    let doc = load(&input.track_id)?;
    let new_doc = apply(doc, &input)?;
    save(&new_doc)?;
    Ok(())
}
```

テスト = クロージャ渡しで呼び出し順序を検証。mockall 不要。

### domain のファイル分割: DDD 概念 + pub 可視性フィルタ

```
libs/domain/src/review/
├── types.rs       # 型 + 構築メソッド + バリデーション
├── state.rs       # 状態遷移ロジック
├── concern.rs     # ReviewConcern 概念
├── escalation.rs  # EscalationPhase 概念
└── error.rs       # エラー型
```

- ファイル分割は **DDD 概念**（人間向け）
- AI 向けフィルタは **`pub` 可視性**（Rust の既存メカニズム）
- 追加のファイル規約やモジュール構造は不要

---

## 7. BRIDGE-01: テンプレートが提供するテスト生成ツール

### `sotp export-schema`

**生成プロジェクトの** 1 つ以上の Rust source root（例: `libs/domain/src/`, `libs/usecase/src/`）を `syn` でパースし、`pub` なアイテムのシグネチャだけ抽出：

```
入力: 生成プロジェクトの libs/domain/src/**/*.rs, libs/usecase/src/**/*.rs, ...
出力:
  pub enum Verdict { ZeroFindings, FindingsRemain }
  pub struct ReviewConcern(...)
  impl InProgress { pub fn pass_fast(self) -> FastPassed; }
  impl FastPassed { pub fn pass_final(self) -> Approved; }
```

- デフォルトの主要対象は domain layer だが、multi-path 指定で他 layer も集約できる
- 関数の存在 = 有効な遷移。不在 = 無効な遷移（テスト不要）
- `pub(crate)` や非 pub は除外（内部実装）
- 29K LOC → 数百行。AI のコンテキスト効率が桁違い

### テスト生成の 3 手法

| 手法 | 入力 | 出力 |
|---|---|---|
| spec 例 → テスト変換 | spec.md の Given/When/Then | `#[test]` 関数 |
| proptest + typestate | `sotp export-schema` のシグネチャ | `proptest!` マクロ |
| usecase モック自動生成 | usecase の `impl Fn` シグネチャ | クロージャモックテスト |

---

## 8. SoTOHE-core 自身のコード方針

ハーネス自身のコードは「実用的に良いコード」であればよい：

- Phase 1.5 のリファクタリング（CLI 肥大化解消, domain 型化）→ **コード品質改善として継続**
- 既存の trait ベース DI → **そのまま維持**（impl Fn への移行不要）
- typestate パターン → **適用しない**（既に動いているコードを壊す理由がない）
- ファイル分割 → **DDD 概念で整理**（CLI-02 の実装通り）

---

## 9. 多言語展開

検証レベル Gold/Silver/Bronze の枠組みは維持。
テスト生成パイプライン（`sotp export-schema` + テストスケルトン生成）は
`syn` を言語別パーサーに差し替えれば他言語にも適用可能。

詳細: `tmp/multi-language-design-2026-03-18.md`
