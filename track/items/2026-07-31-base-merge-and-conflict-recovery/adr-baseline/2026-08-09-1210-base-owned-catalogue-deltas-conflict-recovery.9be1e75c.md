---
adr_id: "2026-08-09-1210-base-owned-catalogue-deltas-conflict-recovery"
decisions:
  - id: D1
    review_finding_ref: "diagnosis:rollback-diagnoser:2026-08-09-recovery-baseline"
    status: proposed
---
# conflict 回復中に base から取り込んだ catalogue 変更は、実際に merge した base の baseline で照合する

## Context

`2026-07-29-0839-base-merge-and-conflict-recovery.md` D2 は、guarded base merge が conflict した場合に、統括役が conflict の解消から指摘がなくなるまでの review、guarded commit までを進めると定めた。`2026-08-02-0715-base-merge-cleanup-state.md` D3 は、conflict になった場合は clean-merge cleanup を実行しないと定めた。

一方、実際に merge した base commit から取り込まれた catalogue entry が、取り込み元の merge 済み track ですでに所有と帰属を持ち、Chain ③ が青信号であっても、回復側の型 baseline が merge 前の base を比較権威としている間は赤信号の差分と評価される。これらを回復側の catalogue に複製すると所有が二重化し、merge 統合作業への帰属だけでは比較権威のずれを解消できない。

## Decision

### D1: 影響する信号連鎖の review 後に、実際に merge した base commit から型 baseline を再取得する

conflict 後の回復では、影響する信号連鎖に限定した review が指摘なしで完了した後、実際に merge した base commit の使い捨ての複製に対する正規の baseline 取得コマンドにより、型 baseline を現在の base に合わせ直してよい。

これは clean-merge cleanup ではなく、`.sync-base.json` の更新、保持用作業領域の公開、表示用ファイルの再生成を含まない。

取り込んだ entry の所有は取り込み元の merge 済み track に残し、回復を実行する track は帰属の網羅性を満たすための merge 統合作業への帰属だけを記録して、同じ catalogue entry を複製しない。

### Existing decision relationship

本 ADR の D1 は `2026-07-29-0839-base-merge-and-conflict-recovery.md` D2 を **refines** し、conflict 後の回復手順に、review 後に baseline を現在の base に合わせ直す手順を追加する。

同時に `2026-08-02-0715-base-merge-cleanup-state.md` D3 を **refines** し、同判断の clean-merge cleanup 禁止が、ここで定める限定的な baseline 再取得を禁止しないことを明確にする。

## Rejected Alternatives

- **base から取り込んだ entry を由来に応じて信号と通過判定で特別扱いする**: Chain ③ の接地判定を二重化する重い変更となるため却下する。
- **取り込んだ entry を回復側の catalogue に複製する**: 取り込み元ですでに成立している所有と帰属を二重化するため却下する。

## Consequences

- 良: 実際に merge した base commit 由来の承認済み catalogue 変更と、回復側で新たに生じた catalogue の差分を、正しい比較権威に基づいて区別できる。
- 良: clean-merge cleanup 禁止と、conflict 後の回復を guarded commit まで進める要求を両立できる。
- 良: 取り込んだ entry の所有を取り込み元に保ち、catalogue の重複を避けられる。
- 負: conflict 後の回復に、影響する信号連鎖に限定した review 後の baseline 再取得が追加される。

## Reassess When

- Chain ③ が base から取り込んだ entry の由来を、接地判定を二重化せず直接評価できるようになったとき。
- 型 baseline を再取得せず、実際に merge した base commit から同じ比較権威を再構成できるようになったとき。

## Related

- `knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md` D2 — conflict 後の回復手順を精緻化する対象。
- `knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md` D3 — clean-merge cleanup 禁止を精緻化する対象。
