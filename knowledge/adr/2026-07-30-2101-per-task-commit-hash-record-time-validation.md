---
adr_id: "2026-07-30-2101-per-task-commit-hash-record-time-validation"
decisions:
  - id: D1
    review_finding_ref: "review_finding:PR #228 review round 7 finding 1 (--commit-hash が形式検査のみのため、偽または誤った well-formed hash が settle を成立させ現在 batch を早期に閉じる)"
    candidate_selection: "from:[record-time-repository-validation,judgment-time-settle-validation,require-hash-equals-current-track-commit-record,accepted-residual-risk-record-only] chose:record-time-repository-validation"
    status: proposed
---
# per-task commit hash の記録時に repository 実在と HEAD 到達可能性を要求する

## Context

タスクの commit hash に対する検査は形式のみである — 7〜40 桁の小文字 hex であれば受け付けられ、その
hash が当該 repository に実在するか、HEAD から到達可能かは問われない。

この記録は settle の成立条件に直結する。settle は commit 記録を伴う done、または skipped であり
（`2026-07-30-1022-batch-plan-declaration-domain-unsettled-tasks.md` D1）、settle は現在 batch の解決に
入る。現在 batch は宣言順で最先行の、未 commit の member を残す batch であり、ある batch の member 全件が
settle するとそれは次へ進む（`2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md` D3）。

したがって形式だけ正しい誤った hash が 1 件記録されると、未 commit の作業を残したまま batch 境界が
閉じ、後続 batch のタスクが admission を通る。その差分は先行 batch の未 commit 差分と同じ review
diff base の内側に積まれ、1521 D3 が構造的に禁じたはずの宣言 batch の**合併**がこの経路で成立する。

記録は batch commit 後の手作業の埋め戻しであり、正規 wrapper を経る。1521 D3 が membership 検査を
機構化した動機は「散文の読み落としによる素通り」を塞ぐことだったが、その判定が依拠する入力値のうち
commit hash だけは無検査の自己申告のまま残っている。架空の hash、他 repository や未到達 branch の
hash、桁を打ち間違えて別 object を指す hash はいずれも形式検査を通る。

hash が状態へ入る経路は一つに限られている。タスク状態遷移のうち hash を書くのは、hash を伴う完了記録
（`Complete`）と後からの埋め戻し（`BackfillHash`）の 2 つだけであり、どちらも同じ遷移 CLI が唯一の
入口である。事故の発生源はこの入口に局在している。

## Decision

### D1: hash を書く遷移は、実在し HEAD の祖先である hash のみを受け付ける

commit hash を伴うタスク状態遷移は、与えられた hash が当該 repository に commit object として実在し、
かつ HEAD の祖先であることを要求する。対象は hash を書く 2 つの遷移の双方 — hash を伴う完了記録
（`Complete`）と後からの埋め戻し（`BackfillHash`）— であり、記録経路による例外は設けない。hash を
伴わない完了記録は検査対象ではない（記録すべき値が存在せず、その done は従来どおり未 settle である）。

満たさない記録は error として拒否し、**状態を一切書き換えない**。`impl-plan.json` は遷移前のまま残り、
部分適用も graceful skip も生じない。検証を実行できない場合（repository の読み取り自体の失敗）も同様に
error とする。検証せずに記録することはしない。

拒否は記録の時点で行う。その時点は operator が present で、遷移の目的は事実の記録そのものであるため、
誤った値を名指す診断がその場で出て、誤った記録が状態に入る前に止まる。形式検査を通る 3 種の誤り
（架空の hash、他 repository / 未到達 branch の hash、桁の打ち間違いで別 object を指す hash）のうち、
実在と到達可能性で判別できるものはここで塞がる。

検証は repository の読み取りを要するため、usecase の port と、track 成果物が置かれた repository を
起点に解決する infrastructure adapter が担う。domain は I/O を持たず、検証結果を入力として受ける。
repository の解決は items_dir を起点とする既存の不変条件に従い、process の作業ディレクトリには
依存しない。

### 既存決定との関係

本 ADR は**何が記録され得るか**を制約するもので、記録がどう読まれるかは変更しない。

- 1022 D1 の settle 述語（commit 記録を伴う done、または skipped）は不変である。settle は引き続き記録の
  存在のみで判定され、既に settle しているタスクの状態が本決定によって変わることはない。
- 1521 D3 の判定入力・判定式・判定対象の遷移集合・`batch-plan.json` 不在 = error・各 scope の 1 件目の
  寄与を常に admit する規則はいずれも不変である。admission が新たに repository を照会することもない。
