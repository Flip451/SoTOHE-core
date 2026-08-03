---
adr_id: "2026-08-02-0715-base-merge-cleanup-state"
decisions:
  - id: D1
    user_decision_ref: "handoff:tmp/handoff/2026-08-02-lane-d-delta-adjudication.md#裁定1:admit; chat:2026-08-02 ユーザー確認「OK。進めて。」"
    status: accepted
  - id: D2
    user_decision_ref: "handoff:tmp/handoff/2026-08-02-lane-d-delta-adjudication.md#裁定1:admit; chat:2026-08-02 ユーザー確認「OK。進めて。」"
    status: accepted
  - id: D3
    user_decision_ref: "handoff:tmp/handoff/2026-08-02-lane-d-delta-adjudication.md#裁定1:admit; chat:2026-08-02 ユーザー確認「OK。進めて。」"
    status: accepted
---
# clean な base merge 後の baseline 再取得と同期状態の原子的 lifecycle を定める

## Context

`2026-07-29-0839-base-merge-and-conflict-recovery.md` D1 は、clean な base merge の後始末を
derived views の再生成、TDDD baseline の再取得、sync-base stamp の順に完了させると決めた。
しかし、既存 baseline は first-write-wins であり、再取得時に旧 baseline を安全に置き換える方法と、
失敗時に有効な旧状態を維持する方法は決められていなかった。

また、sync-base stamp についても、何を権威ある同期値とするか、どこへどの schema で保存するか、
再実行および後続 merge でどう更新するかが未確定だった。この状態で後始末を実装すると、baseline の
先行削除による回復不能状態や、branch 名の再解決による merge 対象との identity drift を実装側が独自に
決めることになる。

## Decision

### D1: baseline は固定済み base commit の隔離 worktree で完全生成してから原子的に置き換える

baseline の再取得元には、guarded merge の方向検査で既に解決された権威ある base commit に固定した、
一時的かつ隔離された git worktree を用いる。後から branch 名を再解決して取得元を決め直さない。
作業退避に `git stash` は用いない。

置換対象となる完全な baseline を、現行 baseline の sibling 一時領域へ先に生成し、必要な全要素を
検証する。生成と検証が成功した後にだけ、完成した replacement を原子的に publish する。replacement が
準備できる前に現行 baseline を削除してはならない。生成、検証、置換のいずれかが失敗した場合は現行
baseline を維持し、置換処理の途中で旧状態の復元が必要になった場合はそれを復元する。復元自体の失敗も
成功として扱わず fail-closed とする。

### D2: sync-base stamp は merge が取り込んだ exact base commit を原子的に記録する

sync-base stamp は、active track の gitignored な運用状態として
`track/items/<track-id>/.sync-base.json` に置く。schema は少なくとも次の field を持つ。

- `schema_version`
- `track_id`
- `base_branch`
- `base_commit`

`base_commit` は guarded merge が実際に取り込んだ exact commit identity であり、同期状態の権威値とする。
stamp の書き込み時点で branch 名を再解決した値に置き換えてはならない。同じ値での再実行は idempotent
とし、後続の成功した base merge は、その merge が取り込んだ新しい exact base commit を持つ stamp へ
原子的に置き換える。stamp の生成、検証、書き込み、置換の失敗は fail-closed とする。

### D3: clean merge の後始末は Views → Baseline → SyncBaseStamp の順でのみ完了する

clean merge の後始末は Views → Baseline → SyncBaseStamp の順を維持する。sync-base stamp は Views と
Baseline の双方が成功した後にだけ書き込む。いずれかの段階が失敗した場合、merge 操作は後始末を含む
完了を報告してはならない。conflict outcome では、この clean-merge cleanup を実行しない。

baseline の置換は type-signals cache の削除を含まない。active track の type-signals freshness は、
`2026-07-29-0839-base-merge-and-conflict-recovery.md` D4 が定めた baseline hash による self-healing に従う。

### Existing decision relationship

本 ADR の D1〜D3 は `2026-07-29-0839-base-merge-and-conflict-recovery.md` D1 の clean-merge cleanup を
**refines** する。同 ADR D4 の baseline-hash freshness decision は変更しない。

## Rejected Alternatives

- **現行 baseline を先に削除してから再取得する**: 生成または検証の失敗時に、有効だった旧 baseline まで
  失われるため却下する。
- **現行 baseline を in-place で段階的に書き換える**: 部分生成物が権威状態として観測され得るため
  却下する。
- **stamp 書き込み時に base branch の現在値を再解決する**: merge が実際に取り込んだ commit と stamp が
  指す commit の間に drift を生じ得るため却下する。
- **baseline 再取得のために `git stash` を使う**: 隔離 worktree で現在の作業状態に触れずに生成でき、
  stash の追加 lifecycle を持ち込む必要がないため却下する。
- **baseline 置換時に type-signals cache を削除する**: freshness は baseline hash を含む権威入力の照合と
  成功後の原子的 cache 更新で自己回復するため、baseline lifecycle の責務には含めない。

## Consequences

- Good: baseline 再取得に失敗しても、既存の有効な baseline を先に失わない。
- Good: stamp が merge の実体である exact base commit を保持し、branch 名の移動と同期 identity を分離できる。
- Good: 後始末の途中失敗を完了として報告せず、baseline と stamp の部分更新を fail-closed にできる。
- Bad: 隔離 worktree と sibling replacement の生成・検証・原子的 publish、および失敗時復元の管理が必要に
  なる。
- Bad: stamp schema の将来変更では、schema version と decode failure の扱いを明示的に維持する必要がある。

## Reassess When

- baseline が immutable content-addressed storage へ移り、置換や復元そのものが不要になったとき。
- git worktree を使わず、同じ commit-pinned isolation を保証する正規の取得機構が導入されたとき。
- track の同期状態を単一 base commit では表現できない branch strategy が導入されたとき。

## Related

- `knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md` D1 — D1〜D3 の refines 対象。
- `knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md` D4 — type-signals cache を
  baseline 置換から分離する baseline-hash self-healing decision。
