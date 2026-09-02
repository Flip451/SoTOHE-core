---
adr_id: "2026-09-02-0000-evaluation-start-capture-time-bounds"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:grok-session-2026-09-02:merge-stage-adoption-1803-D3-D8-and-2026-09-02-0000-D1"
    status: accepted
---
# 評価開始時の authoritative input 捕捉に時間上限を設ける

## Context

`2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md` D4 は、実装 fingerprint を作成する I/O に件数及び bytes の上限を定める。一方で、workspace walk、nightly 選択済み tool の解決、`cargo metadata --no-deps --locked`、及びそれらの出力 drain が停止しない場合の実行時間は定めていない。D6 の 120 秒は rustdoc export と専用出力 directory の lock にだけ適用されるため、評価開始時に authoritative input を捕捉する処理へは適用されない。

この処理が無制限に待機すると、評価は authoritative input を確定できないまま停止する。時間超過時に部分的な fingerprint、以前の snapshot、又は推測した層帰属を用いると、D3 と D4 が要求する fail-closed な入力同一性を損なう。

## Decision

### D1: 評価開始時の capture I/O は実行 120 秒・drain 1 秒で fail-closed にする

`EvaluationStartCapturePort` による authoritative input の捕捉では、workspace walk、nightly 選択済み tool の解決、`cargo metadata --no-deps --locked`、及びこれらに伴う出力 drain に、実行時間 120 秒と drain 時間 1 秒の上限を適用する。

いずれかの上限を超えた場合は `EvaluationStartCaptureError::AuthoritativeInput` として fail-closed で失敗させる。fallback、partial fingerprint、又は生成した層帰属によって処理を続行してはならない。

この決定は、rustdoc export 又は専用出力 directory の lock に適用しない。それらは `2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md` D6 の 120 秒上限に従う。

## Rejected Alternatives

- **A: D4 の件数・bytes 上限だけに委ねる**: 停止した I/O 又は終了しない子プロセスを検出できず、評価開始時の capture が無期限に待機する。
- **B: D6 の rustdoc export・lock 上限を capture I/O に流用する**: 対象となる操作と失敗分類を混同し、D6 が定める専用出力 directory の協調 writer 契約を不必要に拡張する。
- **C: 超過時に以前の fingerprint 又は部分的な結果を使う**: authoritative input を確認できない評価を成功扱いにし、結果の入力同一性を保証できない。

## Consequences

- 良: 評価開始時の I/O 停止は有限時間で明確に失敗し、評価処理が無期限に占有されない。
- 良: capture に失敗した結果が再利用又は層帰属の根拠になることを防ぐ。
- 負: 一時的に遅い workspace 又は toolchain 環境では、利用者が環境を回復するまで評価が失敗する。

## Reassess When

- 正当な workspace で capture I/O が継続的に上限を超え、測定値に基づく調整が必要になったとき。
- 各 capture 操作について、より狭い authoritative-input 失敗分類又は中断機構を導入できたとき。

## Related

- `knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md` D3 / D4 — 本 ADR は実装 fingerprint の authoritative input 捕捉に時間上限を補う。
- `knowledge/adr/2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md` D6 — rustdoc export と lock の時間上限は同決定に残す。
