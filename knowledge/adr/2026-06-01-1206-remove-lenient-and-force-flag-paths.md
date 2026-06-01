---
adr_id: 2026-06-01-1206-remove-lenient-and-force-flag-paths
decisions:
  - id: D1
    user_decision_ref: "chat:2026-06-01:remove-lenient-flag-recommended-option"
    status: proposed
  - id: D2
    user_decision_ref: "chat:2026-06-01:remove-baseline-capture-force-route"
    status: proposed
---
# `--lenient` と `--force` の実行経路を削除する

## Context

TDDD のシグナル系コマンドに、設計上の根拠が曖昧／危険な実行経路フラグが2つ存在する。

(1) `type-signals` の `--lenient`: gate（`track-active-gate`）は `--lenient` を渡してカタログ不在を skip し、ユーザー直接呼び出しは strict のまま、という gate-vs-direct の区別を作っている。一方 `catalogue-spec-signals` はフラグ無しでカタログ不在を無条件 skip する。なお、カタログ不在時の skip 自体は許容された挙動である。再生成コマンド（`type-signals` / `catalogue-spec-signals`）は信号を再計算するためのもので、カタログが存在しない層を skip するのは不完全な入力に対する自然な応答にすぎない。カタログの存在と信号の整合性を強制する厳格な検証は別コマンド `verify catalogue-spec-signals`（`cargo make ci` 経路に含まれる `check-catalogue-spec-signals-local` タスク）が担う。この非対称が問題なのは、呼び出し経路（gate 経由か直接か）で挙動が割れる不整合が生まれることと、`--lenient` フラグと gate-vs-direct 区別が ADR `2026-06-01-0406` D1（「active-gate が入力カタログ不在の層を skip する」のみを決定）に根拠を持たない、spec 段階で後付けされた余分な機構であることにある。

(2) `baseline-capture` の `--force`: 既存 baseline（pre-implementation snapshot）を無条件で上書きできる。実装開始後に再取得すると baseline が現在の実装で汚染され、reverse-signal フィルタの比較基準が壊れる（drift を隠蔽する）危険な経路。

両者とも「余分・危険な実行経路」をフラグとして露出している。これらを撤去し、シグナル評価の挙動を単純で安全な一本道にする。

## Decision

### D1: `type-signals` の `--lenient` 撤去

`type-signals` は views sync / catalogue-spec-signals と同様、カタログ不在を無条件に skip する（gate/直接の区別なし、`--lenient` フラグなし）。これに伴い domain の `MissingCataloguePolicy` enum を削除し、`TypeSignalsExecutorPort::evaluate_layer` から `policy` 引数を削除し、usecase の `TypeSignalsRequest.lenient` を削除する（型契約変更）。カタログが存在する層は引き続き strict に評価し、fail-open を作らない。

### D2: `baseline-capture` の `--force` 撤去

`baseline-capture` は常に冪等とする（既存 baseline は保持）。再取得は「baseline ファイルを削除してから再実行する」運用に倒す。`--force` フラグ、domain `RustdocBaselineCapturePort::capture` の `force` 引数、usecase `BaselineCaptureRequest.force`、infrastructure の上書き分岐、および `force_capture_rustdoc_baseline_for_layer` を削除する（型契約変更）。`--source-workspace`（main の git worktree からの baseline 取得）は維持する。

## Rejected Alternatives

### A. `catalogue-spec-signals` 側にも `--lenient` を追加して対称化する

両コマンドの寛容さの非対称に対する素朴な解法。しかし「余分な機能（フラグ）」をもう一本増やす方向であり、ADR `2026-06-01-0406` が決めていない gate-vs-direct 区別を恒久化する。本決定（撤去）の逆方向。却下。

### B. `--lenient` 漏れを現状のまま受容する

`type-signals` と `catalogue-spec-signals` の呼び出し経路依存の非対称と、ADR に根拠のない `--lenient` フラグの後付けを恒久化する。機構を必要最小限に保つ方針に反する。却下。

### C. `baseline-capture --force` を残し運用注意で対処する

危険な上書き経路をドキュメントだけで防ぐのは脆い（実際に誤用が起きた）。型／CLI レベルで経路自体を消すのが確実。却下。

## Consequences

### Positive

- シグナル評価の挙動が「カタログ不在は常に skip（全コマンド一様）」「baseline は常に冪等」で単純・予測可能になる。
- ADR に根拠のない gate-vs-direct 区別、および危険な baseline 上書き経路が消える。
- ADR に根拠のない `--lenient` フラグと呼び出し経路依存の非対称が除去され、`type-signals` と `catalogue-spec-signals` の両コマンドがカタログ不在を一様に skip するよう統一される。

### Negative

- domain/usecase の公開型（enum・port signature・request 型）の契約変更を伴うため、catalogue に delete/modify として宣言し、baseline・signal を再同期する必要がある。
- baseline 再取得は「ファイル削除 → capture」の2手順になる（`--force` による1コマンド上書きは不可）。

### Neutral

- `--source-workspace`（main worktree からの baseline 取得）は維持され、正規の baseline 運用は不変。

## Reassess When

- gate 以外の呼び出し経路で `type-signals` / `catalogue-spec-signals` の strict 評価が必要になった場合（フラグ再導入ではなく専用経路を設計する）。
- baseline フォーマット移行など、既存 baseline の一括上書きが再び必要になった場合（`--force` 復活ではなく移行専用の手順を検討する）。

## Related

- `knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md` — active-gate のカタログ不在耐性。本 ADR はその gate-vs-direct 区別の後付けを是正する。
- `knowledge/adr/` — ADR 索引
