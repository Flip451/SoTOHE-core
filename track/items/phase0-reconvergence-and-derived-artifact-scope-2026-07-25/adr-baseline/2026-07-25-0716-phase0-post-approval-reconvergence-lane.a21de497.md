---
adr_id: "2026-07-25-0716-phase0-post-approval-reconvergence-lane"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session_011jERZasQLh9DN6GzqkeZdw:2026-07-25:phase0-post-approval-adjudication"
    status: proposed
---
# Phase 0 承認後に修正が入った場合は承認前へ戻して再収束する

## Context

`knowledge/conventions/pre-track-adr-authoring.md` の Phase 0 手順 3 は、user が収束文面を承認したのち、adr-editor が承認 `user_decision_ref` を対象 decision の front-matter へ適用し、adr-diagnoser が再監査し、fresh review で current hash を再収束させることを要求している。
承認記録を ADR 自身の front-matter が担うことは `knowledge/adr/2026-07-16-2001-adr-decision-freeze.md` D6 / D7 の決定である。

この front-matter への適用は ADR の byte を動かし、adr scope の hash を失効させる。
失効した scope の再収束ラウンドは、差分だけではなく scope 全体を再審査する。
その結果、承認以降 1 文字も変わっていない本文に対して、直前のラウンドが通した箇所の新規 finding が出うる。
reviewer はラウンド間で非決定的であり、これは異常ではなく想定される挙動である。

現行の手順 3 は、この再収束で「user 裁定済み文面の意味変更が必要な finding が出た場合は自己解決せず user 裁定へ戻す」と定めている。
そのため reviewer の揺れがそのまま finding 単位の user 裁定差し戻しになりうる。
2026-07-25 の運転では、承認直後の再収束ラウンドが承認済み本文へ P1 を 2 件出し、規範どおり user 裁定へ戻された。
差し戻された内容は決定の変更提案ではなく、直前のラウンドが通した記述への粒度の指摘であった。

この経路は原理的に往復しうる。
承認 → byte 変化 → 全文再審査 → 揺れによる finding → user 裁定 → 承認、という循環に上限がない。

一方、承認前の in-place 収束ループ（手順 2）は、同種の finding を機構で処理している。
adr-editor が適用し、adr-diagnoser が決定保存を判定し、決定を壊す提案には保全代案または修正不要理由の提示を義務づけ、それをレビュアーへ還流する。
user へ載せるのは、レビュアーが所見を維持して対立が解消しない場合だけである。

## Decision

### D1: 承認後に修正が入った場合は承認前へ戻して再収束する

Phase 0 で user が収束文面を承認した後、その文面に修正が入る場合は、承認済みの状態を維持したまま通常のガーディアン・レーンを継続しない。
承認前の収束段階へ戻し、手順 2 と同じ review loop を再開する。

再開した review loop で findings を収束させた後、修正後の収束文面を user へ再度提示して承認を得る。
修正が意味変更か、または守護者が決定保存的と判定したかを問わず、修正前の承認を修正後の文面へ流用しない。

具体的な収束手順と Phase 0 境界処理は `knowledge/conventions/pre-track-adr-authoring.md` を規範の正とし、workflow SSoT は同 convention に委ねる。

## Rejected Alternatives

### A. 承認記録を front-matter から ledger 刻印へ移す

承認後に front-matter を触らなければ byte が動かず、再審査ラウンド自体が発生しない。
しかし `2026-07-16-2001-adr-decision-freeze.md` D6 / D7 が承認記録の所在を front-matter と定め、ledger 側に承認記録を重複して持たないとしている。
この案はその決定を覆すため、採用しない。

### B. 再収束ラウンドを差分限定の審査にする

承認後のラウンドが変更された front-matter 1 行だけを見るようにする案。
scope hash とレビュー記録の単位は scope 全体であり、差分限定の審査は「その scope が承認済みである」という記録の意味を変える。
機構全体の前提に触れるため、採用しない。

### C. 承認後の再収束ラウンド自体を省略する

承認時点で zero_findings に達しているのだから、front-matter 適用後の再審査は不要とする案。
承認済み文面の hash に対する review 記録が存在しないまま staging へ進むことになり、commit gate の `check-approved` が前提とする不変条件を壊す。
採用しない。

### D. 現行のまま維持する

承認後の修正を承認済みの状態のまま通常のガーディアン・レーンで処理し、守護者判定だけで修正後の文面を採用しうる。
これは Phase 0 の init 文面への変更を commit 直前に user が審査するという裁定境界を満たさないため、採用しない。

## Consequences

### Positive

- 承認後の修正を finding 単位で user へ戻さず、review loop で収束させてから修正後の全体を提示できる。
- init 文面から変更された収束文面は、守護者判定だけで採用されず、commit 直前に必ず user の審査を通る。

### Negative

- 承認後に修正が入るたびに承認前へ戻るため、review loop と user の再審査が必要になる。
- 再審査後にも修正が入れば同じ手順を繰り返すため、往復の可能性が完全に消えるわけではない。

### Neutral

- 承認記録の所在、境界刻印、commit gate の byte 照合はいずれも変更しない。
- `adjudication-ready` 経路の扱いは変更しない。

## Reassess When

- 承認後の修正による review loop と user 再審査の反復が、実務上の負担になったとき。
- 収束後の user 再審査を経ずに、init 文面から変更された文面が commit へ到達したとき。
- review-refinement kind の実装により境界刻印の記録内容が変わったとき。

## Related

- `knowledge/adr/2026-07-16-2001-adr-decision-freeze.md`
- `knowledge/adr/2026-07-17-1203-adr-baseline-review-gate-init-existence-only.md`
- `knowledge/conventions/pre-track-adr-authoring.md`
- `.harness/workflows/track/plan.md`
- `.harness/workflows/track/adr2pr.md`
