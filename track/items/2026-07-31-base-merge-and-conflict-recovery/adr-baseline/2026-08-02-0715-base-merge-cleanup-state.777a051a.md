---
adr_id: "2026-08-02-0715-base-merge-cleanup-state"
decisions:
  - id: D1
    user_decision_ref: "chat:2026-08-12-strip-excess-decisions"
    status: accepted
  - id: D2
    user_decision_ref: "chat:2026-08-12-strip-excess-decisions"
    status: accepted
  - id: D3
    user_decision_ref: "chat:2026-08-12-strip-excess-decisions; chat:2026-08-12-baseline-loss-manual-recapture; chat:2026-08-13-conflict-views-parity"
    status: accepted
---
# base merge 後の baseline 再取得と同期記録を定める

## Context

`2026-07-29-0839-base-merge-and-conflict-recovery.md` D1 は競合なしの base merge 後に baseline、表示用ファイル、同期記録を更新すると定めたが、その供給元と順序は未確定だった。

競合した merge でも merge 前の baseline を使うと、取り込んだ base 所有の catalogue 変更を回復側の差分として誤認する。

track 成果物は Git ブランチと一対一に対応するため base merge では競合せず、表示用ファイルは永続済みの信号、baseline、track 成果物のみから生成できる。

## Decision

### D1: baseline は merge が取り込んだ exact base commit から再生成する

baseline はガード付き merge コマンドが実際に取り込んだ base commit から再生成する。

### D2: sync-base stamp は merge が取り込んだ exact base commit を記録する

sync-base stamp はガード付き merge コマンドが実際に取り込んだ base commit を記録する。

### D3: merge 結果ごとに実行する後始末段階を固定する

競合なしの merge の後始末は Baseline → Views → SyncBaseStamp の順で実行する。

表示用ファイルは baseline の確定後に一度だけ生成する。

競合した merge の後始末は Baseline → Views の順で実行し、Baseline の供給源には競合中の作業ツリーではなく、ガード付き merge コマンドが実際に取り込んだ base commit を用いる。

競合した merge の Views は永続済みの信号、baseline、track 成果物のみを読み、競合中のソースコードには依存しない。

SyncBaseStamp は競合なしの merge に限る。

いずれかの段階が失敗した場合は失敗を報告する。

baseline は手動で再取得できるため、失敗からの回復手段は設けない。

### Existing decision relationship

本 ADR の D1〜D3 は `2026-07-29-0839-base-merge-and-conflict-recovery.md` D1 の競合なし merge の後始末を **refines** する。

本 ADR の D3 は同 ADR D2 の競合回復手順を **refines** する。

## Rejected Alternatives

- **後始末の各段階に原子的な置換と自動復元を要求する**: 失敗は検出でき、baseline は手動で再取得できるため採用しない。
- **表示用ファイルを baseline の更新前後に二度生成する**: baseline の確定後に一度生成すれば足りるため採用しない。
- **競合した merge では Baseline だけを実行する**: 表示用ファイルの入力は競合中のソースコードに依存せず、安全に生成できるため採用しない。

## Consequences

- Good: baseline と同期記録が merge の実体である exact base commit を指す。
- Good: 表示用ファイルの生成を一度に減らせる。
- Good: 競合した merge でも新しい baseline に整合する表示用ファイルを生成できる。
- Bad: 後始末の失敗後は運用者が baseline を手動で再取得する。

## Reassess When

- baseline の手動再取得が頻発し、自動回復の費用に見合う場合。
- track の同期状態を単一の base commit では表現できなくなった場合。

## Related

- `knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md` D1 — D1〜D3 の refines 対象。
- `knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md` D2 — D3 の競合回復手順に関する refines 対象。
