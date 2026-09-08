---
adr_id: "2026-09-04-0058-catalogue-impl-signals-absolute-workspace-root"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:claude-session-01MMHqwkDTNYynXPTiZKqcg5:2026-09-04 Phase 0 boundary approval of the converged D1 text (catalogue-impl-signals workspace root is normalized to an absolute path before target exclusion)"
    status: proposed
---
# catalogue-impl-signals の workspace root は絶対パスに解決してから使う

## Context

`bin/sotp track catalogue-impl-signals` の既定の workspace root は相対パスで、入力コーパスの除外判定で `./target` と cargo が報告する絶対 target directory が一致せず、`target/` 配下が走査対象に入る。結果として入力コーパス上限(512 MiB)の超過や `target/` 内 symlink による fail-closed 停止が起きる。`--workspace-root "$PWD"` を明示すれば回避できるが、既定値で失敗するコマンドは既定値が誤っている。

## Decision

### D1: workspace root は起動時に絶対パスへ正規化し、除外判定は正規化後のパスで行う

既定・明示いずれの workspace root も、起動時にカレントディレクトリ基準で絶対パスへ正規化する。target directory の除外判定(および同種のパス比較)は正規化後の絶対パス同士で行う。正規化できない(存在しない・権限がない)場合は fail-closed で停止する。回帰テストとして、相対 root 指定で `target/` が除外されることを固定する。

## Rejected Alternatives

- **文書で `--workspace-root "$PWD"` の指定を必須と案内する**: 既定値の欠陥を利用者の手順に転嫁する。
- **除外判定側で相対・絶対の両形を受理する**: 比較箇所ごとに正規化を重複させ、次の比較箇所で同じ漏れが再発する。

## Consequences

- 良: 既定の呼び出しで target 除外が正しく働き、consumer の回避手順が不要になる。
- 中立: パス正規化は起動時 1 回。

## Reassess When

- workspace root を複数取る(multi-root)呼び出しが必要になったとき。
