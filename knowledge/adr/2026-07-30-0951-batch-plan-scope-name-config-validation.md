---
adr_id: "2026-07-30-0951-batch-plan-scope-name-config-validation"
decisions:
  - id: D1
    review_finding_ref: "review_finding:PR #228 review round 1 finding 2 (inline comment on libs/domain/src/batch_plan/mod.rs — 設定に存在しない scope 名が ceiling 照合を素通りする)"
    candidate_selection: "from:[cross-file-structural-check,scope-name-construction-boundary,advisory-warning,default-ceiling-fallback] chose:cross-file-structural-check"
    status: proposed
  - id: D2
    review_finding_ref: "review_finding:PR #228 review round 1 finding 2 (inline comment on libs/domain/src/batch_plan/mod.rs — 設定に存在しない scope 名が ceiling 照合を素通りする)"
    candidate_selection: "from:[configured-scopes-and-other-only,unknown-name-as-unconstrained] chose:configured-scopes-and-other-only"
    status: proposed
---
# `batch-plan.json` の scope 名を review scope 設定に照合し、未知名を fail-closed で拒否する

## Context

`batch-plan.json` の見積りが宣言する scope 名は、decode 時に `MainScopeName` として構築される。
その構築時検査は非空 / ASCII / 予約名 `other` の 3 点のみで、設定済み scope 集合との照合は含まれない。

ceiling の解決は `ReviewScopeConfig::diff_ceiling_for_scope` が担い、設定に存在しない名前では
per-scope 上限も global 既定も引けず `None` を返す。`ScopeCeiling::resolve(None)` はこれを
`Unconstrained` に変換し、`Unconstrained` はあらゆる合計を admit する。

その結果、設定に存在しない scope 名（typo、改名・削除された名前、架空の名前）を宣言した見積りは、
batch 単位の Σ 照合と遷移時の admission 判定の**双方で照合対象から外れる**。ceiling による強制は
宣言側が名前を一文字変えるだけで無効化でき、その逸脱は fail する代わりに沈黙して通る。

さらに「ceiling が解決されない scope は照合されない」という記録は、未知名からの経路も許容解釈として
読めてしまう。設定上の意図（上限を置かない）と、宣言の誤りが検査を消す事故とが同じ状態に写る。

## Decision

### D1: main scope 名を設定済み scope 集合と照合し、未知名は fail-closed で拒否する

`batch-plan.json` の見積りが宣言する `ScopeName::Main` は、`ReviewScopeConfig` の設定済み scope 集合に
実在しなければならない。実在しない名前を宣言した `batch-plan.json` は fail-closed で拒否する。
`ScopeName::Other` は暗黙 scope であり常に有効。graceful skip は設けない。

**検査レーンは `batch-plan.json` の cross-file 構造検査**（Phase 3 の終端 gate）とする。加えて、
遷移時の admission 判定も独立に ceiling を解決するため、そこでも未知名は `Unconstrained` に落とさず
見積り欠落と同格の error とする。gate 通過と遷移は別時点の判定であり、片側だけの検査では、gate 後に
書き換えられた宣言が照合なしで admit される。

このレーンを選ぶ根拠:

- 照合の相手は別ファイル（`.harness/config/review-scope.json`）に置かれた設定である。「file 内で
  表現できる不整合は codec、他ファイルとの整合は構造検査」という既存の切り分けに従えば cross-file 側に
  落ち、task id の実在検査と同じ位置に載る。
- `ReviewScopeConfig` の構築は track id を要する（scope pattern の placeholder 展開と現行 track prefix の
  解決）。対して `batch-plan.json` の decode は、active でない track の成果物描画からも呼ばれる
  設定非依存の読み取りである。構築境界に設定を渡す形にすると、decode が track 文脈の解決に結合し、
  設定を読めない読み取り経路が成立しなくなる。
- 必要な照合 API は既に存在する（`ReviewScopeConfig::contains_scope` / `all_scope_names`。いずれも
  `Other` を有効として扱う）。新しい domain API も codec 変更も要さない。

### D2: `Unconstrained` を許すのは設定済みで上限未設定の scope と `other` に限る

ceiling 未解決（= 照合せず全 admit）が正当なのは次の 2 通りに限る:

1. 設定済み scope で、per-scope 上限も global 既定も設定されていない。
2. `other` — named scope のいずれにも一致しなかった残り物を表す予約名で、per-scope 上限を持たず
   global 既定も継承しない。

未知名から `Unconstrained` に至る第 3 の経路は存在してはならない。「ceiling が解決されなければ照合
しない」という規則は、名前が設定に実在することを前提として初めて意味を持つ。その前提を D1 の照合が
保証する。

### 既存決定との関係

本 ADR は `2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md` D2 の cross-file 構造検査の
項目集合に一項を追加し（D1）、同 D3 の admission 判定に未知名の error 化を加える形で、両者を
**refines** する。`batch-plan.json` の schema、ceiling 照合式、単独寄与者の免除、membership 検査の
内容は変更しない。

## Rejected Alternatives

- **scope 名の構築境界に設定を渡して検査する**: 未知名を含む document が構築段階で存在し得なくなる
  点は強いが、`MainScopeName` の構築と decode が設定と track 文脈に結合する。設定を読まない読み取り
  経路（非 active track の描画）が壊れ、file-internal 不変条件と cross-file 整合の切り分けにも反する。
- **未知名を警告して通す（advisory）**: 検出が review 依存に戻り、typo が強制を無効化する経路が残る。
  強制力の梯子を機構から docs へ一段下げる。
- **未知名を global 既定 ceiling で照合する**: 名前の誤りを、有効な宣言として silently 読み替える。
  誤りの是正ではなく隠蔽であり、設定 SSoT に存在しない scope に review 単位が存在しないという矛盾も
  残る。

## Consequences

- Good: 宣言側の名前の誤りが ceiling 照合を無効化する経路が閉じる。強制の有効性が宣言の綴りに
  依存しなくなる。
- Good: 照合されない状態が設定上の意図（上限未設定 / `other`）のみに対応し、許容解釈の幅が消える。
- Good: gate と admission の双方で fail-closed になり、片側通過後の宣言書き換えでも素通りしない。
- Good: 既存 accessor で足り、codec・domain API・設定 schema の変更を伴わない。
- Bad: 設定から scope 名を削除・改名すると、その名前を宣言していた active track の計画成果物が gate で
  落ちる。是正は計画側の書き換えで、実装開始前のため手戻りは生じない。
- Neutral: `other` の扱いと、上限未設定 scope の扱いは変わらない。既存成果物へ遡及しない。

## Reassess When

- scope 設定の改名・削除が頻繁になり、active track の計画成果物が繰り返し落ちる状態が常態化した場合。
- 上限未設定の scope が多数を占め、`Unconstrained` を許す 2 経路の区別が実務上意味を失った場合。

## Related

- `knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md` D2 / D3 — refines 対象。
- `knowledge/adr/2026-07-29-0358-task-dependency-declaration-and-batch-order-check.md` — 同じ検査集合を
  refine した先行 delta。本 ADR はその内容を変更せず、別の一項を追加する。
- `knowledge/adr/2026-04-18-1354-review-scope-prompt-injection.md` D5 — `other` を予約名かつ
  predicate-of-absence として扱う原典（D2 の第 2 項の根拠）。
- `knowledge/conventions/enforce-by-mechanism.md` — 未知名を advisory に留めない判断の原則。
