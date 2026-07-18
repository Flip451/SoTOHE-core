# Pre-Track ADR Authoring Convention

## Purpose

ADR は track 内成果物ではなく **track 前段階** で author が作成する track 横断資産とする。track のライフサイクル (作成 → 進行 → 完了 → archive) と ADR のライフサイクル (書き換え可だが原則安定、全 track で参照される) を独立に保つため、ADR 生成を `/track:plan` の内部ステップから切り離し、事前確認のみを行う。

## Scope

- 適用対象: `knowledge/adr/*.md` の新規作成 / 更新、`/track:plan` の起動条件、adr-editor サブエージェントの invocation、`.claude/commands/**/*.md` および `.claude/skills/**/SKILL.md` 本文からの ADR 参照スタイル。
- 適用外: track 内 research note (`track/items/<id>/research/` 配下)、spec.json や型カタログなど track 内 SSoT 成果物、既に `archive/` に移動した track の ADR 修正。

## Rules

- **配置**: ADR は `knowledge/adr/YYYY-MM-DD-HHMM-slug.md` に配置する。`track/items/` 配下には置かない。
- **作成タイミング**: ADR ファイルの **初期起草** (knowledge/adr/ への file 作成) は `/track:plan` **起動前** に user + main 対話 (または手動) で完了させる。`/track:plan` は ADR を自動生成しない。**初回 commit はその ADR を最初に必要とする track の `/track:init` 直後に `/track:review` → `/track:commit` で行う**のが typical (この時点で ADR commit 履歴ありとなり、後続の `/track:plan` の back-and-forth `adr-editor` ループが「commit 履歴あり」 path で動作する)。ADR は track 横断資産であり 1 つの ADR が複数 track にまたがる関係 (ADR ⊇ track) を維持する。
- **起動時の事前確認**: `/track:plan` は起動直後に、参照予定 ADR が `knowledge/adr/` に存在するかを確認する。未整備なら停止し、ADR 整備を促す (厳密モード)。
- **状態フィールドなし**: ADR に `Status` 見出しや `approved` のような状態フィールドは作らない。ファイルが存在して内容が読めることは `/track:plan` の起動前提として十分であるが、Phase 0 の user 承認（下記の zero_findings 後の裁定）を代替しない。
- **書き換え可 (pre-track 文脈に限る)**: track 前段階の user + main 対話 (または手動) で既存 ADR を直接書き換えられるのは、`knowledge/conventions/adr.md` の effective merge target 判定で **pre-merge** の ADR に限る。merge target に既に入った ADR は永続 record であり、意味上の変更は新 ADR で supersede または refinement する。track が当該 ADR を baseline として刻印した後の書き換えは、下記「In-track 意味変更の裁定権」節に従う (この一文を track 内の包括的な編集許可として読んではならない)。
- **track 内成果物からの参照**: spec.json は ADR を構造化参照 (`AdrRef { file, anchor }`) で cite できる (SoT Chain ① spec → ADR)。型カタログ (Phase 2) は spec を `spec_refs[]` で参照し、ADR を直接 cite することは SoT Chain のレイヤースキップになるので禁止 (`type catalogue → ADR` は逆流/スキップ)。impl-plan (Phase 3) も同様に spec / 型カタログ経由で参照し、ADR を直接 cite しない。逆方向 (ADR から track 内成果物への参照) は SoT Chain 逆流なので禁止。
- **back-and-forth 自動修正 (adr-editor)**: `/track:plan` の探索的精緻化ループで下流 signal が 🔴 になって ADR 側の修正が必要になった場合、adr-editor サブエージェントが ADR を working tree レベルで編集する (正規経路の全体像と刻印の扱いは下記「In-track 意味変更の裁定権」節)。in-place 編集の対象は effective merge target 判定 (`knowledge/conventions/adr.md`) で **pre-merge** の ADR に限る — merge 済み ADR が編集を要する場合は下記裁定権節の post-merge 規則 (新 ADR 化) に従う。
  - ADR ファイルに commit 履歴あり (かつ pre-merge) → auto-edit (working tree のみ、loop 中は commit しない)
  - ADR ファイルに commit 履歴なし → user pause (ADR を先に commit してから再開)
