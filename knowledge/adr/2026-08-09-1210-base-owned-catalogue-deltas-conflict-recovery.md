---
adr_id: "2026-08-09-1210-base-owned-catalogue-deltas-conflict-recovery"
decisions:
  - id: D1
    review_finding_ref: "diagnosis:rollback-diagnoser:2026-08-09-recovery-baseline"
    status: proposed
---
# conflict recovery 中の base-owned catalogue delta は exact merged base baseline で照合する

## Context

`2026-07-29-0839-base-merge-and-conflict-recovery.md` D2 は、guarded base merge が conflict した場合に、conflict 解消から zero-findings review、guarded commit までをオーケストレーターが駆動すると定めた。`2026-08-02-0715-base-merge-cleanup-state.md` D3 は、conflict outcome では clean-merge cleanup を実行しないと定めた。

一方、exact merged base commit から取り込まれた catalogue entry が、取り込み元の merged track ですでに ownership と attribution を持ち、Chain ③ が Blue であっても、回復側の type baseline が merge 前の base を比較権威としている間は Red drift と評価される。これらを回復側の catalogue に複製すると ownership が二重化し、merge-integration task への attribution だけでは比較権威のずれを解消できない。

## Decision

### D1: chain-limited review 後に exact merged base commit から type baseline を再取得する

conflicted guarded base merge の conflict hunks を解消し、影響を受けた chain が chain-limited zero-findings review を通過した後、exact merged base commit の disposable clone を source workspace として、sanctioned な `--source-workspace` baseline-capture surface から type baseline を再取得する。

再取得した `<layer>-types-baseline.json` は exact merged base commit を新しい比較権威とする。base-owned catalogue delta とは、MERGE_HEAD blob の entry と source-identical であり、取り込み元の merged track の catalogue と task-contract ですでに ownership と attribution を持ち、Chain ③ が Blue の entry である。この比較権威の更新により、base-owned catalogue delta を回復側固有の Red drift として扱わない。

この再取得は clean-merge cleanup ではない。`.sync-base.json` stamp の更新、retained tree への publish、derived view の再生成を含めず、`2026-08-02-0715-base-merge-cleanup-state.md` D3 の conflict outcome に対する cleanup 禁止を維持する。また、再取得は前述の review 通過後に sanctioned baseline-capture surface だけを通じて行う。

incoming entry の ownership は取り込み元の merged track の catalogue と task-contract に残す。回復を実行する track は coverage completeness のための merge-integration attribution だけを記録し、同じ catalogue entry を複製しない。

### Existing decision relationship

本 ADR の D1 は `2026-07-29-0839-base-merge-and-conflict-recovery.md` D2 を **refines** し、conflict recovery sequence に review 後の baseline-currency step を追加する。

同時に `2026-08-02-0715-base-merge-cleanup-state.md` D3 を **refines** し、同 decision の cleanup 禁止が Views、Baseline、SyncBaseStamp からなる clean-merge cleanup bundle を対象とし、ここで定める限定的な baseline recapture を禁止しないことを明確にする。

## Rejected Alternatives

- **base-owned entry を provenance-aware な signal/gate 分岐で特別扱いする**: Chain ③ に第 2 の grounding semantics を追加する重い変更となるため却下する。
- **incoming entry を回復側の catalogue に複製する**: 取り込み元ですでに成立している ownership と attribution を二重化するため却下する。

## Consequences

- Good: exact merged base commit 由来の承認済み catalogue delta と、回復側で新たに生じた catalogue drift を、正しい比較権威に基づいて区別できる。
- Good: clean-merge cleanup 禁止と、conflict recovery を guarded commit まで進める要求を両立できる。
- Good: incoming entry の ownership を取り込み元に保ち、catalogue の重複を避けられる。
- Bad: conflict recovery に、chain-limited review 後の限定的な baseline recapture が追加される。

## Reassess When

- Chain ③ が base-owned entry の provenance を単一の grounding semantics のまま直接評価できるようになったとき。
- type baseline を recapture せず、exact merged base commit から同じ比較権威を再構成できるようになったとき。

## Related

- `knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md` D2 — conflict recovery sequence の refines 対象。
- `knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md` D3 — clean-merge cleanup 禁止の refines 対象。
