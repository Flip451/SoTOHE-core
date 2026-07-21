---
adr_id: adr-decision-freeze
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session-01AiXA34wxdwHFBKQzxiNDmG:2026-07-15 track 開始後にしか不意の ADR 編集は起きない性質を利用し、track/items/<id> 配下に対象 ADR の複製を保持する提案 + 同日追補: hash 一覧でなく逐語コピー（hash は高速化用）とする refinement への同意 + chat_segment:session-01XgEf2rrfSL8jqSD9F4KRmn:2026-07-16 正規経路の再刻印を経た baseline は merge 時の user 監査を待つ文面とし、承認の最終確定は merge が担うとする錨定義の精緻化裁定"
    candidate_selection: "from:[in-file-body-hash,track-local-hash-list,track-local-verbatim-copy] chose:track-local-verbatim-copy"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session-01XgEf2rrfSL8jqSD9F4KRmn:2026-07-15 刻印は track init と混ぜず別コマンドとし /track:init workflow が二つを順に呼ぶ裁定 + 同日追補: 対象 ADR の解決は文脈依存として orchestrator が判断しコマンドに明示的に渡す裁定、および複製元の指定は path でなく file 名とし knowledge/adr/ 直下から解決する裁定"
    candidate_selection: "from:[stamp-inside-track-init,separate-command-composed-by-workflow] chose:separate-command-composed-by-workflow"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session-01XgEf2rrfSL8jqSD9F4KRmn:2026-07-15 刻印のたびに新しい複製を追加し、古い複製はいかなる操作でも上書き不能とする（累積が tamper-evident な監査線になる）裁定 + chat_segment:session-01XgEf2rrfSL8jqSD9F4KRmn:2026-07-16 同一 hash 再刻印は ledger 追記のみ・hash8 prefix 衝突は prefix 延長で解決する裁定"
    candidate_selection: "from:[verbatim-copy-replace-latest,verbatim-copy-accumulate] chose:verbatim-copy-accumulate"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:session-01AiXA34wxdwHFBKQzxiNDmG:2026-07-14 review fix lead による ADR への逸脱的決定の混入（複数セッションで再発）を機構で解消する裁定 + chat_segment:session-01XgEf2rrfSL8jqSD9F4KRmn:2026-07-15 初版ドラフトの機構（信号機置換・hunk 裁定・record チェーン）は目的に対して過大であり、信号機には触れず commit 時のバイト照合 binary check に縮小する裁定 + chat_segment:session-01XgEf2rrfSL8jqSD9F4KRmn:2026-07-16 刻印検査を review 入口へ前倒しし、init 刻印忘れを fixer 実行前に検出する裁定 + 同日追補: hook 迂回 commit の backstop として同じ check を track-aware CI 群（local の cargo make ci と PR CI で同一に走り merge 前 backstop を構成）にも置く修正 + chat_segment:session-01XgEf2rrfSL8jqSD9F4KRmn:2026-07-16 刻印要求の対象を track が対象とする ADR に縮小（review 入口は主対象 init 刻印の存在確認、cite しない ADR は対象外）し、track 生まれの user 未承認 draft を裁定まで要求外・refs 昇格後は刻印必須とする裁定"
    candidate_selection: "from:[signal-replacement-freeze,binary-gate-check,briefing-only] chose:binary-gate-check"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:session-01XgEf2rrfSL8jqSD9F4KRmn:2026-07-15 不一致検知時は adr-diagnoser に委譲し、逸脱なしなら再刻印・逸脱ありなら機械復元して briefing に経緯を注入し差し戻す flow の提示 + 同日追補: 再刻印は非意味的編集のみ・意味変更は一律逸脱・判断に迷えば逸脱側（fail-closed）とする verdict 方針への同意"
    candidate_selection: "from:[human-escalation-always,diagnoser-triage-with-bright-line] chose:diagnoser-triage-with-bright-line"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:session-01XgEf2rrfSL8jqSD9F4KRmn:2026-07-15 「ledger 追記は diagnoser が？orchestrator が？」の問いに対し、書込は orchestrator が起動する原子的な機械コマンドに限定し diagnoser は read-only とする整理への同意 + chat_segment:session-01XgEf2rrfSL8jqSD9F4KRmn:2026-07-16 刻印種別は 5 値 enum kind とし、意味を持ち込む刻印（escalation / new-adr）のみ自己完結散文の reason を必須で ledger に記録する（下流 SSoT への参照は埋め込まない。承認記録は front-matter refs のまま重複させない）裁定"
    candidate_selection: "from:[diagnoser-writes,orchestrator-invoked-atomic-command] chose:orchestrator-invoked-atomic-command"
    status: proposed
  - id: D7
    user_decision_ref: "chat_segment:session-01XgEf2rrfSL8jqSD9F4KRmn:2026-07-15 後続 cite の ADR は cite 時点で orchestrator が刻印し、未刻印の cite ADR は gate が fail-closed で block する簡約への同意 + 同日追補: 未刻印 ADR を track 中に改変してから cite させると working tree 刻印で洗浄される穴の指摘を受け、cite 刻印の複製元を分岐点 committed 文面に限定し、分岐点に無い ADR の cite は fail-closed とする修正 + 同日追補: track 中の新規 ADR の承認経路（user 同席 hearing→promote → working tree 刻印 reason: new-adr。承認記録は新 ADR の front-matter refs = chain ⓪ の守備範囲に一本化し ledger に重複させない）の確定 + chat_segment:session-01XgEf2rrfSL8jqSD9F4KRmn:2026-07-16 新 ADR の起草を user 同席 hearing に限定せず、pipeline の自発起草も kind: new-adr + reason 必須の正規経路に含める（自発起草は chain ⓪ の 🟡 評価と strict merge gate が user の非同期裁定を強制する）裁定 + 同日追補: 本機構は user の承認成果物の保護を目的とし user 承認前の新規成果物の変化を遮らないという原則の明文化 — 自発起草 ADR は user 裁定まで刻印せず凍結対象外の draft とし、裁定時に refs 昇格 → 刻印で凍結域に入れる裁定"
    candidate_selection: "from:[gate-prestep-auto-stamp,cite-time-stamp-with-gate-backstop] chose:cite-time-stamp-with-gate-backstop"
    status: proposed
  - id: D8
    user_decision_ref: "chat_segment:session-01AiXA34wxdwHFBKQzxiNDmG:2026-07-14 これは精緻化ではなく user 決定からの逸脱です、という裁定（agent-dispatch-cost-reduction track の増補全棄却） + chat_segment:session-01XgEf2rrfSL8jqSD9F4KRmn:2026-07-15 reviewer prompt に「初期案から意味を変えるべからず」を常設で差し込む指示"
    candidate_selection: "from:[silent-in-place-fix,amendment-proposal-lane,block-all-adr-findings] chose:amendment-proposal-lane"
    status: proposed
