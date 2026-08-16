---
adr_id: "2026-08-14-1048-child-output-ring-capture"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-14"
    candidate_selection: "from:[A,B,C,C+config] chose:C"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-14"
    status: proposed
---
# 子プロセスの診断出力は末尾リングで保持し、出力量で kill しない

## Context

review-fix runner と program_runner は子プロセスの stdout / stderr を各 1 MiB で収集し、超過すると SIGKILL する。codex CLI はセッションの全トレース（ビルド進捗・テスト進捗・diff 表示）を stderr に流し続けるため、反復の多い fix ラウンドや pre-entry command が決定論的に殺される。briefing で出力を抑制する運用回避は prompt-level の緩い制約であり、遵守下でも再発した。

出力量は暴走の指標にならない。長いセッションほど診断出力が大きくなるのは正常であり、暴走の抑止は既存のタイムアウトが担っている。

## Decision

### D1: 診断出力の収集を末尾リングに変え、出力量による kill を廃止する

診断目的の出力（review-fix runner の stdout / stderr、program_runner の同等面）は、固定容量の末尾リングで保持する。上限到達は打ち切りであって異常ではなく、プロセスは実行を継続する。停止は既存のタイムアウトが担う。保持した内容には切り詰めが起きた旨を明示する。

### D2: 検証対象として読む出力には本決定を適用しない

verdict envelope の抽出のように、内容そのものを検証入力とする経路の上限と fail-closed は不変とする。診断のための記録と、検証のための入力を分けて扱う。

### Existing decision relationship

本 ADR は `2026-08-02-0806-operator-owned-phase-command-config.md` D3（infrastructure が bounded process execution を所有する）と D5（bounded output の検証と上限超過時の停止）を **refines** する。同 D5 が定める config 検証（argv 非空・cwd 固定・timeout・再帰検出）と first-failure stop は変更せず、診断出力に対する「上限超過 = 停止」の扱いのみを改める。検証入力として読む出力の上限は D2 のとおり不変である。

## Rejected Alternatives

- **上限の増額（config 化を含む）**: 「何 MiB なら十分か」に根拠がなく、テスト数の増加で再発する。量を異常の指標として使い続ける点も変わらない。
- **上限到達で収集のみ打ち切り、先頭 N + 末尾 M を保持**: 中間欠落の扱いが複雑になる割に、失敗診断に有用なのは直近文脈であり、末尾リングで足りる。
- **codex 側の出力抑制オプションに依存**: sandbox がパイプ挿入を拒否する制約下で有効性が未確認であり、provider 依存の回避策は本質的な誤りを残す。

## Consequences

- 良: 長いセッションが構造的に完走する。メモリ使用は一定。
- 負: 序盤のログが失われる。長時間の暴走出力プロセスはタイムアウトまで走り続ける。
- 中立: 上限挙動を固定している既存テストの改訂が必要。

## Reassess When

- タイムアウトだけでは抑止できない暴走（短時間での資源枯渇）が観測されたとき。
- 診断出力の全量保持が必要な運用要求が生じたとき（外部ログ収集への委譲を検討する）。
