---
adr_id: 2026-04-08-1800-reverse-signal-integration
decisions:
  - id: 2026-04-08-1800-reverse-signal-integration_grandfathered
    status: accepted
    grandfathered: true
---
# TDDD: 逆方向チェック信号機統合 + designer capability

## Status

Accepted

## Context

ADR `2026-04-08-0045-spec-code-consistency-check-design.md` で逆方向チェック (code → spec) が実装された。`sotp verify spec-code-consistency` が未宣言型/trait を CI error として報告する。

残る課題:

1. 逆方向チェックと信号機 (ConfidenceSignal) が独立した 2 つのパスで動作している
2. domain-types.json を誰がいつ作るかのワークフローが未定義
3. 未宣言型の扱いが未確定

### TDDD (Type-Definition-Driven Development)

TDD が「テストを先に書き、コードがテストを通す」ように、TDDD では domain-types.json を「型レベルのテスト」として先に書き、実装がそれに従う。

- **domain-types.json** = 型レベルのテスト（期待する型、メンバー、遷移を宣言）
- **forward check** (spec → code) = テストが通っているか（定義した型がコードに存在するか）
- **reverse check** (code → spec) = テストに書かれていないコードが存在しないか（未定義の型がコードにないか）

forward Yellow（定義済みだが未実装）は作業途中の正常状態 (WIP)。reverse Red（実装済みだが未定義）は TDDD 違反。

## Decision

### 1. 逆方向 Red シグナル変換

`check_consistency` の `undeclared_types` / `undeclared_traits` を `DomainTypeSignal` (Red) に変換する関数を domain 層に追加。

- `kind_tag`: `"undeclared_type"` / `"undeclared_trait"`
- `signal`: `ConfidenceSignal::Red`
- `found_type`: `true`
- `missing_items` / `extra_items`: 空

forward signals と reverse signals が同じ `Vec<DomainTypeSignal>` で統一される。

### 2. domain-type-signals コマンドの逆方向チェック拡張

`sotp track domain-type-signals` を拡張:

1. domain-types.json 読み込み
2. rustdoc → TypeGraph 構築
3. `check_consistency` で逆方向チェック
4. undeclared types/traits を Red シグナルとして signals に追加
5. domain-types.json に保存（signals を更新）
6. domain-types.md レンダリング
7. サマリ出力: `"blue=N yellow=M red=K (undeclared=U)"`
8. undeclared > 0 の場合 `/track:design` の再実行を促すメッセージを表示

**domain-types.json が存在しない場合**: コマンドはエラー終了し `/track:design` を先に実行せよ と案内する。domain-types.json の初回作成は `/track:design` の責務であり、domain-type-signals は既存ファイルの評価のみを行う。

### 3. forward 未実装 = Yellow、reverse 未定義 = Red

`evaluate_domain_type_signals` を拡張し、定義済みだが未実装の型に Yellow シグナルを返す。
逆方向の未定義型は従来通り Red シグナル。

TDD アナロジーに対応する 3 値:

| 状態 | シグナル | TDD 対応 |
|------|---------|----------|
| 定義済み + 実装済み + 構造一致 | 🔵 Blue | テスト pass |
| 定義済み + 未実装 | 🟡 Yellow | テスト書いたがまだ pass していない (WIP) |
| 未定義 + 実装済み | 🔴 Red | テストなしのコード (TDDD 違反) |

### 4. verify spec-states ゲート: 2 段階判定

**途中コミット時** (通常の `verify spec-states`):

- Red → fail + `/track:design を再実行せよ` と案内
- Yellow → pass（作業途中、タスク消化で Blue に昇格）
- Blue → pass

**merge 時** (track 完了判定):

- Red → fail
- Yellow → fail（未実装の型定義が残っている）
- Blue のみ → pass

kind_tag による forward/reverse 区別は不要。シグナルの色だけで判定できる。

CI フローでは `domain-type-signals → verify spec-states` の順序で実行。

### 5. designer capability + /track:design コマンド

`agent-profiles.json` に `designer` capability を追加。既定 provider は `claude`（planner と同じ）。

`/track:design` コマンドを `.claude/commands/track/design.md` に作成:

1. 対象トラックの `plan.md` を読み込み、必要な型を分析
2. 既存の `domain-types.json` があれば読み込み（増分更新）
3. 既存コードがあれば TypeGraph も参照
4. designer capability を呼び出し、DomainTypeKind / members / transitions を設計
5. `domain-types.json` を生成・更新

domain-types.json の初回作成はこのコマンドが担う。これにより TDDD フローが成立する:
`/track:plan → /track:design → /track:implement`

### 6. ワークフロー導線

- `/track:plan` 完了後のメッセージに `/track:design` を次ステップとして案内
- `registry.md` の Next 列に `/track:design` を追加
- `DEVELOPER_AI_WORKFLOW.md` / `knowledge/WORKFLOW.md` に TDDD フローを追記

## Rejected Alternatives

- **未宣言型の自動追加 (auto_add_undeclared + approved: false → Yellow)**: TDDD に反する。型定義はコードから逆生成するのではなく、designer が意図を持って先に書くべき
- **kind_tag による forward/reverse 区別でゲート判定**: forward Red を kind_tag で識別して pass する案。Yellow を使えば色だけで判定でき、kind_tag 依存が不要になるため却下
- **逆方向チェックを独立 CI ゲートとして維持**: 信号機に統合し単一ゲートを実現
- **domain-types.json 不在時に空ドキュメントから自動生成**: domain-type-signals が勝手にファイルを作ると /track:design をバイパスする TDDD 違反パスが生まれる

## Consequences

- Good: TDDD フローが確立される — 型定義を先に書き、実装がそれに従う
- Good: Yellow (未実装 WIP) と Red (TDDD 違反) の色だけで判定でき、途中コミットと TDDD 強制を両立
- Good: verify spec-states が前方向+逆方向の単一ゲートとして機能
- Good: designer capability により型定義の作成が体系的なプロセスになる
- Bad: /track:design を実行しないと domain-type-signals が動かない（意図的な制約）

## Reassess When

- designer capability の provider を Claude 以外に変更する需要が生じた時
- domain-types.json の手動修正がボトルネックになる場合、ユーザーが確認・承認するステップを経る提案表示（Red のまま修正候補を表示するだけで自動書き込みは行わない）を再検討。ただし TDDD 方針 (型定義は先に人間が書く) は維持し、自動書き込み (auto-add) は引き続き採用しない
- verify spec-code-consistency コマンドの役割を再評価 — 信号機統合後に診断専用として維持するか廃止するか
