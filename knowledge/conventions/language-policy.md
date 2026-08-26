# Language Policy Convention

## Rust First

すべての新規ロジックは Rust で実装する。Python への新規投資は最小化する。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope

**理由**: SoTOHE-core は Rust CLI (sotp) が中心。Python はレガシースクリプト（`scripts/`）と
hook のセルフテストにのみ残存する。新規の検証ロジック・パーサー・ワークフロー制御は
すべて Rust の domain/usecase/infrastructure/cli 層に配置する。

**Fail-closed 前提**: hook やガードは常に fail-closed（解析失敗 → Block）で設計する。
これは Python でも Rust でも同じだが、Rust の型システムで fail-closed を構造的に強制できるため、
新規ロジックは Rust を優先する。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition scope

`scripts/` の既存 Python と hook セルフテストには専用の review scope / lint がないため、そこへの新規投資の抑制は自動強制しない。

> **強制先**: 強制なし (明記) — scripts/ の専用 review scope / lint は未整備

## ファイル名タイムスタンプ

`knowledge/` 配下のファイル名タイムスタンプはローカル時間（JST）を使用する。
UTC ではない。

> **強制先**: 強制なし (明記) — JST のファイル名を判定する既存機構なし

例: `<YYYY-MM-DD-HHMM>-forgecode-comparison.md`（JST 22:44）
