---
adr_id: 2026-06-19-2335-dry-gate-configurable-default-off
decisions:
  - id: D1
    user_decision_ref: "chat_segment:dry-gate-configurable-default-off:2026-06-20"
    candidate_selection: "evaluated:[A:完全撤去, B:advisory既定3値, C:CLI bypassフラグ, D:opt-out既定ON, E:細粒度] chose:既定OFF/boolean/グローバル"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:dry-gate-configurable-default-off:2026-06-20"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:dry-gate-configurable-default-off:2026-06-20"
    status: proposed
---
# DRY ゲートを利用者設定で切り替え可能にし、既定を無効（opt-in）とする

## Context

現状、DRY ゲート（`sotp dry check-approved`）は無条件必須の blocking gate である。commit メッセージゲート（`track-commit-message`）/ full-cycle の per-task DFP ループ / `fixpoint_resolve` で commit をブロックする（2026-06-02-0716 D7）。有効/無効の設定スイッチは無く、`.harness/config/dry-check.json` は threshold などの tuning のみを持つ。

ワークフロー telemetry の実測（実 DFP が回ったトラックを対象）で、DRY ゲート関連処理が開発時間の約 15% を占めることが分かった。内訳は DRY 処理本体が約 11.5%、加えて DFP⇄RFP オシレーションが誘発する再レビューである。DRY ラウンド 58 回のうち 84%（49 回）が指摘ゼロの空振り再検証で、実際に違反を修正したのは 9 回のみだった。DFP⇄RFP は対称プロトコルだが編集頻度は非対称（レビュー編集 約 120 回 対 DRY 編集 9 回）で、レビューが修正するたびに DRY の空振り再検証が強制される構造になっている。

一方で、embedding モデルや native 依存の整備を前提とする DRY ゲートは、テンプレート利用者や立ち上げ初期のプロジェクトでは前提条件を満たせないことがある。

これらから、DRY ゲートを「無条件必須」から「利用者が選べる設定」へ変える判断が必要になった。

## Decision

### D1: DRY ゲートの実行を利用者設定で切り替え可能にし、既定を無効（OFF）とする

DRY ゲート（`sotp dry check-approved` による commit ブロックと DFP 修正ループ）を、無条件必須から利用者設定で有効/無効を切り替えられるようにする。既定は無効（OFF）とし、ゲートを使いたい利用者が明示的に有効化する opt-in 運用とする。当初は「既定 ON のまま opt-out」を検討したが、上記の実測コスト（開発時間の約 15%・空振り率 84%）から、既定で走らせる便益が薄いと判断し、既定 OFF を選んだ。

### D2: 設定機構は `.harness/config/dry-check.json` の boolean キー `enabled`（グローバル単位）

有効/無効の切り替えは、既存の DRY ゲート設定ファイル `.harness/config/dry-check.json`（現 `schema_version: 3`）に boolean キー `enabled` を追加して表現する。既定は `enabled: false`。`enabled: false` のとき DRY ゲートは走らず（検出も DFP 修正ループも実行しない）、ゲート評価は通過として扱い commit をブロックしない。`enabled: true` のとき従来どおり blocking gate として走る。適用範囲はリポジトリ全体の単一設定とし、トラック単位・違反単位の上書きは設けない。無効化は DRY ゲートの 2 つの評価点 ― commit ゲートの `sotp dry check-approved` と、full-cycle の DFP 起動を判定する `fixpoint_resolve`（`sotp track fixpoint-resolve`）― が `enabled` を読み、`false` のとき「通過 / DFP 不要」を返すことで実現する。これにより commit メッセージゲート（`track-commit-message`）など上位の Makefile 配線は変更しない。キー追加に伴い `dry-check.json` の `schema_version` は 3 から 4 へ上げる（`max_parallelism` を追加した 2026-06-10 D3 と同じ前例）。

<!-- illustrative, non-canonical -->

```json
{
  "schema_version": 4,
  "enabled": false,
  "threshold": "...",
  "max_parallelism": "..."
}
```

