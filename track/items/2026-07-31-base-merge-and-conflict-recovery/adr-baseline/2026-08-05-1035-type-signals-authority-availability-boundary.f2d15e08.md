---
adr_id: "2026-08-05-1035-type-signals-authority-availability-boundary"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:authority-availability-boundary-directive:2026-08-05"
    review_finding_ref: "diagnosis:rollback-diagnoser:2026-08-09"
    status: proposed
---
# type-signals の権威照合は権威が存在する文脈に限定する

## Context

型シグナルは catalogue 宣言と実装の一致判定結果であり、その cache は 3 つの照合値がすべて一致する
場合にのみ再利用される。3 値は、catalogue 宣言の要約 hash（catalogue declaration hash）、実装側の
compile input の要約 hash（`implementation_input_hash`）、rustdoc baseline の要約 hash（baseline hash）
である。`2026-07-29-0839-base-merge-and-conflict-recovery.md` D4 は、この 3 値照合と baseline の欠損・
読み取り失敗の fail-closed を、文脈を限定せずに定めた。

しかし baseline は gitignored な local 状態であり、commit にも branch blob にも現れない。そのため fresh CI checkout や merge gate のように baseline 権威がそもそも存在しない文脈では、D4 をそのまま適用すると構造的に必ず fail-closed になる。権威の不在という構造的事実と、権威の破損という異常が、同じ扱いに畳まれていた。

本 decision はこのうち `implementation_input_hash` の構成と可用性、および権威が不在となる文脈での
照合範囲を定める。

## Decision

### D1: 権威照合の適用範囲を、その権威が存在する文脈に限定する

D4 の 3 hash 照合と baseline の fail-closed 扱いは、baseline 権威が存在する文脈、すなわち local pre-commit workspace に適用する。この workspace が baseline 新鮮度の唯一の enforcement point である。

baseline 権威が構造的に存在しない文脈（fresh CI checkout、merge gate）は、commit された権威のみを照合する。

規則は非対称とする。存在する権威が読み取り不能または不一致であれば、文脈を問わず常に fail-closed とする。照合を縮退させるのは権威の構造的不在だけであり、取得失敗をこれに含めない。

`implementation_input_hash` は、commit された成分（source tree、Cargo manifests、`Cargo.lock`、commit された `tddd-features.json` の feature selection）と、local-only 権威である nightly toolchain identity（`rustup run nightly rustc -Vv`）の積として構成する。

nightly toolchain が構造的に不在であると判定できるのは、toolchain enumeration（例: `rustup toolchain list`）自体が成功し、かつ一致する nightly toolchain が install されていない場合に限る。この場合に限り、既存の非対称規則に従って implementation-input の照合だけを skip する。rustup または probe の実行失敗、install 済み nightly の identity 読み取り失敗、および hash を計算できる場合の不一致は、すべて fail-closed とする。存在する権威の読み取り不能を構造的不在へ格下げしない。

この意味論は、catalogue 宣言と実装の一致および cache freshness を local pre-commit で確認する
`signal check-impl-catalog` と、merge gate の branch implementation-input verification の両方に同一に
適用する。両 consumer は commit された declaration 権威を照合し、baseline が local に存在する場合は
既存の baseline 照合も維持する。stable-only CI tools image では、nightly の構造的不在が上記 probe で
確定した場合だけ toolchain-dependent な implementation-input 照合を縮退させる。

代替案として、stable-only tools image に pinned nightly を provision する案と、composite hash を分割して commit 成分だけの hash を永続化する案を検討した。いずれも toolchain provisioning または永続 artifact の contract 追加を要する重い変更であるため採用せず、将来の独立した判断で再検討可能とする。

### Existing decision relationship

本 ADR の D1 は `2026-07-29-0839-base-merge-and-conflict-recovery.md` D4 を **modifies** する（適用範囲を狭める）。D4 の照合内容そのもの（3 hash の同値要求、cache miss 時の再評価、原子的書き戻し）は変更しないが、baseline 不在に対する D4 の無条件 fail-closed を、権威が存在する文脈に限定する点で decision を変更している。

これは user が承認済みの decision に対する modification proposal であり、PR merge 段階での user 裁定を待つ。

## Rejected Alternatives

- **baseline hash の attestation artifact を commit し、merge gate でも baseline 新鮮度を照合できるようにする**: 新しい成果物の生成・更新・失効という lifecycle を追加することになり、権威の適用範囲を定めるという当面の問いの範囲を超えるため却下する。独立した判断として改めて検討し得る。

## Consequences

- Good: 権威が存在しない文脈で構造的に不可避な fail-closed が起きなくなり、CI と merge gate が成立する。
- Good: 権威の構造的不在と権威の破損が分離され、後者は文脈を問わず fail-closed のまま保たれる。
- Bad: baseline 新鮮度の enforcement point が local pre-commit workspace だけになり、その一点が回避されると merge gate では検出できない。

## Reassess When

- baseline が commit される成果物、または commit された権威から再構成可能な成果物になったとき。
- merge gate が local workspace 相当の権威を利用できる実行形態を得たとき。

## Related

- `knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md` D4 — D1 が modifies する対象。
- `knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md` — 同じ D4 の baseline-hash self-healing を前提に baseline lifecycle を定めた decision。
