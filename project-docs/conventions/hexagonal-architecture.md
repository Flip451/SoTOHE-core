# Hexagonal Architecture Convention

## Purpose

Layer boundaries, port placement, and adapter rules for the SoTOHE-core workspace.
ヘキサゴナルアーキテクチャの標準パターンに則り、各層の責務を明確にする。

## Scope

- Applies to: `libs/domain/`, `libs/usecase/`, `libs/infrastructure/`, `apps/cli/` の全 Rust コード
- Does not apply to: `vendor/`, テストコード（`#[cfg(test)]`）、ビルドスクリプト

## Layer Dependencies

```
domain          依存先なし（最内層）
usecase         → domain
infrastructure  → domain, usecase
CLI             → domain, usecase, infrastructure（composition root）
```

`docs/architecture-rules.json` と `deny.toml` で強制。`cargo make check-layers` + `cargo make deny` で検証。

## Port Placement Rules

| ポートの種類 | 定義場所 | 例 |
|---|---|---|
| 永続化・集約に関するポート | **domain** | `TrackReader`, `TrackWriter`, `WorktreeReader` |
| アプリケーションサービスが必要とするポート | **usecase** | `GitHasher`, `RecordRoundProtocol` |

- domain ポート: ドメインの概念（永続化、ワークツリー状態）を抽象化
- usecase ポート: インフラの機能（git hash、二相コミット等）を抽象化。domain に属さない概念

## Adapter Rules

| 項目 | ルール |
|---|---|
| 定義場所 | **infrastructure** |
| 責務 | ポートの実装（domain trait + usecase trait の両方を実装可能） |
| 厚さ | 制限なし。外部システムとの統合ロジック（git index protocol 等）は adapter が担当して良い |

## Usecase Layer Purity Rules

usecase 層は**純粋なオーケストレーター**であり、以下を**禁止**する:

### std I/O モジュール（網羅的ブロック）

| 禁止 | 理由 | 正しい対処 |
|---|---|---|
| `std::fs::*` | ファイル I/O | CLI がファイルを読んでデータを渡す |
| `std::net::*` | ネットワーク I/O | infrastructure adapter 経由 |
| `std::process::*` | プロセス管理 | port trait 経由 |
| `std::io::*` | I/O trait・型 | usecase 独自のエラー型を定義 |
| `std::env::*` | 環境変数アクセス | CLI が設定を読んで引数で渡す |

### 暗黙的外部依存

| 禁止 | 理由 | 正しい対処 |
|---|---|---|
| `chrono::Utc::now()` | 暗黙的な時刻依存 | タイムスタンプを引数で受け取る |
| `std::time::SystemTime` | システム時計アクセス | 同上 |
| `std::time::Instant` | 単調時計アクセス | 同上 |

### 出力マクロ

| 禁止 | 理由 | 正しい対処 |
|---|---|---|
| `println!` / `eprintln!` | 出力は CLI の責務 | `Result<T, E>` で結果を返す |
| `print!` / `eprint!` | 同上 | 同上 |

> **CI 検証**: `sotp verify usecase-purity`（syn AST ベース）が上記全パターンを検出。
> use import（alias, glob, self）も追跡。現在 warning-only、INF-17 で error 昇格予定。

## CLI as Composition Root

CLI の責務は:
1. clap で引数をパース
2. infrastructure adapter を構築（DI）
3. usecase 関数を呼び出し
4. 結果を出力 + ExitCode にマッピング

CLI の非テストコードに `domain::` / `infrastructure::` への直接参照があっても良い（composition root として adapter を構築するため）。

## Examples

Good:
```rust
// usecase: 純粋なオーケストレーション
pub fn check_approved(
    input: CheckApprovedInput,
    reader: &impl TrackReader,      // domain port
    writer: &impl TrackWriter,      // domain port
    hasher: &impl GitHasher,        // usecase port
) -> Result<(), String> {
    let hash = hasher.normalized_hash(&input.items_dir, &track_id)?;
    let track = reader.find(&track_id)?;
    // domain logic...
    Ok(())
}
```

Bad:
```rust
// usecase に I/O が混入
pub fn check_approved(input: CheckApprovedInput) -> Result<(), String> {
    let hash = SystemGitRepo::discover()?.index_tree_hash(...)?;  // infra 直接参照
    let content = std::fs::read_to_string(path)?;                  // ファイル I/O
    println!("[OK]");                                                // 出力
}
```

## Exceptions

- `extract_verdict_from_content` (usecase): テキストパース（`&str` → verdict）は純粋関数なので usecase に置いてよい。ファイル読み込みは CLI が行う。

## Review Checklist

- [ ] usecase に `std::fs`, `chrono::Utc::now`, `println!`, `eprintln!` がないか
- [ ] port trait が正しい層に定義されているか（domain 概念 → domain、アプリ機能 → usecase）
- [ ] adapter が infrastructure に配置されているか
- [ ] CLI は composition root パターンに従っているか
- [ ] `cargo make check-layers` が pass するか
- [ ] `cargo make deny` が pass するか

## Related Documents

- `docs/architecture-rules.json`: レイヤー依存ルールの SSoT
- `deny.toml`: Cargo レベルのレイヤー強制
- `.claude/rules/04-coding-principles.md`: Trait-Based Abstraction
- `track/tech-stack.md`: アーキテクチャパターンの決定