### D3: 有効時の blocking 性は維持し、2026-06-02-0716 D7 の「無条件必須」側面のみを部分 supersede する

設定で有効化したときの DRY ゲートは従来どおり blocking であり、個別の違反を場当たり的に握りつぶす抜け道は設けない。すなわち 2026-06-02-0716 D7 の「全 above-threshold ペアの verdict が確定するまで進めない blocking gate」「genuine な違反への人間による許容の抜け道は無い」という性質は、有効時にはそのまま維持する。本 ADR が置き換えるのは D7 の「ゲートは無条件で必須」という側面のみで、ゲートを走らせるか否かが利用者設定になった点において D7 を部分的に supersede する。検出・報告はするがブロックしない advisory 中間状態は設けず、boolean の有効/無効 2 状態に留める。

## Rejected Alternatives

### A. DRY ゲートを完全撤去する

撤去すると discoverability も将来 opt-in する余地も失う。設定で無効化できれば、撤去せずとも「既定で走らせない」目的を達せる。

### B. advisory（報告のみ・非ブロック）を既定とする 3 値モード

blocking / advisory / disabled の 3 状態を持たせ、既定を advisory にする案。検討したが、状態が増えて設定が複雑になり、advisory でも検出パス自体は走るため処理コストが残る。既定 OFF の boolean で目的（既定では走らせずコストも掛けない）を満たせる。

### C. CLI の bypass フラグ（`--skip` / `--force`）

`sotp dry check-approved` に通過用フラグを足す案。2026-06-01-1206 が bypass フラグ（`--lenient` / `--force`）を「余分・危険な実行経路」として撤去済みで、per-invocation の抜け道は方針に逆行する。恒久的なプロジェクト方針は設定ファイルで表現する。

### D. opt-out（既定 ON のまま無効化可能）

当初案。実測コストと空振り率から、既定で走らせる便益が薄いと判断し、既定 OFF（opt-in）を選んだ。

### E. トラック単位 / 違反単位の粒度

グローバルなプロジェクト方針として 1 箇所で表現すれば足り、粒度を増やすと設定機構が複雑になる。

## Consequences

### Positive

- 既定で DRY ゲートが走らないため、DFP⇄RFP オシレーションと空振り再検証（実測で開発時間の約 15%）が既定構成から消える。
- embedding モデル未整備や立ち上げ初期のプロジェクトでも、追加設定なしでワークフローが回る。
- DRY ゲートを使いたいプロジェクトは設定一つで従来の blocking 運用に戻せる。

### Negative

- 既定 OFF のため、明示的に有効化しないプロジェクトでは DRY 違反の自動検出が働かず、重複の混入はレビューや規約など別手段で補う必要がある。
- DRY ゲート前提で書かれた既存ドキュメント・ワークフロー記述（full-cycle / dry-check など）を「設定依存」に書き換える保守コストが生じる。
- 2026-06-02-0716 D7 を部分 supersede するため、ADR 間の整合（相互参照・Follow-up 記述）の追従が必要。

### Neutral

- 有効化したときの挙動（blocking 性・DFP ループ・fixpoint の評価順序）は従来と変わらない。

## Reassess When

- DRY ゲートを有効化したプロジェクトで、オシレーション削減が効いて実測コストが大きく下がり、既定 ON に戻す便益が上回ったとき。
- advisory（報告のみ）モードへの需要が実利用で確認され、3 値設計を再検討する価値が出たとき。
- semantic-dup 検出の精度・コスト特性が改善し、空振り率が大きく下がったとき。

## Related

- `knowledge/adr/2026-06-02-0716-dry-checker.md` — DRY ゲートの基盤設計（本 ADR が D7 の無条件必須側面を部分 supersede）
- `knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md` — DRY ゲートの運用性改善
- `knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md` — DFP⇄RFP コスト削減（本 ADR は別アプローチ＝既定無効化）
- `knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md` — bypass フラグ撤去（CLI フラグ案 C の却下根拠）
- `knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md` — discoverability の系譜
