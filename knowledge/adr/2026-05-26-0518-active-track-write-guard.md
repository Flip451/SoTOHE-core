---
adr_id: 2026-05-26-0518-active-track-write-guard
decisions:
  - id: D1
    user_decision_ref: "chat_segment:adr2pr-unresolved-issue-1:active-track-write-guard:2026-05-26"
    status: proposed
---
# 完了済みトラックのアーティファクト保護を frozen ブロックから「現在ブランチに紐づくトラックのみ書き込み許可」へ置き換える

## Context

track の status が done になると、catalogue-spec-signals / type-signals などの宣言系アクションが「Completed tracks are frozen」として拒否される（catalogue active-track guard `2026-04-15-1012-catalogue-active-guard-fix.md` 由来）。これは完了済みトラックのアーティファクトを誤更新から守る目的で導入された。

しかし 2026-05-26 の `typestate-struct-kind-orthogonal-2026-05-26` トラックの adr2pr 実走で、この frozen ブロックが full-cycle の commit を詰まらせる構造的な問題が明らかになった:

- full-cycle の implementer が commit 前に全 task を done マークすると track status が done になる。
- その後の review 過程で catalogue 修正（StructKind の new() 宣言追加）が必要になったが、track が done のため catalogue-spec-signals の再生成が frozen で拒否され、signals.json が古い状態のまま CI の verify-catalogue-spec-refs が FAIL した。
- track-commit-message の pre-commit でも「Pre-commit type signals: skipped (track is done — frozen)」となり、done トラックでは signal 再計算がバイパスされる。

frozen ブロックは「status=done」という状態フィールドを判定基準にしているが、次の 2 つの弱点を持つ:

1. 判定基準が脆い: status は task 群から導出されるため、full-cycle の途中（commit 前の done マーク）でも done になり、まだ作業中のトラックを誤って frozen 扱いする。
2. 回避が手作業: 現状は task を一時 in_progress に戻して active 化 → 再生成 → done に復元、という手作業の回避を強いられる。

保護したい本来の対象は「すでに完了して別の作業対象になっている、現在のブランチに紐づかないトラック」である。

## Decision

### D1: frozen ブロック機構を削除し、「現在のブランチに紐づくトラックのアクションのみ許容する」バリデーションに置き換える

完了済みトラックのアーティファクトを保護する機構を、track status（done か否か）ベースの frozen ブロックから、現在の git ブランチに紐づくトラックかどうかを判定基準とするバリデーションに置き換える。

- catalogue-spec-signals / type-signals / sync-views などアーティファクトを書き換えるアクションは、対象トラックが現在のブランチ（`track/<id>`）に紐づく場合のみ許容する。
- それ以外のトラック（現在のブランチに紐づかない別トラック・完了済みトラック）への書き込みは拒否する。
- これにより、status が done であっても現在のブランチが当該トラックブランチである限りアクションは通る（full-cycle 途中の done マークでも詰まらない）。一方、現在のブランチに紐づかない完了済みトラックのアーティファクトは引き続き保護される。

判定基準を「status」から「現在ブランチとの紐付き」に移すことで、保護の本来の意図（現在の作業対象でないトラックを守る）を、状態フィールドの脆さなしに表現する。

## Rejected Alternatives

### A. frozen を残したまま done 判定に例外条件を追加する

frozen ブロックを維持しつつ、「uncommitted な done（commit されていない done トラック）では再生成を許可する」のような例外条件を done 判定に追加する案。

却下理由: status ベースの判定に条件分岐を重ねるだけで、判定の脆さ（status が作業途中でも done になる）の根本は解決しない。「uncommitted done」という新たな状態区別の導入で判定ロジックがさらに複雑化し、別のエッジケースを生む。保護の意図を status で近似し続ける限りずれが残る。

### B. full-cycle の done マークを commit 後に遅延させる

full-cycle が task を done マークするタイミングを commit 後（implement → review → commit → done）に変更し、commit 前は done にしない案。

却下理由: done マークのタイミング変更は full-cycle 途中で詰まる症状を緩和するが、「完了済みトラックのアーティファクト保護」という frozen 本来の目的の代替にはならない（保護機構そのものは別途必要）。また review 過程で commit 後にさらに catalogue 修正が必要になるケースでは、commit 後の done でも同じ問題が再発する。タイミング調整は対症療法に留まる。

## Consequences

### Positive

- full-cycle 途中（commit 前の done マーク）でも、現在のブランチが当該トラックブランチである限りアーティファクト更新が通り、手作業での active 化回避が不要になる。
- 保護の判定基準が「状態フィールド（status=done）」から「現在ブランチとの紐付き」という明確で誤判定しにくい基準に変わる。
- 形骸化しやすい状態フィールドに依存しない方針（workflow-ceremony-minimization）に沿う。

### Negative

- branch ベースのバリデーションを、アーティファクトを書き換える全アクション（catalogue-spec-signals / type-signals / sync-views など）に一貫して適用する実装作業が発生する。
- 既存の catalogue active-track guard（`2026-04-15-1012`）との関係を整理し、frozen ブロック判定を branch ベースに置き換える必要がある。

## Reassess When

- ブランチに紐づかない文脈でのトラック操作が必要になった場合（CI 環境での detached HEAD、ブランチ外からのバッチ処理など）。branch 紐付きを唯一の判定基準にすると正当な操作も拒否されるため、判定基準の拡張を検討する。
- 複数トラックを同時に操作する必要が出た場合（現在ブランチ = 単一トラックという前提が崩れる）。

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/adr/2026-04-15-1012-catalogue-active-guard-fix.md` — catalogue active-track guard の起源。本 ADR はその frozen ブロック判定を branch ベースに置き換える。
- `knowledge/conventions/workflow-ceremony-minimization.md` — 状態フィールド依存の最小化原則。
- `knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D7` — explicit `--track-id` の read/write 分岐を決定。WRITE 操作では本 ADR のブランチ照合バリデーションを全コマンドへ一般化し、escape hatch を撤廃する。READ 操作には従来どおり override を許可する。