- **終端処理**: `/track:plan` 終了時に ADR working tree に HEAD からの diff があれば、user に diff を提示して判断 (accept / revert / 手動修正 / 中止) を仰ぐ。
- **main による直接編集の禁止**: back-and-forth での ADR 修正も含めて、main orchestrator が `knowledge/adr/*.md` を直接 Edit してはならない。adr-editor サブエージェントを経由する (1 ファイル = 1 writer 原則)。
- **skill / command からの特定 ADR 参照禁止**: `.claude/commands/**/*.md` および `.claude/skills/**/SKILL.md` の本文では、特定 ADR を file path / 日付付きスラグ (`knowledge/adr/YYYY-MM-DD-HHMM-slug.md`) で直接 cite しない。commands / skills は運用ドキュメントとして自己完結させ、ADR の背景知識を前提とせず動作・条件だけで読者が理解できる粒度で記述する。ADR の decision を workflow に反映する際は、その workflow 文章が ADR を指さなくても意味が通じるように書き直す。設計背景を残したい場合は、変更を持ち込んだ track の commit message / 該当 ADR 側 (Consequences / Related) に記述する。
  - OK: 「`knowledge/adr/` 配下に事前 ADR を配置する」「ADR を参照する spec.json がある場合は…」のような generic / pattern description。
  - NG: 「per ADR `YYYY-MM-DD-HHMM-slug.md` §Dn」のように特定 ADR を日付付きスラグで cite。

## In-track 意味変更の裁定権

ADR の意味は user に属する。track が ADR を扱う全期間について、意味変更の正規経路と裁定点を以下に固定する。

**適用前提 (post-merge 不変則)**: 本節の in-place 編集 lane (Phase 0 ループ・Phase 1+ escalation ループ) はすべて、対象 ADR が effective merge target 判定 (`knowledge/conventions/adr.md` §Lifecycle) で **pre-merge** であることを前提とする。merge target に既に入った ADR は永続 record であり、track 内であっても in-place の意味変更 (決定保存の精緻化を含む) は行わない — 許されるのは誤字・参照 path・後方参照の追記のみで、意味に関わる変更は新 ADR (supersede / refinement) として起草し、新規 ADR の承認経路で扱う。守護者 (adr-diagnoser) は merge 済み ADR への意味編集提案に対し、保全代案を「新 ADR 起草」の形で提示する。

役割は 4 者で分離する:

| 役割 | 担い手 | 責務 |
|---|---|---|
| 書き手 | adr-editor | 編集の作成・適用 (working tree のみ) |
| 守護者 | adr-diagnoser | **track 内の ADR 編集ごとに「元の決定を壊していないか」を判定する**。精緻化 (許容) と決定破壊 (不許可) の峻別はこの判定者に属する。決定を壊す提案には、**決定を保全する代案を示すか、修正が必要ない旨を理由付きで提示する義務**を負う (裸の差し止めで終わらない) |
| 配達人 | orchestrator | dispatch・記録・proposal の運搬のみ。意味の裁定を行わない |
| 裁定者 | user | Phase 0 の承認エスカレーションと merge 監査 |

### Phase 0 — ADR-baseline 確定まで (境界の内側)

