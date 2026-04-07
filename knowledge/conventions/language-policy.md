# Language Policy Convention

## Rust First

すべての新規ロジックは Rust で実装する。Python への新規投資は最小化する。

**理由**: SoTOHE-core は Rust CLI (sotp) が中心。Python はレガシースクリプト（`scripts/`）と
hook のセルフテストにのみ残存する。新規の検証ロジック・パーサー・ワークフロー制御は
すべて Rust の domain/usecase/infrastructure/cli 層に配置する。

**Fail-closed 前提**: hook やガードは常に fail-closed（解析失敗 → Block）で設計する。
これは Python でも Rust でも同じだが、Rust の型システムで fail-closed を構造的に強制できるため、
新規ロジックは Rust を優先する。

## ファイル名タイムスタンプ

`knowledge/` 配下のファイル名タイムスタンプはローカル時間（JST）を使用する。
UTC ではない。

例: `2026-04-07-2244-forgecode-comparison.md`（JST 22:44）
