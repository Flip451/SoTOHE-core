# Typed Deserialization Convention

## Rule

Verify/guard コードで外部データ (JSON, YAML, TOML) を読む場合、`serde_json::Value` (または同等の untyped API) を手動で走査してはならない。代わりに `#[derive(Deserialize)]` 付きの型を定義し、`serde_json::from_str::<T>()` で直接デシリアライズすること。

## Rationale

`serde_json::Value` の手動走査は以下のリスクを生む:

1. **Silent data loss**: `filter_map(|v| v.as_str())` のようなパターンで不正データが黙って捨てられる (fail-open)
2. **型安全性の欠如**: フィールド名のタイポや型の不一致がコンパイル時に検出されない
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

- `libs/infrastructure/src/verify/` — all JSON/YAML parsing
- `libs/domain/src/guard/` — config file parsing
- 新規コードに適用。既存コードは段階的に移行。

## Exceptions

- 構造が事前に不明な JSON (e.g., `#[serde(flatten)]` で unknown fields を保持するケース) は `Value` の使用を許容する
- テストコードは対象外