---
# ADR baseline の累積刻印とバイト照合 binary check による無断改変検出

## Context

agent-dispatch-cost-reduction track (2026-07-13) の ADR-baseline review で、review fixer が既存の `user_decision_ref` の傘の下で Decision 本文の意味論を大幅に増補し（execution-contract fingerprint、build-input closure、rustdoc snapshot 検証等）、全 gate を通過して commit まで到達した。user はこれを「精緻化ではなく user 決定からの逸脱」と裁定し、全増補の棄却と ADR 完全復元・下流 SoT 再整合を要した。同型の事故は他セッションでも再発している。

機構上の穴は二つ:

1. chain ⓪（adr_user）の信号評価は front-matter の ref 種別の自己申告である（`2026-04-27-1234-adr-decision-traceability-lifecycle.md` D1: `user_decision_ref` あり → 🔵）。被監視者が信号の入力を自己申告する構造のため、既存 ref の傘の下での本文乖離を検出できない。
2. reviewer の正当な設計指摘（例: hash 入力の網羅性懸念）に対し、本文を直接編集する以外の正規の出口が存在せず、zero_findings への圧力が本文改変に向かう。

利用できる性質: ADR の不意の編集は track 開始後にしか起こらない。track 開始前の ADR 編集は定義上 user 同席の pre-track authoring（hearing → 起草 → promote）である。また track 内での正規の ADR 編集（Phase 1-3 の grounding escalation による adr-editor の修正）は存在するため、凍結はこの正規経路を殺してはならない。

本 ADR は再設計版である。初版ドラフトは record チェーン・hunk 単位 user 裁定・信号機の評価器置換まで含む機構を設計したが、user が目的に対して過大と裁定し（2026-07-15）、機械検出と自動トリアージだけを残す最小機構に縮小した（Rejected D）。