1. **init 刻印**: track init が持ち込み時点の ADR を ledger に init 種別で刻印する。「track に何を持ち込んだか」の記録であり、後続レビューで user が見る diff の基準点。
2. **baseline review ループ**: ADR-baseline review の findings への修正は、**意味変更を含めて**適用してよい (writer は adr-editor。dispatch は capability の provider routing に従う)。**各編集の直後に adr-diagnoser が「元の決定を壊していないか」を判定する** — 決定を保存する編集 (情報の精緻化・grounding 補強) のみループ内で採用できる。決定を壊すと判定された変更は採用しない (編集前の文面へ戻す)。その際 adr-diagnoser は決定を保全する代案を示すか、修正が必要ない旨を理由付きで提示し、orchestrator は**その出力をそのままレビュアーへ伝達する** — 代案は adr-editor が適用して再レビューに進み、修正不要の判定はレビュアーが所見への回答として再判定する。レビュアーが所見を維持して対立が解消せず `zero_findings` に到達できない場合だけ、orchestrator は裁定せずにその対立を Phase 0 の user エスカレーションへ載せ、刻印せずに user の裁定を待つ。裁定後は下記の再レビュー規則に従う。**ループ中は ledger に一切書き込まない** — 中間刻印は禁止。init 記録との乖離は「レビュー中の draft」を意味する正常状態である。
3. **zero_findings 到達 → user エスカレーション**: 全 required scope が zero_findings に至ったら、user に (a) 採用済み編集の init 刻印との diff、(b) 守護者が差し止めた提案とその判定 (保全代案 / 修正不要理由) を提示してレビューを仰ぐ。この提示は user が実際に読める形で行う — diff の内容 (変更 hunk の原文、または hunk 単位の忠実な要約) を裁定依頼と同じ chat 本文に明示する。user に表示されない tool 出力・ファイル path 参照・添付だけによる提示は裁定依頼の前提を満たさない。finding 単位で user を割り込ませない — user がレビューするのは収束後の全体である。user が proposal を採用して ADR 文面を変える場合、adr-editor が適用し、adr-diagnoser の判定を通した後、承認済み review hash は stale になるため手順 2 の fresh review から再収束する。このとき user が決定変更を明示的に裁定した文面は、その裁定と採用文面（後で付す `user_decision_ref` を含む）を守護者 briefing に渡し、fresh review 中の比較基準とする。これは init 刻印の更新でも中間刻印の許可でもない — init は user が見る diff 基準のまま保持する。文面を変えない承認だけが手順 4 に進める。手順 2 の未解消対立に対する user 裁定も、文面を変える場合は同じ再レビュー規則に従う。
4. **承認 → 刻印 → コミット**: user が編集を承認したら、はじめて編集後文面を ledger に記録し、その上で ADR-baseline commit を行う。**ここまでが境界**。
5. Phase 0 で pipeline を止めて user に届けてよいのは、上記エスカレーションと「先へ進めない設計欠陥」のみ。

### Phase 1 以降 — 自律区間 (境界の外側)

- **正規の意味変更経路は signal / gate 駆動の escalation ループのみ** (Phase 1-3 の grounding escalation、impl 以降の diagnose ルーティングが adr を指した場合等)。編集は adr-editor が行う。
- **正規ループ内の編集も守護者判定を通す**: 編集直後に adr-diagnoser が「元の決定を壊していないか」を判定し、決定を保存する編集のみ escalation 種別の刻印 (reason 必須) に進める。決定を壊すと判定された変更は revert し、守護者は保全代案または修正不要の理由を提示する。orchestrator はそれを所見の発生元 (レビュアー等) へ伝達し、代案は adr-editor が適用できる。解消しない対立は amendment proposal として merge 監査へ先送りする。
- **情報の精緻化** (決定を覆さない補強・明確化) は正規ループ内で許容されうる。精緻化か決定破壊かの峻別は adr-diagnoser の判定に委ね、orchestrator が代行しない。ADR は決定を記録するものであり設計詳細に立ち入らないため、精緻化が意味を持つ場面は稀である。
- **正規ループ外からの意味変更提案** (レビュー所見としての選好・別案・scope 論等) に対して、orchestrator は**自律裁定しない**。かつ**走行中の pipeline を user 確認のために止めない** — amendment proposal として記録し、user 裁定は **merge 段階 (PR review での user 監査) へ先送り**する。
- **不意の変更** (経路不明の baseline 乖離) は byte 照合で検出し、adr-diagnoser が分類する — 守護者のもう一つの持ち場。「意味を変えない」ことの要求がこの検出機構の目的であり、意味を変えない差分のみ再刻印できる。それ以外は restore する。

### 機構との整合

本節が規範の正であり、機構は以下の形で追随している:

- `adr-baseline check-review` — review 入口の ADR 状態検査は「init 刻印の存在確認と帳簿完全性」のみであり、byte 照合の発火点は commit gate / CI に限られる (決定: `knowledge/adr/` の該当 ADR)。専用の承認 kind は存在しない。承認の記録は、ループで修正した decision 自身の front-matter `user_decision_ref` への承認 ref 追記が担い (chain ⓪ が検証)、その後の escalation 刻印の reason には自己完結の欠落説明だけを記す (承認の ledger 重複記録なし)。編集なしで収束した場合は追加の刻印を行わない。
- 守護者判定の workflow 配線 — capability 定義 (`.harness/capabilities/adr-diagnoser.md`) が編集判定モード (決定保存 / 決定破壊 + 保全代案または修正不要理由) を定義し、review / plan / adr2pr の workflow SSoT が「編集ごとに守護者判定を挟み、その出力をレビュアーへ還流させる」手順を刻む。
- plan / adr2pr workflow の autonomy 制約 — Phase 0 の user 承認エスカレーションが明示的例外として carve-out されている。

