# TDDD 実装計画

> **作成日**: 2026-04-11
> **ADR**: TDDD-02 (`0001`), TDDD-01 (`0002`), TDDD-03 (`0003`)
> **前提**: ADR 議論完了、実装順序確定

---

## 概要

TDDD (Type-Definition-Driven Development) を 3 段階で強化する。
各段階は独立した track として実装可能。

```
Step 1: TDDD-02  Baseline reverse signal        ← 即効性: 既存型ノイズ解消
Step 2: TDDD-03  Type action declarations        ← 制約解消: 型削除と TDDD の併用
Step 3: TDDD-01  Multilayer extension            ← 大規模: 多層化 + シグネチャ + TypeGraph 拡張
```

## Step 1: TDDD-02 — Baseline reverse signal

**ADR**: `knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md`

**目的**: reverse check の既存型ノイズ (100+ Red) を排除し、CI を通るようにする

**scope**:
1. 各層に `tddd/` モジュール作成 + 既存ファイルの移動 (`domain_types.rs` → `tddd/catalogue.rs`, `domain_types_codec.rs` → `tddd/catalogue_codec.rs`, `domain_state_signals.rs` → `tddd/signals.rs`)
2. `TypeBaseline` / `TypeBaselineEntry` / `TraitBaselineEntry` 型を domain 層に追加 (`libs/domain/src/tddd/baseline.rs`)
3. Baseline codec を infrastructure 層に追加 (`libs/infrastructure/src/tddd/baseline_codec.rs`)
4. TypeGraph → TypeBaseline 変換を infrastructure 層に追加 (`libs/infrastructure/src/tddd/baseline_builder.rs`)
5. `check_consistency` に `&TypeBaseline` 引数を追加し、4 グループ評価を実装 (`libs/domain/src/tddd/consistency.rs`)
6. CLI `baseline-capture` コマンドを追加 (`apps/cli/src/commands/track/tddd/baseline.rs`): baseline 生成専用
7. CLI `domain-type-signals` を拡張 (`apps/cli/src/commands/track/tddd/signals.rs`): baseline 読み込み + 4 グループ評価。baseline がなければエラー
8. `/track:design` の Step 4 で `baseline-capture` を呼び出すよう更新

**成果物**:
- `domain-types-baseline.json` (track ディレクトリに生成、コミット推奨)
- 4 グループ評価 (A\B, A∩B, B\A, ∁(A∪B)∩C)

**制約** (Step 2 まで):
- TDDD を使用する track では既存型の削除を含められない

**見積もり**: 小〜中規模

## Step 2: TDDD-03 — Type action declarations

**ADR**: `knowledge/adr/2026-04-11-0003-type-action-declarations.md`

**目的**: Step 1 の「型削除と TDDD の併用不可」制約を解消する

**scope**:
1. `DomainTypeEntry` に `action` フィールド追加 (domain 層)
2. codec の `action` 対応 (infrastructure 層)
3. forward check に `action` 別ロジック追加:
   - `"add"` (デフォルト): C に存在し宣言と一致 → Blue
   - `"modify"`: 同上
   - `"reference"`: 同上 (参照目的の転記)
   - `"delete"`: C に存在しない → Blue
4. `action` と baseline の矛盾検出 (警告)
5. `/track:design` で `action` を選択できるよう UX 更新

**成果物**:
- `action: "add" | "modify" | "reference" | "delete"` フィールド
- Step 1 の制約解消

**見積もり**: 小規模

## Step 3: TDDD-01 — Multilayer extension

**ADR**: `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md`

**目的**: TDDD を domain 層以外にも適用可能にし、メソッドシグネチャの検証精度を向上させる

**scope**:

### 3a. リネーム
- `DomainTypeKind` → `TypeDefinitionKind`
- `DomainTypeEntry` → `TypeCatalogueEntry`
- `DomainTypesDocument` → `TypeCatalogueDocument`
- `domain-types.json` のファイル名は `catalogue_file` で設定可能 (デフォルト: `<crate>-types.json`)
- ファイルの `tddd/` 移動は TDDD-02 (Step 1) で完了済み。本 Step では型名・ファイル内シンボルのリネームのみ
- 後方互換性は対応しない (一括リネーム)

### 3b. MethodDeclaration 導入
- `MethodDeclaration` / `ParamDeclaration` 型を domain 層に追加
- `expected_methods: Vec<String>` → `Vec<MethodDeclaration>` に拡張
- JSON スキーマ: `{ name, receiver, params: [{ name, ty }], returns, async }`
- 型表現: モジュールパスは短縮名、ジェネリクス構造は完全保持 (完全マッチ)
- L1 検証ロジック: forward (Yellow) + reverse (Red)

### 3c. TypeGraph 拡張
- `TypeInfo::members`: `Vec<String>` → `Vec<MemberDeclaration>` (field 名 + 型)
- `FunctionInfo`: `params`, `returns`, `receiver`, `is_async` を構造化フィールドとして追加
- `TypeNode`: `method_return_types: HashSet<String>` → `methods: Vec<MethodDeclaration>`
- `TraitNode`: `method_names: Vec<String>` → `methods: Vec<MethodDeclaration>`
- `build_type_graph`: FunctionInfo → MethodDeclaration 変換を実装
- Step 1 の baseline 比較解像度が自動的にシグネチャレベルに向上

### 3d. 多層化
- `architecture-rules.json` に `layers[].tddd` ブロック追加
- `sotp track type-signals` を `--layer` パラメタライズ
- `sotp verify spec-states` を全層 catalogue AND 集約に拡張
- `/track:design` を多層対応 (`architecture-rules.json` から層を発見)

**成果物**:
- 任意の層構成で TDDD 利用可能
- メソッドシグネチャの完全マッチ検証
- `MethodDeclaration` がカタログ / TypeGraph / baseline の 3 箇所で共有

**見積もり**: 大規模 (3a〜3d を sub-task に分割推奨)

## 依存関係

```
Step 1 (TDDD-02)
  ↓
Step 2 (TDDD-03)  ← Step 1 の制約解消
  ↓
Step 3 (TDDD-01)  ← Step 1 の baseline を拡張 + Step 2 の action をリネーム後の型に適用
```

Step 2 と Step 3 の間に厳密な依存はないが、Step 3 のリネームが Step 2 のフィールドにも影響するため、Step 2 → Step 3 の順が手戻りが少ない。

**前提**: 各 Step は順次実行し、前の Step の track を完了してから次の Step を開始する。in-progress track が複数の Step をまたぐ状況は想定しない。後方互換性は対応しないため、Step 間のスキーマ移行は不要。