## Decision

### D1: track が対象とする ADR の逐語複製を track 配下に刻印する

track が対象とする ADR について、「user が最後に承認した文面」の逐語複製を `track/items/<id>/adr-baseline/` 配下に baseline として刻印し、以後の無断改変検出（D4）と復元（D5）の錨とする。ADR の不意の編集は track 開始後にしか起きないため、track 開始時点と正規編集の時点の文面を track に固定すれば足りる。track 中の正規経路の再刻印（D5 の非意味的再刻印・D6 の escalation 刻印）を経た baseline は「正規経路で編集され merge 時の user 監査を待つ文面」であり、承認の最終確定は merge（PR review での user 裁定）が担う。

hash 一覧のみの保持ではなく逐語複製とするのは、(a) 照合をバイト等価に還元して散文パースと LLM を判定経路から排除するため、(b) 逸脱検出時の復元を機械コピーで済ませるため。逐語複製は意味的に乖離し得ない machine-written read-only record であり、no-upstream-restatement が禁じる「言い換えによる再記述」には当たらない。

### D2: 初回刻印は track init と別コマンドで行い、対象 ADR は orchestrator が渡す

初回刻印は、track 骨格の生成（`bin/sotp track init`）とは混ぜず、独立した機械コマンド — D6 の `bin/sotp track adr-baseline snapshot` — が行う。`/track:init` workflow が Phase 0 で二つを順に呼ぶ: `track init`（track directory / metadata.json / branch の生成）→ `adr-baseline snapshot --kind init --adr <file>`（主対象 ADR の刻印）。

どの ADR を track の対象とするかは文脈依存（adr2pr の入力、user の指示）であり、機械導出しない。orchestrator がそれを判断し、ADR の **file 名**を引数としてコマンドに明示的に渡す。ADR は `knowledge/adr/` 直下にのみ存在するため、コマンドは file 名から複製元を一意に解決でき、`knowledge/adr/` 外の file を刻印対象にできない（source domain の制限）。

### D3: baseline は累積とし、複製と ledger は追記のみとする

刻印のたびに新しい複製 file `track/items/<id>/adr-baseline/<slug>.<hash8>.md` を追加し、`adr-baseline/ledger.jsonl` に `{source, hash, kind, reason?, timestamp}` を追記する（source は `knowledge/adr/` 直下の file 名。kind と reason は D6 で定める）。hash algorithm は SHA-256（ledger が完全 hash を保持し、file 名の hash8 は先頭 8 hex の可読ラベル）。刻印内容の完全 hash が既存 ledger entry と一致する場合、同一バイト列の複製 file は既に存在するため file の追加は行わず、ledger への追記のみ行う。内容が異なるのに file 名の hash8 prefix が衝突する場合は、一意になるまで prefix を延長する（ledger は常に完全 hash を保持する）。凍結対象の記録は ledger が保持するため、metadata.json への pointer field は導入しない（`2026-06-10-1335` Rejected B が却下済みの anti-pattern）。

複製と ledger は**追記のみ**であり、既存の複製 file を上書き・削除する操作は存在しない。正規経路の全版が逐語のまま累積するため、後段の判定誤りや二重違反があっても監査線そのものは破壊できない（tamper-evident）。書き手は刻印機構（機械処理）のみで、いかなる writer capability の書込 lane にも属さない。

### D4: 刻印済み ADR のバイト照合 binary check を review 入口・commit gate・CI で行う

check の内容: ledger に刻印済みの各 ADR について、現在の file hash を ledger の最新 hash と照合する（一致 → 通過、不一致 → block し D5 のトリアージへ）。あわせて、track が対象とする ADR の刻印を要求し、欠落があれば fail-closed で block する — commit gate では spec.json の ADR 参照（存在すれば。JSON から機械導出）がすべて刻印済みであること、review 入口では主対象 ADR の init 刻印が ledger に存在すること（init 刻印忘れの検出）。track が cite しない ADR は要求対象に含めない（Neutral の write-scope 可視性に委ねる）。例外は track 生まれの draft ADR — track 分岐点に存在せず front-matter に user_decision_ref を持たない ADR（いずれも機械判定可能）は、user 裁定まで刻印要求の対象外とする（D7）。user_decision_ref を持つに至った track 生まれの ADR は、刻印済みでなければ block する（昇格後の刻印忘れの遮断）。

