---
adr_id: 2026-08-11-1857-review-fix-delivery-boundary
decisions:
  - id: D1
    user_decision_ref: "chat:2026-08-12:delta ADR 棄却(実装レベルの入力衛生であり ADR 不要と裁定)"
    status: deprecated
  - id: D2
    user_decision_ref: "chat:2026-08-12:delta ADR 棄却(実装レベルの入力衛生であり ADR 不要と裁定)"
    status: deprecated
  - id: D3
    user_decision_ref: "chat:2026-08-12:delta ADR 棄却(実装レベルの入力衛生であり ADR 不要と裁定)"
    status: deprecated
---
# review-fix コマンド群の delivery boundary と briefing の信頼境界

## Rejection (deprecated)

本 ADR は棄却する。briefing path の相対パス、symlink 拒否、repository root 内への containment、
regular file、64 KiB 上限、および content を運ぶ dispatch は実装レベルの入力衛生であり、
永続的なアーキテクチャ判断として記録する必要がない。「信頼境界」という位置づけは過大であり、
content-passing の義務化は正当化されない transport mechanism の連鎖を生むため、D1〜D3 を
deprecated とする。

## Original Record (historical — rejected and not in force)

以下の原文全体は棄却された提案の記録としてそのまま保存するものであり、いずれの記述も現在は効力を持たない。

### Context

review-fix コマンド群では、汎用的な CLI delivery の層分離だけでは、raw input の所有者、
書き込み前の active track 照合、および briefing の安全な読み取り位置まで一意に定まらない。
family 固有の境界を永続的な判断として記録し、入力検証、実行、表示、ファイル読み取りの責務を
混在させない必要がある。

### Rejected Decision Record (historical)

以下の D1〜D3 は deprecated であり、現在有効な決定ではない。原文を歴史的記録として残す。

#### D1: review-fix の composition root、driver、bin の delivery boundary を固定する

review-fix の composition root は依存を配線して driver を構築し、呼び出し元へ返すだけとする。
driver の呼び出しや結果の render は行わない。

review-fix driver は、driver が所有する raw delivery DTO を受け取る。この DTO は raw scope、
briefing path、optional track id、round type、および optional model strings を保持する。driver は
raw values を usecase が所有する request constructor へ渡して集中的な validation を受け、typed な
usecase result または error を command outcome に render する。

bin は raw values を driver へ渡して outcome を emit するだけとする。domain 型または usecase 型を
構築せず、re-export もしない。

この境界により、配線、外部入力から application request への変換、usecase invocation、結果表示の
所有者が分離され、bin や composition root に application boundary の知識が逆流しない。

#### D2: review-fix の書き込みは current track との一致を実行前に要求する

review-fix の write-side execution は、runner を呼び出す前に現在の `track/<id>` branch を解決する。
track id が明示された場合は解決した id との一致を要求し、不一致または non-track branch では
runner を呼び出さず fail-closed で拒否する。

raw scope と round type は usecase boundary で検証し、invalid な値は対象 field を識別できる typed
error として返す。

この境界により、別の track を現在の作業文脈で誤って変更する経路を閉じ、入力の形式違反を外部実行の
開始後ではなく application boundary で確定できる。

#### D3: briefing は信頼できる chokepoint で一度だけ読み取る

review-fix run が消費する briefing は、repository root 内に収まること、path の各要素が symlink で
ないこと、regular file であること、および size 上限内であることを検証する一つの信頼できる
chokepoint で一度だけ読み取る。run が保持して dispatch するのは検証済みの content とし、後から
再度開ける path は渡さない。

この境界により、検証後の path 差し替えや検証を迂回した再読み取りを防ぎ、dispatch が受け取る
briefing を検証済み bytes に固定できる。

### Rejected Alternatives

#### A. composition root が driver を呼び出して結果を render する

配線と request 単位の実行・表示が同じ境界に混在し、composition root を再び command facade にするため
採用しない。

#### B. bin が domain 型または usecase request を構築する

delivery 固有の raw input と application boundary の型変換が bin に混在し、usecase が validation を
所有できなくなるため採用しない。

#### C. 明示された track id を current branch と照合せず書き込みを許可する

現在の code と working tree の文脈を別の track の成果物へ適用でき、誤対象への変更を成功として扱うため
採用しない。

#### D. briefing path を dispatch 先で再度開く

最初の検証と実際の読み取りの間で参照先が変わり得るうえ、すべての再読み取り箇所に同じ安全条件を
重複実装しなければならないため採用しない。

### Consequences

#### Positive

- review-fix command family の wire、invoke、render、emit の責務境界が一意になる。
- runner invocation より前に active track と raw input の不整合を typed failure として確定できる。
- briefing の validation と読み取りが一つの境界に集約され、dispatch 後の再解釈を防げる。

#### Negative

- driver は raw delivery DTO から usecase request への変換と outcome render の両方向を所有する。
- briefing の size 上限を設けるため、上限を超える input は内容が正しくても拒否される。
- write-side execution は non-track branch から explicit track id を指定して実行できない。

### Reassess When

- review-fix に CLI 以外の delivery adapter が加わり、raw input DTO を共有する必要が生じたとき。
- briefing を file 以外の信頼済み source から受け取る要件が生じたとき。
- review-fix が track artifact を変更しない read-only operation へ分割されたとき。

### Related

- `knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md` D1 — CLI から usecase boundary を経由し、
  domain 型を delivery surface に出さない基礎決定。
- `knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md` D2 / D3 / D5 —
  wire-only composition root、invoke と render を所有する primary adapter、および thin bin の基礎決定。
- `knowledge/adr/2026-05-26-1813-track-id-default-active-track.md` D7 — write operation で explicit track id と
  current branch の一致を要求する基礎決定。