- `batch-plan.json` の schema、Σ 照合、単独寄与者免除、宣言域を未 settle タスクに限る規則、依存順序
  検査、scope 名の照合もいずれも変更しない。

## Rejected Alternatives

- **判定の時点で照合し、検証されない記録のタスクを未 settle として扱う**: 記録経路を経ずに現れた hash
  にも効くが、settle 述語そのものを narrow するため、既に settle しているタスクが遡って未 settle へ
  落ちる。その波及は判定の拒否だけでは終わらず、当該タスクが Phase 3 gate の宣言域へ戻って宣言更新を
  要求し、再開には reopen の判定を経る必要が生じる。一方、得られる保護の差分は薄い — hash を書く経路は
  遷移 CLI に限られており記録時の検査で発生源が閉じるうえ、実在し HEAD の祖先である**別の**過去 commit
  の誤記録は判定時の照合でも同じく通過する。決定を保存したまま同じ事故経路を閉じられる以上、settle 述語
  の変更は不可避ではない。
- **記録される hash が track の現在の commit 記録と一致することを要求する**: 実在する別 commit の誤記録も
  塞げるほど強いが、正規手順から外れた埋め戻しや復旧経路を機構で封鎖する。運用経路の封鎖は機構設計だけ
  で決める事項ではなく、実在と到達可能性という弱い述語で事故の主要経路を塞ぐほうが釣り合う。
- **無検査のまま維持し、偽または誤った well-formed hash による batch 早期クローズを受容残余リスクとして
  記録する**: 実装コストは 0 で、正規 wrapper を経る operator の記録を信頼する姿勢に収まる。しかし
  1521 D3 が membership 検査を機構化した理由は、正規経路を通る operator が散文を読み落としても迂回
  できないようにすることだった。同じ経路の入力値を無検査の自己申告に残すのは、機構化した guard の前提を
  docs 段へ置き直すことに等しく、`enforce-by-mechanism.md` の梯子を一段下げる。1521 が受容した残余
  リスク（見積りの大外れ）は機械が持たない予測の誤りだが、hash の実在と到達可能性は repository が
  答えられる事実であり、受容の性質が異なる。
- **形式検査を強めて 40 桁の完全長を要求する**: 打ち間違いの一部を減らすが、実在しない 40 桁 hex は
  依然として通り、短縮 hash を用いる正規手順を壊す。形式の強化は実在の代理にならない。

## Consequences

- Good: 事故の発生源（hash を書く唯一の入口）で、repository に実在しない hash と HEAD の祖先でない hash が
  止まる。この 2 種の誤りについて、1521 D3 の合併禁止が自己申告に依存しなくなる。タスクと commit の
  対応そのものは依然として自己申告であり、本決定はそれを検証しない。
- Good: 拒否が記録の時点に出るため、検査が判別できる誤りについては値そのものを名指す診断が得られる。
  後の無関係な判定の拒否として症状が現れることがない。
- Good: settle 述語も admission の判定入力も変わらないため、既存の判定結果・既存の計画成果物・既に
  settle したタスクの状態はいずれも影響を受けない。
- Neutral: hash を伴わない完了記録と skipped の扱いは変わらない。
- Bad: 実在し HEAD の祖先である**別の**過去 commit を誤って記録した場合は、実在と到達可能性の双方を
  満たすため検査を通る。この経路は残余リスクとして受容し、埋め戻しが batch commit の直後に行われる
  手順の直後性に依存する。
- Bad: 記録された hash が後に非祖先となる履歴書き換え（amend / rebase）は検知されない。検査は記録の
  時点の事実についてのみ成立する。
- Bad: 記録経路に repository 照合が入るため、port / adapter の新設と composition の配線が必要になる。

## Reassess When

- 検査を通った誤記録（実在する別 commit）による batch 早期クローズが実際に観測された場合 — 記録値の
  導出そのものを機構化するか、より強い述語へ移す。
- 履歴書き換えによって記録が非祖先となった状態が事故として観測された場合 — 記録時のみという検査時点の
  見直し。
- hash を書く遷移が増える、あるいは遷移を経ない記録経路が実際に成立した場合 — 検査点の再設計。

## Related

- `knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md` D3 — 本 ADR が入力値を
  保護する対象。判定式・判定入力・遷移集合はいずれも変更しない。
- `knowledge/adr/2026-07-30-1022-batch-plan-declaration-domain-unsettled-tasks.md` D1 — settle 述語の
  原典。本 ADR は述語を変更せず、記録され得る値を制約する。
- `knowledge/conventions/enforce-by-mechanism.md` — 自己申告を機構検査へ移す判断の原則。