発火点は三つ置く。**review 入口**（pre-review task-contract gate と同族の binary 前提）では、init 刻印忘れが fixer の走る前の最初の review で loud に発覚する — この時点の working tree はまだどの capability も触れていないため、その場の刻印で安全に回復できる。**commit gate** は、review 中の fixer 編集を含むそれ以降の乖離を捕捉する。**CI（track-aware CI の task 群 = ci-track 系）**にも同じ check を組み込む — この task 群は local の `cargo make ci` と PR 上の GitHub CI の双方で同一に走り、`/track:merge`（wait-and-merge）は PR CI の成功を待つ。commit gate は local hook 経路であり、外部 subprocess 内の直接 git 操作には効かない（既知の hook coverage の穴）ため、PR 上の CI 実行が hook 迂回 commit の merge 前 backstop になる。

これは `task-contract check` と同類の **binary check** であり、SoT Chain の信号機（chain ⓪ の評価機構、signal-gates.json の strictness cells）には一切手を触れない。判定経路に散文パースも LLM も入らない。

### D5: 不一致は adr-diagnoser がトリアージし、非意味的編集のみ再刻印を許す

block 後、orchestrator は read-only の adr-diagnoser capability を dispatch し、baseline 最新版との diff を判定させる:

1. **非意味的編集**（誤字・整形・参照 path 修正等）→ 再刻印 verdict → orchestrator が D6 の snapshot を実行 → 発火元の check を再試行
2. **意味に触れる変更** → 内容の良し悪しを問わず一律「逸脱」→ 最新複製から機械復元し、編集元 capability に経緯を briefing 注入して差し戻す。正当な設計指摘は amendment 提案として報告させる（D8）。編集元を特定できない場合（中断セッションの残骸等）は復元のみ行い、経緯を observations.md に記録して続行する
3. **判断に迷う場合**は逸脱側に倒す（fail-closed）。人間への同期エスカレーションは pipeline を止めるため原則行わず、amendment 提案を PR review に載せて user が非同期に裁定する

復元も機械コマンド（`bin/sotp track adr-baseline restore --adr <file>`、仮称）が最新複製のバイト列をそのまま書き戻す。これは `knowledge/adr/` の file への書込だが、正規経路で刻印済みの baseline バイトの機械コピーであり、writer capability の書込 lane には属さない（D1 の刻印機構と同じ整理）。

誤って再刻印された意味変化も隠蔽は不可能である（D3 の累積）。新複製の追加は PR diff に必ず現れ、user は merge 前に「init 版 vs 最終版」の diff 一発で監査できる。誤判定のコストは merge の汚染ではなく下流手戻りに限定される。adr-diagnoser は `.harness/config/agent-profiles.json` に判定系 capability（read-only sandbox）として追加する。

### D6: baseline への書込は orchestrator が起動する原子的機械コマンドに限る

`bin/sotp track adr-baseline snapshot --adr <file> --kind <enum> [--reason <text>]`（仮称）が、新複製 file の追加と ledger 追記を 1 コマンドで原子的に行う。初回刻印（kind: init / cite / new-adr）も編集後の再刻印（kind: non-semantic-fix / escalation）も同一コマンドの呼び分けであり、kind はこの 5 値の enum とする。意味を持ち込む刻印（kind: escalation / new-adr）では `--reason` を必須とし（欠落は fail-closed）、何が欠けていて何が必要だったから ADR を書き換える／新設するに至ったかを、下流 SSoT への参照に依存しない自己完結の散文で ledger に記録する。他の kind では `--reason` を受け付けない。承認の記録は従来どおり ADR 自身の front-matter refs が担い（D7）、reason はそれと重複しない欠落説明である。baseline への書込面はこの 1 コマンドに集約される。複製元は kind が決める: init / new-adr は working tree、cite は track 分岐点の committed 文面（D2 / D7）、再刻印 2 種は working tree（編集後の現在文そのもの）。diagnoser を含む判定系 capability は read-only であり verdict を返すのみ。orchestrator も複製や ledger を手作業では書かない。

