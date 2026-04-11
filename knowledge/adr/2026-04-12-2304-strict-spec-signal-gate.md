# Strict Spec Signal Gate — Yellow がマージをブロックする

## Status

Accepted

## Context

SoTOHE-core は 3 段階の信頼度シグナル (Blue/Yellow/Red) で仕様要件の根拠を評価する。本 ADR 以前、シグナルゲートは CI で Red (根拠なし) のみをブロックし、Yellow (inference/discussion) はマージを含む全段階で許容されていた。

### 問題 1: マージ時の Yellow 許容

Yellow は「推定」「議論」などの非永続的な根拠を示す。CI は Red のみをブロックするため、Yellow の要件がそのまま main にマージされ、設計判断の根拠がどこにも永続化されない状態が許容されていた。

### 問題 2: feedback が Blue にマッピングされていた

本 ADR の D1 (マージゲート追加) だけでは不十分であった。`feedback` ソースタイプは `document` や `convention` と同じ Blue にマッピングされていたが、永続的なファイル参照を持たない。そのため、Yellow の項目を `feedback — 承認済み` と書き換えるだけで Blue に昇格でき、マージゲートをバイパスできた。

### Fail-closed 原則

SoTOHE-core は全ゲートで fail-closed を設計原則とする:
- Hook エラー → ブロック (ADR 2026-03-11-0050)
- review.json 読取不能 → bypass 不可
- Baseline 不在 → signal 評価エラー
- マージゲートも同原則に従うべき

## Decision

### D1: Yellow がマージをブロックする

`wait-and-merge` は PR head ref から `spec.json` を `git show` で読み取り、以下の条件でマージをブロックする:

- `signals` が存在しない → BLOCKED (未評価)
- `signals.red > 0` → BLOCKED (根拠なし)
- `signals.yellow > 0` → BLOCKED (根拠が非永続的)

`signals.yellow == 0 && signals.red == 0` (全 Blue) のみマージを許可する。

これにより、マージ前に設計判断を ADR や convention として記録する構造的インセンティブが生まれる:
- `inference` / `discussion` / `feedback` → Yellow → マージブロック
- ADR を書いて `document` ソースとして参照 → Blue → マージ許可

### D2: feedback を Yellow に降格

`SignalBasis::Feedback` を `ConfidenceSignal::Blue` から `ConfidenceSignal::Yellow` に再マッピングする。

Blue ソースは永続的なファイル参照を持つもののみ:
- `document` → ファイル参照あり (ADR, spec, PRD)
- `convention` → convention ファイル参照あり

Yellow ソースは永続的な記録を持たない:
- `feedback` → 「ユーザーが言った」(ファイルなし)
- `inference` → 「推定」(ファイルなし)
- `discussion` → 「合意した」(ファイルなし)

`feedback` を Blue に戻すには、決定内容を ADR か convention ドキュメントに記録し、`document` ソースとして参照する必要がある。

### D3: spec.json 必須 (fail-closed)

`/track:plan` で作成される全ての新規 track は `spec.json` を含む。`spec.json` が存在しない場合はレガシー track であるが、完了済みで再マージされることはない。`git show` で `spec.json` が見つからない場合、マージをブロックする。

### D4: check_tasks_resolved と同じパターン

実装は既存のタスク完了ガードと同じ `git show origin/branch:path → decode → check` パターンを使用し、ローカルではなくリモートの状態を検証する。

## Rejected Alternatives

### A. verify_from_spec_json を temp ファイル経由で呼び出す

リモートの `spec.json` を temp ファイルに書き出し、`verify_from_spec_json(path, strict=true)` を呼ぶ方式。

却下理由:
- temp ファイルの管理が必要 (作成・削除)
- `verify_from_spec_json` は sibling の `domain-types.json` も読むが、リモート ref からは利用不可
- `git show → decode → signals チェック` のシンプルなパターンで十分

### B. feedback を Blue のまま維持

既存の Blue マッピングを維持する方式。

却下理由:
- strict gate の trivial なバイパスを提供する
- 永続的なアーティファクトが作成されない
- 設計根拠の文書化を要求する目的を損なう

### C. spec.json 不在時にゲートをスキップ

`spec.json` が見つからない場合に SUCCESS を返す方式。

却下理由:
- fail-closed 原則に違反する
- 全ての新規 track は `spec.json` を含む
- レガシー track は完了済みで再マージされない

## Consequences

### Good

- **ADR 作成が構造的に促進される**: Yellow → Blue の昇格には永続的なドキュメントの作成が必要。設計判断がマージ前に自然に記録される
- **Fail-closed**: 未評価・根拠不十分な spec はマージをブロックする
- **既存ゲートと一貫性**: タスク完了ガードと同じパターン
- **Stage 1 + Stage 2 の両方をチェック**: マージゲートは spec signals (Stage 1) と domain type signals (Stage 2) の両方で Yellow/Red をブロックする。domain-types.json が存在しない track (TDDD 未使用) では Stage 2 をスキップする

### Bad

- **小規模変更の摩擦増加**: trivial な機能でも spec ソースにファイル参照が必要。5 行の変更に ADR を書くのは不釣り合いに感じる場合がある
- **Race condition**: ガードはポーリングループの前に 1 回実行され、マージ時には再検証されない。ポーリング中の push でバイパス可能。SEC-10 として TODO.md に記録済み

## Reassess When

- 小規模変更の摩擦を軽減する「micro-track」ワークフローが導入された場合
- `feedback` を Blue に再昇格する必要がある場合 (永続的アーティファクトの要件付き)