## Examples

- Good: user が `knowledge/adr/<date>-<slug>.md` を作成 → `/track:plan "feature X"` を invoke → `/track:plan` が ADR 存在を確認して Phase 0 (init) に進む。
- Good: Phase 1 (spec) で signal 🔴 発生 → `/track:plan` が adr-editor を自動 invoke (ADR に commit 履歴あり) → working tree 編集 → loop 再開 → 終端で user に diff 提示。（Phase 2 の 🔴 は spec-designer が再 invoke される。adr-editor が呼ばれるのは Phase 1 信号が ADR 側の修正を要求する場合のみ。）
- Good: Phase 0 の ADR-baseline review が意味変更を含む修正で zero_findings に収束 → user が init 刻印との diff をレビューして承認 → 承認文面を刻印 → ADR-baseline commit。
- Good: レビュー所見が決定変更を要求 → adr-diagnoser が決定破壊と判定し保全代案を提示 → orchestrator が判定と代案をレビュアーへ伝達、adr-editor が代案を適用 → 再レビューで収束。レビュアーが所見を維持した場合はその対立を裁定点 (Phase 0 エスカレーション / merge 監査) へ運ぶ。
- Bad: `/track:plan "feature X"` を実行、内部で ADR を自動生成 (spec に合わせて decision を後付けする rationalization の温床になる)。
- Bad: main が `Edit` tool で `knowledge/adr/xxx.md` を直接書き換える (1 ファイル 1 writer 原則違反)。
- Bad: Phase 0 の review ループ中、編集のたびに escalation 刻印を打つ (承認前の中間刻印は、user 監査の diff 基準を自己洗浄する行為)。
- Bad: zero_findings 到達後、user エスカレーションを経ずに刻印・コミットまで自律で進める (裁定点の無断通過)。
- Bad: レビュー所見の別案提案を根拠に、走行中の pipeline で ADR の決定内容を書き換える (正規ループ外の意味変更。merge 監査へ先送りすべきもの)。
- Bad: adr-editor の編集を adr-diagnoser の決定保存判定を通さずに採用・刻印する (守護者の不在。精緻化か決定破壊かを orchestrator が自己裁定することになる)。

## Exceptions

- **既存 ADR 流用**: 本機能の設計判断が既存 ADR でカバーされているなら新規作成は不要。
- **緩和モードなし**: 「ADR 未整備でも stub で進行を許す」モードは意図的にサポートしない。必要性が実証された時点で別 ADR で検討する。

## Review Checklist

- [ ] track 内成果物 (`track/items/<id>/` 配下) に ADR を配置していないか
- [ ] ADR ファイルに `Status` / `approved` 等の状態フィールドを追加していないか
- [ ] `/track:plan` 起動前に ADR が `knowledge/adr/` に存在するか (厳密モード条件を満たすか)
- [ ] ADR の back-and-forth 修正で main が直接編集せず adr-editor を経由しているか
- [ ] Phase 0 の baseline review ループ中に ledger へ書き込んでいないか (中間刻印禁止)
- [ ] in-track の ADR 編集ごとに adr-diagnoser の決定保存判定を通しているか (守護者を迂回していないか)
- [ ] zero_findings 到達後、user 承認を経てから刻印・コミットしているか
- [ ] 正規ループ外の意味変更提案を自律裁定せず、amendment proposal として merge 監査へ先送りしているか
- [ ] ADR からの参照が track 内成果物を指していないか (SoT Chain 逆流禁止)
- [ ] `.claude/commands/**/*.md` / `.claude/skills/**/SKILL.md` の本文が特定 ADR を日付付きスラグで cite していないか (commands / skills は自己完結)

## Decision Reference

- [knowledge/adr/README.md](../adr/README.md) — ADR 索引。本 convention の原典となる ADR はこの索引から辿る
- [knowledge/conventions/adr.md](./adr.md) — ADR 運用の基本ルール
- [knowledge/conventions/workflow-ceremony-minimization.md](./workflow-ceremony-minimization.md) — 事後レビュー方式 / 事前承認限定の原則