正規の ADR 編集 — Phase 1-3 の grounding escalation で orchestrator 自身が adr-editor に指示した編集 — は「予期された編集」であり、diagnoser を経由せず編集直後に snapshot を実行する。D5 のトリアージは予期しない不一致の専用経路である。

### D7: 後続 cite の ADR は cite 時点で刻印する

spec.json が後続 Phase で別 ADR を新たに cite した場合、orchestrator がその file 名を渡して同じ機械コマンドで刻印する（kind: "cite"）。複製元は working tree ではなく **track 分岐点（configured base との merge-base）の committed 文面に限定する**。未刻印の ADR は D4 の照合対象外であり、cite 前に加えられた無断改変を working tree ごと刻印すると baseline に洗浄されてしまう — 分岐点刻印なら、改変された working tree は次の commit で baseline と不一致になり D5 のトリアージが捕捉する（cite が保護域への入口となり、track 開始以降の改変を遡って検出できる）。

分岐点に存在しない ADR（track 中に新規作成された ADR）が生まれる経路は二つあり得る: user 同席の hearing を経た起草（pre-track と同じ儀式の track 中実施）と、pipeline が必要と判断した自発的な起草である。本機構が守るのは user の承認成果物であり、user 承認前の新規成果物の変化を遮らない: 自発起草の ADR は user 裁定が届くまで刻印せず、凍結対象外の draft として正規の編集 lane で自由に改稿できる。その間の管轄は信号機側にある — 非 user 系の根拠（review_finding_ref 等）に留まる限り chain ⓪ が 🟡 と評価し、merge gate（strict）が merge 前の user 裁定を機械的に強制する。新しい decision は定義上 user の裁定を要するが、その裁定は同期である必要はない。user 裁定が届いた時点（同席 hearing では即時、自発起草では PR review 等の非同期裁定時）で、orchestrator が根拠を user_decision_ref へ昇格させ、その直後に working tree から刻印する（kind: "new-adr"、reason 必須 — D6）— 以後この ADR は通常の凍結対象になる。昇格後の刻印忘れは D4 が fail-closed で block するため、凍結域に入らないまま merge に到達する漏れはない。承認の記録は新 ADR 自身の front-matter refs が担い、既存の chain ⓪ 検査（本 ADR が触れない信号機）が従来通り検証する — ledger 側に承認記録を重複して持たない。刻印の可視性は累積複製の PR diff と merge 前の user 監査が担保する。

### D8: reviewer / fixer の briefing に ADR 意味凍結の常設規定を置く

ADR scope の reviewer briefing と fixer 系 capability の運用契約に常設で記載する: 「ADR の意味論を baseline（初期案）から変えてはならない。意味に関わる指摘は in-place 編集ではなく amendment 提案として報告する。」非意味的修正は D5 の再刻印レーンで通る。decision record の完全性基準は「決定を忠実に記録しているか」であり「設計として完全か」ではない。docs 規定単独では圧力下の逸脱を防げない（本事故が実例）ため、本規定は D4/D5 の機構の上に重ねる緩和層である。

## Rejected Alternatives

### A. front-matter 内に decision 本文 hash を刻印する

刻印が守るべき file の内部にあるため、ADR を編集する agent が hash も再計算でき、自己証明にしかならない。track 配下への分離は隠蔽に二重の lane 違反を要求し、write-scope 外 diff として必ず可視化される。却下。

### B. chain ⓪ を LLM 意味検証にする

user 決定は repo 外（chat）にあり、in-repo の検証器は意味照合の照合先を持たない。LLM 照合はコストと不安定性も伴う。忠実に検証可能な唯一の錨は「正規経路で刻印された文面」であり、バイト照合が同じ事故をより安く確実に捕まえる。却下。

### C. 複製 + briefing 規定のみ（gate なし）

検出が人の diff 確認に依存する。同種の禁止規律は事故当時も存在したが圧力下で破られており、enforce-by-mechanism に従い検出だけは機械の gate（review 入口 / commit / CI）に置く。複製と briefing 規定自体は本設計に含まれる（D1 / D8）。却下。

### D. record チェーン + hunk 単位 user 裁定 + 信号機の評価器置換（初版ドラフト）

