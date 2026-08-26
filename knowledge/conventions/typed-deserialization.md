# Typed Deserialization Convention

## Rule

Infrastructure の verify/guard コードで外部データ (JSON, YAML, TOML) を読む場合、`serde_json::Value` (または同等の untyped API) を手動で走査してはならない。代わりに `#[derive(Deserialize)]` 付きの型を定義し、JSON は `serde_json::from_str::<T>()`、YAML は `serde_yaml::from_str::<T>()`、TOML は `toml::from_str::<T>()` のように入力形式に対応する typed decoder で直接デシリアライズすること。

> **強制先**: review 観点 — infrastructure scope

## Usecase Boundary Handoff

- 外部入力を usecase に渡すとき、command usecase の入力 boundary には検証済みの usecase 所有 `Command` 型を 1 個だけ、query usecase の入力 boundary には検証済みの usecase 所有 `Query` 型を 1 個だけ渡す。未検証の `String` や untyped data を入力 boundary の公開シグネチャに置いてはならない。
  > **強制先**: review 観点 — types / usecase / cli_driver scope
- `String` から対応する `Command` または `Query` へのパースと検証は usecase 所有の boundary 型が担い、CLI の driving path（規約上の `cli`、実装上は `cli_driver`）がその処理を一度だけ呼び出してから対応する入力 boundary を呼び出す。薄い `cli` bin は `cli_driver` を呼び出すだけで usecase crate に直接依存しない。
  > **強制先**: review 観点 — usecase / cli / cli_driver scope
- domain enum の鏡像を CLI 側に定義してはならない。境界語彙は usecase 所有の boundary 型に統一し、`cli` と `cli_driver` は domain 型を直接参照しない。
  > **強制先**: review 観点 — usecase / cli / cli_driver scope
- この handoff 規則は新規の production boundary code に適用し、既存の境界実装はこの規約だけを理由に遡及改修しない。
  > **強制先**: review 観点 — usecase / cli / cli_driver scope

## Rationale

`serde_json::Value` の手動走査は以下のリスクを生む:

1. **Silent data loss**: `filter_map(|v| v.as_str())` のようなパターンで不正データが黙って捨てられる (fail-open)
2. **型と入力検証の不整合**: Rust 側のフィールド名・型はコンパイル時に検査され、入力側のフィールド名・型の不一致は decoder 実行時にエラーとして検出される
3. **重複バリデーション**: 各フィールドの存在チェックと型変換を手書きすることになり、DRY 違反

Typed deserialization は serde が自動的に:
- 必須フィールドの欠落をエラーにする
- 型不一致をエラーにする
- `#[serde(default)]` で明示的なデフォルトを提供する

## Examples

```rust
// Bad: hand-rolled Value walking
let concern = entry.get("concern").and_then(|v| v.as_str()).ok_or("missing")?;
let allowed_in: Vec<String> = raw.iter().filter_map(|v| v.as_str().map(String::from)).collect();

// Good: typed deserialization
#[derive(Deserialize)]
struct CanonicalRule {
    concern: String,
    forbidden_patterns: Vec<String>,
    allowed_in: Vec<String>,
    #[serde(default)]
    convention: String,
}
let rules: ArchitectureRules = serde_json::from_str(&content)?;
```

## Scope

### Typed deserialization

- `libs/infrastructure/src/verify/` — all JSON/YAML/TOML parsing and external-format DTO ownership
  > **強制先**: review 観点 — infrastructure scope
- `libs/domain/` — remains serde-free; validated domain values are created through domain constructors or ports after boundary parsing
  > **強制先**: review 観点 — domain scope
- 外部形式の typed deserialization は新規 infrastructure code に適用し、既存コードは段階的に移行する。
  > **強制先**: review 観点 — infrastructure scope

## Exceptions

### Typed deserialization

- infrastructure の typed deserialization で構造が事前に不明な JSON (e.g., `#[serde(flatten)]` で unknown fields を保持するケース) は `Value` の使用を許容する
  > **強制先**: review 観点 — infrastructure scope
- infrastructure の typed deserialization に関するテストコードは対象外とする
  > **強制先**: review 観点 — infrastructure scope