編集 record の hash チェーン、unified diff hunk 単位の user 承認/棄却/保留の儀式、chain ⓪ の評価器を自己申告から凍結判定へ差し替える設計まで含む完全版。無断改変の遮断という目的に対して機構が過大であり（workflow-ceremony-minimization）、部分裁定の粒度・増補 grounding の機械監査・敵対的二重違反への耐性はいずれも実証された需要がない。user 裁定（2026-07-15）により却下。設計記録は起草ドラフトとして保存し、Reassess When の条件で再訪する。

### E. 最新 1 枚を置換する複製方式

上書き可能な baseline は、誤判定・二重違反の際に監査線ごと消える。累積方式なら証拠の破壊が構造的に不可能で、コストは track 配下の数 KB × 版数に過ぎない。却下。

### F. track 外を含む全期間の ADR 凍結

track 外の ADR 編集は定義上 user 同席の pre-track authoring であり、守る必要のある autonomous 実行期間は track 内に限られる。却下。

## Consequences

### Positive

- 無断の ADR 改変が最初の commit で機械的に block される（本事故は Step 2-4 の baseline commit 時点で検出されていた）。
- 逸脱検出時の復元が最新複製からの機械コピーで済み、adr-editor dispatch を要しない。
- 非意味的修正は diagnoser トリアージで autonomous に流れ、人間の同期介入なしに pipeline が進む。
- 累積複製 + ledger が tamper-evident な監査線となり、PR review で init 版 vs 最終版の diff 一発監査ができる。merge がそのまま user 裁定になり、儀式の追加はゼロ。
- gate の判定経路に LLM も散文パーサも入らない。LLM（diagnoser）は block 後のトリアージに限定され、その誤りも隠蔽不能。

### Negative

- track/items 配下に複製が版数分累積する（数 KB × 版数/ADR）。
- 正規の ADR 編集（escalation）に snapshot 一手間が追加される。忘れると次の commit が block される（fail-closed 側に倒れる）。
- diagnoser が意味変化を誤って再刻印した場合、merge 前まで通過し下流 SoT の手戻りコストになり得る（隠蔽はされないため PR review で必ず捕捉可能）。
- 並行 track が同一 ADR を正規に修正した場合、他方の track で不一致として可視化される（再確認を強制する望ましい副作用だが、単独開発でも手間は増える）。

### Neutral

- chain ⓪（adr_user）の信号評価機構には手を触れない。ref 種別自己申告という構造的弱点は Context に記録し、解消は将来の再訪対象とする。
- pre-track ADR authoring / promotion 儀式は変更しない。
- 対象は track が cite する ADR に限る。無関係 ADR の in-track 編集は write-scope 外 diff としての可視性に委ねる。

## Reassess When

- in-track の ADR 増補が頻発し、複製単位の刻印では裁定粒度が粗くなったとき（初版ドラフトの hunk 単位裁定・record チェーンを再訪）。
- diagnoser の誤った再刻印が実測で問題化したとき（非意味的判定の機械化、verdict の抜き取り監査を検討）。
- chain ⓪ の自己申告弱点を機構で塞ぐ需要が実証されたとき（信号機の評価器を凍結判定へ差し替える設計を再訪）。
- 並行 track 間の同一 ADR 修正が実測で頻発し、cross-track 通知の仕組みが必要になったとき。

## Related

- `knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md` — chain ⓪（ref 種別自己申告）の原典。本 ADR が Context で記録した検出不能の穴の出所
- `knowledge/adr/2026-06-16-1030-signal-gate-strictness-config.md` — chain × gate strictness の SSoT（本 ADR は信号機に不干渉）
- `knowledge/adr/2026-06-11-1018-spec-ref-embedded-hash-removal.md` — SoT 内 hash 埋込の禁止。hash を track 側 ledger に置く本設計は準拠
- `knowledge/conventions/pre-track-adr-authoring.md` — pre-track ADR 起草規約
- `knowledge/conventions/adr.md` — ADR front-matter 規約
- `knowledge/conventions/review-protocol.md` — reviewer capability cycle
- `knowledge/conventions/no-upstream-restatement.md` — 逐語複製と言い換え再記述の区別
- `knowledge/conventions/enforce-by-mechanism.md` — 機構 > 規定の優先順位
- `knowledge/conventions/workflow-ceremony-minimization.md` — 初版ドラフトからの機構縮小の判断根拠
- `.harness/custom/review-prompts/` — ADR scope reviewer briefing（D8 の反映先）
