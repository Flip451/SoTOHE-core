---
adr_id: 2026-06-26-0503-adr2pr-back-and-forth-skill-definition
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session-track-cli-layers-adr2pr-debrief:2026-06-26"
    candidate_selection: "from:[A,B,C,D,E,F] chose:adopt-/track:diagnose"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session-track-cli-layers-adr2pr-debrief:2026-06-26"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session-track-cli-layers-adr2pr-debrief:2026-06-26"
    candidate_selection: "from:[hard-block,soft-prompt] chose:soft-prompt"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:session-track-cli-layers-adr2pr-debrief:2026-06-26"
    candidate_selection: "from:[5-class,3-class-collapsed,4-class-no-impl] chose:5-class-with-impl"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:session-track-cli-layers-adr2pr-debrief:2026-06-26"
    candidate_selection: "from:[regex-rule,llm-semantic] chose:llm-semantic"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:session-track-cli-layers-adr2pr-debrief:2026-06-26"
    status: proposed
  - id: D7
    user_decision_ref: "chat_segment:session-track-cli-layers-adr2pr-debrief:2026-06-26"
    candidate_selection: "from:[skill-chains-writers,orchestrator-dispatches] chose:orchestrator-dispatches"
    status: proposed
  - id: D8
    user_decision_ref: "chat_segment:session-track-cli-layers-adr2pr-debrief:2026-06-30"
    status: proposed
  - id: D9
    user_decision_ref: "chat:2026-06-29:adr2pr-baseline-blocker-resolution"
    candidate_selection: "from:[patch-gate,bundle-commits,stub-contract,abort] chose:patch-gate"
    status: proposed
---
# impl 段階の構造的不整合検出時のフェーズ遷移診断スキル

## Context

`/track:adr2pr` は ADR を起点とする 10-step thin-orchestrator skill であり、次の順で実行される。

1. **step 1** — `/track:init` (= Phase 0): track 初期化
2. **step 2** — `/track:review`: ADR baseline をレビュー
3. **step 3** — `cargo make add-all`: ADR + metadata を staging
4. **step 4** — `/track:commit`: first commit (ADR baseline)
5. **step 5** — Phase 1-3: `/track:spec-design` + `/track:type-design` + `/track:impl-plan` を順次起動
6. **step 6** — `/track:review`: plan artifacts をレビュー
7. **step 7** — `cargo make add-all`: plan artifacts を staging
8. **step 8** — `/track:commit`: plan artifacts commit
9. **step 9** — `/track:full-cycle`: implement → review → commit
10. **step 10** — `/track:pr-review`: 最終 PR-based レビュー

各 phase は SoT Chain (① spec → ADR / ② impl → spec / ③ catalogue → impl) の signal (🔵 / 🟡 / 🔴) で検証され、🔴 が出た場合は back-and-forth escalation で上流 writer (adr-editor / spec-designer / type-designer / impl-planner) を再呼び出しする。

しかし、現状の skill 定義には次の gap がある。

**review で surface した構造的不整合を、orchestrator が場当たり的に分類している。** 構造的不整合の検出機構は段階ごとに用意されており、SoT Chain の 3 つの edge 上の食い違いは内部 signal (`bin/sotp signal calc-spec-adr` / `calc-catalog-spec` / `calc-impl-catalog`) が機械検出し、計画文書 (plan artifacts) の構造的不整合は step 6 `/track:review` が reviewer 経由で検出する (通常はこの段階で impl 着手前に解消される設計)。それでも step 6 で捉えきれない不整合 — 例えば ADR / spec に明示記述のない暗黙原則違反や、同じ症状が複数 phase に原因を持ちうる不整合など — は step 9 `/track:full-cycle` の impl 後 review か step 10 `/track:pr-review` の外部 reviewer 指摘で surface する。**いずれの段階 (step 6 / step 9 / step 10) で surface しても、「どの phase に戻って直すか」 (新 ADR を起こす / spec を改訂 / impl-plan task 記述を直す / source 側だけ直す) を一意に決めるための structured な mechanism がなく、orchestrator は ADR / impl-plan / source を読み比べて意味的に分類するしかない。** 典型例:

- **例 (a): PR review (step 10) で外部 reviewer が指摘した暗黙原則違反。** hexagonal purity / SOLID 等の原則は ADR / spec に明示記述されていない場合が多く、internal signal では検出不能。reviewer 指摘から「ADR に原則を明文化すべきか」「impl が原則違反しているのを直すべきか」「既存 convention を引用するだけで済むか」を分類する必要がある (D4 routing で言えば `adr` / `impl` のどちらに送るかの判別)。
- **例 (b): 同じ指摘メッセージが複数の段階に原因を持ちうるケース。** たとえば「実装が ADR で定めた hexagonal レイヤ方針と食い違っている (usecase に I/O が混入しているなど)」と指摘されたとき、その原因として少なくとも次の 3 通りが考えられる: (i) ADR は方針を決定済みだが、impl-plan のタスク記述が ADR と異なる配置方針を前提に書かれていた、(ii) ADR / impl-plan の方針は正しいが、実装担当がレイヤ境界を理解せず誤って I/O 呼び出しを usecase に書いた、(iii) ADR の方針表現が曖昧で複数解釈可能だったため、impl-plan / 実装が独自解釈した。指摘メッセージだけからは「どの段階に戻って直せばよいか」を一意に決められず、診断スキルが ADR / impl-plan / 該当 source を読み比べて意味的に判定する必要がある。

直近 1 PR cycle の事例では、Codex Cloud reviewer 往復 8 rounds のうち 5 rounds がこのカテゴリ (内部 signal で検出できない構造的不整合) の修正だった。なお、D4 の `impl` category は本 Context の例 (a)(b) には現れないが、これは他 4 category (adr / spec / type / impl_plan) と並ぶ対等な分類であり、「設計文書遡及は不要、source 側で契約違反を直すべき」と積極的に判定する独立の routing 先である。

本 ADR では、**impl 段階以降で impl-plan の実装計画と食い違う 🟡 / 🔴 が surface した時** に発火する **phase rollback 診断 skill** の導入とその仕様を判断する。具体的な主 trigger は、impl-plan の各 task が宣言した型契約 (`task-contract.json` の `TaskContractDocument` における `ContractedEntryRef`) を impl が満たしたかを review 直前に検査する `bin/sotp task-contract check` (PR #175 で導入される PreReviewGate) が `PreReviewGateOutcome::Blocked` を返した時である。Blocked が出るのは、task が「自分が 🔵 化する」と宣言した型エントリが impl 完了後も 🟡 / 🔴 のまま残っているケースで、これは「実装が設計を歪めた」事故の最も典型的な signal となる。診断 skill の本質的価値は、この Blocked を起点に「契約未充足の原因」を 5 categories (adr / spec / type / impl_plan / impl) のどれに routing すべきかを semantic に判定し、orchestrator の場当たり判断を構造化することにある。

## Decision

### D1: `/track:diagnose` (仮称) を新設する

impl 段階以降で impl-plan の実装計画と食い違う 🟡 / 🔴 が surface した時に発火し、rollback 先 phase と次のアクションを返す診断 skill を新規定義する。主 trigger は `bin/sotp task-contract check` (PR #175 で導入される PreReviewGate) が `PreReviewGateOutcome::Blocked` を返した時。`/track:plan` 内の既存 Phase 1-3 loop はそのままに、impl phase 由来の遡及だけを別 skill に分離することで、orchestrator が「どこに戻るか」を ad-hoc 判断する負担を構造的に減らす。

### D2: trigger 範囲は PreReviewGate Blocked + step 6 plan-artifacts review findings の 2 経路を主軸とする

`/track:diagnose` の routing 対象 trigger は次の 2 つを主とする。

1. **PreReviewGate Blocked** (step 9 `/track:full-cycle` 中): PR #175 で導入される `bin/sotp task-contract check` (Makefile.toml の `track-local-review-fix-codex` task で `bin/sotp review fix-local` の前段に接続) は、impl-plan の各 task が `task-contract.json` の `TaskContractDocument` 内 `ContractedEntryRef` で宣言した型エントリが impl 後に 🔵 に到達したかを検査する。到達していない (= attributed entries が 🟡 / 🔴 残存) 場合は `PreReviewGateOutcome::Blocked` が返され、レビュー開始が妨げられる。診断 skill はこの Blocked 時の blocked entries (どの layer のどの entry が 🔵 化されなかったか) を読み取り、原因を意味的に分類する (D4 5 categories のうち `type` / `impl_plan` / `spec` / `adr` / `impl` のいずれに routing するか)。本 trigger は PR #175 が main にマージされた時点で有効化される。
2. **step 6 plan-artifacts review findings** (`/track:review` plan-artifacts scope): reviewer 指摘 / signal 🔴 等で surface した構造的不整合。

加えて step 10 (`/track:pr-review`) の Codex Cloud / 外部 reviewer 指摘は本 ADR の主軸 trigger ではないが、orchestrator が手動で診断スキルに渡すことは妨げない (本 Context 例 (a) はこの経路の典型)。

### D3: invocation enforcement は PreReviewGate 出力内 soft prompt を採用する (専用 hook は不要)

`bin/sotp task-contract check` (PreReviewGate) が `PreReviewGateOutcome::Blocked` を返す際の stderr / 出力に **`/track:diagnose` の呼び出しを推奨する soft prompt** を含める。orchestrator は Blocked exit code とこの prompt を見て、`/track:diagnose` を invoke するか自前で原因分類するかを判断する。新たな PreToolUse hook や lock file / bypass 機構は導入しない。

採用根拠:
- **enforcement の主構造は既に PR #175 の PreReviewGate が担保**: Blocked が出れば Makefile.toml の `track-local-review-fix-codex` chain 上で `bin/sotp review fix-local` まで到達せず exit 1 で止まる。orchestrator は次のアクションを取る必要に必然的に直面するため、追加の hook で edit を intercept する必要がない。
- **soft prompt は workflow brick リスクゼロ**: 既存 PreReviewGate の出力に文字列を 1 行追加するだけで、追加の機構 (PreToolUse hook / writer subagent 用 bypass lock / pid 検証 等) を一切持ち込まない。診断 skill / writer subagent / `/adr:add` 等が自前で edit する経路にも影響しない。
- **soft → hard 化は容易だが逆は難**: 運用上 soft prompt を無視する事例が累積したら、orchestrator 側 skill (`/track:full-cycle`) に「Blocked を受けたら必ず `/track:diagnose` を invoke」の規約を追加することで hard 化できる。逆 (hard → soft) は既存機構の bypass 経路が積み上がった状態からの後退になり困難。

### D4: routing 出力カテゴリは 5 クラス (adr / spec / type / impl_plan / impl) とする

診断 skill は次の 5 categories のいずれかを返す。

| target | 意味 | 対応 writer / アクション |
|------|------|------|
| `adr` | ADR に未記載の決定が必要 | `/adr:add` で新 ADR、または `adr-editor` で既存 ADR D 改訂 |
| `spec` | ADR 決定はあるが Phase 1 spec author が表現し切れていない | `spec-designer` を再呼び出し (Phase 1 partial re-entry) |
| `type` | catalogue にアーキテクチャなどの観点から瑕疵がある (= 役割配置 / シグネチャ / impl 宣言などが設計上問題) | `type-designer` を再呼び出し (Phase 2 partial re-entry) |
| `impl_plan` | ADR / spec / catalogue は正しいが impl-plan task 記述に欠落 / 誤りがある | `impl-planner` を再呼び出し (Phase 3 partial re-entry) |
| `impl` | 設計文書側の問題ではなく実装の契約違反 | 設計文書には触らず source 側で修正 (型カタログの責任ではなく実装の責任である旨を明示) |

`impl` カテゴリは「out_of_scope」 (=何もしない) ではなく、「実装が設計契約を破っているという診断結果」を伝える明示的な出力である。

### D5: routing 判断は LLM の意味判断に委ねる (vs regex / keyword matching)

`/track:diagnose` の routing は呼び出された LLM が 🔴 signal text / reviewer 指摘 text / 関連ファイル (spec.json / `<layer>-types.json` / impl-plan.json / 該当 source / `*-signals.json`) を読み取って **意味的に判断** する。

regex / keyword ベースの決定的ルール (例: signal text に "forbidden_roles" を含めば `adr` に routing 等) は採用しない:
- 同じ keyword が複数 phase 由来の構造的不整合に現れうる (例: `ApplicationService` は ADR / spec / impl-plan / lint config いずれにも出現)
- 新 check 追加のたびに rule table を更新する保守コストが累積する
- LLM が SoT 階層を理解した上で判断する方が、文脈に応じた precise な routing になりやすい

### D6: 診断 skill の出力スキーマを規定する

`/track:diagnose` は次の 3 フィールドを構造化して返す。

| field | type | 意味 |
|------|------|------|
| `routing_target` | enum (`adr` / `spec` / `type` / `impl_plan` / `impl`) | D4 の 5 カテゴリのいずれか |
| `reason` | string (日本語) | 判断根拠 (どの 🔴 / 🟡 signal / 構造的不整合を見て、なぜそのカテゴリと判定したかの semantic summary) |
| `recommended_next_action` | string (日本語) | 呼び出し orchestrator が次に取るべき具体的アクション (例: `「/adr:add <slug> を起動して新ADRを起こす」`、`「adr-editor で 該当 ADR D の <field> を改訂」`、`「type-designer で <layer>-types.json の <type> エントリを修正」`) |

orchestrator は `routing_target` を読んで dispatch (D7) するか、`reason` をユーザに提示して判断を仰ぐ。

### D7: writer dispatch は orchestrator の責任 (skill は one-shot resolution しない)

`/track:diagnose` は D6 の出力を返すだけで、実際の writer (adr-editor / spec-designer / type-designer / impl-planner) 起動は呼び出し orchestrator が行う。診断と修正を別 skill に分離することで:

- 診断結果をユーザが review してから dispatch する余地を残す (誤判定時に orchestrator が override 可能)
- 診断 skill 自身が再帰的に writer を起動する複雑性 (chain orchestration) を避ける
- 既存の `/track:plan` の back-and-forth loop と同型 (loop 主体は plan、escalation 先は writer) で skill 設計の一貫性を保つ

### D8: routing 判断専用の capability を `.harness/config/agent-profiles.json` に定義する

D5 で routing 判断を LLM の semantic 判断に委ねることを決定したが、その LLM の provider / model 選択を ad-hoc にしない。`.harness/config/agent-profiles.json` に新規 capability `rollback-diagnoser` (仮称) を追加し、`/track:diagnose` 内部の LLM 呼び出しは `capabilities.rollback-diagnoser.provider` / `capabilities.rollback-diagnoser.model` を resolve した上で dispatch する。

採用根拠:
- **既存 capability アーキテクチャとの一貫性**: `spec-designer` / `type-designer` / `impl-planner` / `adr-editor` / `implementer` / `reviewer` / `researcher` / `dry-fix-lead` / `review-fix-lead` 等の writer / verifier capability が既に `agent-profiles.json` で provider 解決される設計に揃える (`.claude/rules/08-orchestration.md` 参照)。
- **provider / model 切替の容易性**: 運用上 routing 判断の品質が provider 依存だと判明した場合、capability の `provider` / `model` を差し替えるだけで対応可能 (本 ADR 本文を再改訂する必要がない)。
- **telemetry の独立性**: 専用 capability にすることで token / コスト / 誤判定率を他 capability と分離して計測でき、Reassess When で論ずる「routing 性能を再評価する」フィードバックループの基準にできる。

capability 名は仮称 `rollback-diagnoser`。`/track:diagnose` skill 起動時に capability resolve → provider dispatch → D6 の出力契約 (routing_target / reason / recommended_next_action) を取得 → orchestrator に返す (D7 の dispatch は orchestrator 側)。

### D9: PR #175 PreReviewGate を欠損 `task-contract.json` に寛容にする (precondition hotfix)

本 ADR D1-D8 を実装する `/track:adr2pr` 起動時、Phase 0 baseline commit ステップ (Step 2 `/track:review` → Step 4 `/track:commit`) が PR #175 で導入された PreReviewGate (`bin/sotp task-contract coverage` / `bin/sotp task-contract check`、`cargo make ci-track` 経由) によって fail-closed する事象が観測された。原因は、impl-plan / catalogue / task-contract がまだ存在しない pre-impl phase の入力欠損を「契約未提出」と解釈し、レビュー / コミット gate が blocked と扱う設計だった。Phase 0 では task-contract artifact は未生成であるため、この入力欠損は契約違反ではなく「評価対象なし」として扱う必要がある。

この事象自体が本 ADR D1-D8 で定義する `/track:diagnose` の存在意義 (impl 段階以降で surface する構造的不整合の routing) を裏付ける meta-circular な実例であり、D4 5 クラス routing でいえば `impl` (gate の実装が Phase 0 baseline 案件を未対応) または `impl_plan` (PR #175 の impl-plan が Phase 0 案件を未列挙) に分類される。

precondition として、PR #175 PreReviewGate を欠損 `task-contract.json` に寛容にする bootstrap hotfix を Phase 0 baseline gate の再実行前に成立させる。この hotfix は blocked baseline commit の後続 implementation task として schedule しない。base branch または現在の working tree に欠損 `task-contract.json` を許容する gate 挙動が既に存在する状態で、Step 2 `/track:review` → Step 4 `/track:commit` を再実行する。存在しない場合は、gate patch を separate pre-baseline change として先に適用してから ADR baseline review / commit を再開する。

- PreReviewGate と coverage verification の双方で、`task-contract.json` が存在しない pre-impl phase は `Blocked` ではなく `Passed` として扱う。
- `task-contract.json` が存在する場合の挙動 (entries の coverage / impl_catalog の liveness 評価) は不変。fail-open は作らない。
- 兄弟 gate の寛容化 ADR (`2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact` / `2026-06-01-0406-review-gate-tolerate-missing-catalogue`) と同型の precedent pattern を踏襲する。
- 「impl-plan が存在するのに task-contract が無い」等の将来の精緻化に備え、missing-task-contract violation の概念自体は残す。
- D9 hotfix の実装において `pre_review_gate.rs` を修正した結果、モジュールが `verify-module-size` ゲートの 700 行上限 (ADR `2026-06-06-1609-enforce-module-size-limit-splitting`) を超過した。上限超過時は圧縮ではなく分割を選ぶ方針に従い、`libs/usecase/src/pre_review_gate/helpers.rs` サブモジュールを新設し、`pre_review_gate.rs` から 7 本以上の純粋ヘルパー free function を `pub(super)` で抽出することで親モジュールを 700 行上限内に収める。分割パターンは `ref_verify.rs + ref_verify/*.rs` のサブモジュール分割を踏襲する。

採用根拠:
- precedent ADR 2 件 (spec-states / review-gate) と同型の "gate を欠損入力に寛容にする" pattern。本ケースは 3 例目。
- blocked baseline gate の後続に置くと bootstrap deadlock になるため、ADR baseline review / commit の再実行前に成立している precondition として明示する必要がある。
- D9 を独立 decision として明示することで、track 完了時のレビューで「diagnose skill と別軸の gate behavior hotfix が同じ branch に含まれる理由」を Pull Request reviewer に説明できる。

## Rejected Alternatives

### A. Makefile.toml chain で PreReviewGate Blocked → `/track:diagnose` 自動 invoke を強制する hard enforcement

PreReviewGate (`bin/sotp task-contract check`) が Blocked を返した時に、Makefile.toml の `track-local-review-fix-codex` task が次の step として `/track:diagnose` を自動 invoke するよう chain を改造する設計案。orchestrator の意思を介さず Blocked → 診断 を mechanical に連鎖させる。

却下理由: (1) `/track:diagnose` は LLM semantic 判断を含む skill であり、cargo make task chain から `Skill` tool 経由で自動 invoke する仕組み自体が確立していない (skill invocation は orchestrator の context 内 tool 呼び出しが基本)、(2) 自動 invoke すると orchestrator が誤判定を override する余地が消える (D7 で分離した「診断 → dispatch は orchestrator 責任」の原則と衝突)、(3) Blocked 時に常に診断が必要とは限らない場合 (例: 明らかに source bug で orchestrator が即修正可能) も自動 invoke すると無駄な token / 時間を消費する、(4) PR #170 cycle の問題は本質的に分類フローの不在であり、enforcement の強度は二次的。soft prompt → hard 化は将来容易だが逆は難しく、まず soft で導入して運用 telemetry を蓄積するのが妥当。

### B. 診断 skill が writer を chain 起動する one-shot resolution

`/track:diagnose` 内部で routing 判定後に直接 `adr-editor` / `spec-designer` 等の writer subagent を呼び、設計文書修正まで完結させる設計案。

却下理由: (1) 診断と修正を 1 つの skill に詰め込むと skill の責任範囲が膨れて mental model が複雑化、(2) 誤判定時のユーザ介入余地が小さくなる、(3) 既存 `/track:plan` の Phase loop は「signal → 上流 writer escalation」を skill-orchestrator 階層で実装しており、`/track:diagnose` のみ chain 起動にすると skill 設計の一貫性が失われる。診断 skill は出力契約 (D6) だけ規定し dispatch は orchestrator に委ねる方が SOLID の SRP に整合する。

### C. regex / keyword matching による決定的 routing rule

signal text / reviewer 指摘 text / signal source file name / 修正対象 path などをキーに `if-else` の決定的 routing table を skill 内に持つ設計案。

却下理由: (1) 同じ keyword が複数 phase 由来の構造的不整合に現れうる (`ApplicationService` の例)、(2) 新 check 種類を追加するたびに table を増補する保守コストが累積、(3) signal source ファイル名で routing しても「spec を直すか catalogue を直すか」のような semantic 区別はファイル名から決定できない、(4) LLM が SoT 階層を semantic に理解した上で判断する方が文脈に応じた precise な routing になる。LLM 判断の品質ブレは telemetry で計測し、必要なら future ADR で rule-based hybrid を再評価する。

### D. 3-class taxonomy (adr / catalogue / out_of_scope) へのコラプス

設計文書遡及のモチベーションを `adr` (決定欠落) と `catalogue` (記載ミス) の 2 軸 + `out_of_scope` の 3 categories だけにする案。「中間 phase からの再入は ADR 決定不足の派生として扱う」というユーザの初期仮説に近い view。

却下理由: 「ADR 決定は完全にあるが Phase 1 (spec) / Phase 3 (impl-plan) author が表現しきれていない」ケース (例: ADR が「N 個の特定 pair について refactor を行う」と decided しているが spec が N-1 個しか列挙していない、impl-plan task の対象範囲が ADR D 全部をカバーしていない、等) は新 ADR が不要であり、spec-designer / impl-planner の partial re-entry で済む。これを `adr` カテゴリに折り畳むと不必要な ADR re-author が走り、cost が増す。`spec` / `impl_plan` を独立 category にすることで降下方向の incompleteness を新 ADR 不要で表現できる。

### E. 新 skill を作らず `/track:plan` の back-and-forth section を拡張する

`/track:plan` の "Phase N loop" 規約に impl 段階の構造的不整合 / 🟡 を起点とする rollback rule を追記し、独立 skill を立てない案。

却下理由: (1) `/track:plan` は phase 4 単位の plan 駆動 skill で、impl phase 中の ad-hoc な遡及 trigger を `/track:plan` が観測することはできない (orchestrator が `/track:plan` を呼ばないとループしない)、(2) `/track:plan` を impl 中に呼ぶことは canonical な flow から外れるため orchestrator の mental model が乱れる、(3) 診断と planning は責務が異なる (Phase の plan 自体は確定済み、構造的不整合由来の rollback を semantic に分類するのが診断の仕事) ので skill 単位を分けた方が SOLID SRP に整合。

### F. CLAUDE.md 規約記述のみ (hook も skill も作らない)

`.claude/rules/` に「impl 中は設計文書を触らない」「触る必要がある時は ADR や spec の修正動機を明文化する」と記述するだけの passive な案。

却下理由: 規約記述は orchestrator が context 中に保持し続ける前提に依存しており、長時間 / 多 tool call の session では LLM が許容量超過で forget する可能性が高い。enforcement はゼロ。本 ADR が解決しようとしている問題 (PR #170 cycle のような impl 中の ad-hoc 設計遡及) は規約記述だけでは構造的に防げない。

## Consequences

### Positive

- 設計文書遡及のモチベーションが 5 categories に意味的に分類され、orchestrator が ad-hoc に「どこに戻るか」を判断する負担が減る。PR #170 のような 8 rounds の review cycle が、phase rollback の明示化により短縮される可能性。
- soft prompt 採用により writer subagent / `/adr:add` / `/track:diagnose` 自身に bypass 機構を一切実装する必要がなく、workflow が brick するリスクがゼロ。
- 診断と dispatch が分離 (D7) されており、誤判定時のユーザ介入余地が残る。SOLID SRP との整合性が高い。
- routing 判断を LLM の semantic 判断に委ねる (D5) ことで、新 check 種類追加のたびに rule table を保守するコストが発生しない。
- D2 で PreReviewGate (PR #175 由来) と接続されることで、impl-plan task 単位での契約充足が構造化されたフィードバックとして orchestrator に戻る。

### Negative

- routing 判断品質が LLM 性能に依存。LLM のミス routing (例: `impl_plan` issue を `adr` に routing する誤判定) が発生した場合、orchestrator は誤った writer を起動し追加 round を消費する。telemetry での品質計測が必要。
- soft prompt のため LLM が `<system-reminder>` を無視して edit を続行する可能性がある。「無視されすぎ」事象が累積した場合は hard block への再評価 (Reassess When 参照) を要する。
- D2 の PreReviewGate trigger 部分は PR #175 で導入済みの仕様に依存しており、現在の baseline gate / review gate では既に active な前提として扱う必要がある。PR #175 仕様が変更された場合、D2 / D9 / `/track:diagnose` の trigger 条件を同期させる必要がある。
- 既存 PreReviewGate 出力への soft prompt 追加と新 skill (`/track:diagnose`) の追加により、実装 / メンテナンス コストと診断呼び出し時のレイテンシが新規に発生する。
- 診断 skill が呼ばれる頻度が高いと token budget が消費される。特に PR review 多ラウンドが続く track では cumulative cost に注意。

## Reassess When

- 本 ADR の soft prompt (PreReviewGate stderr 内の `/track:diagnose` 呼び出し推奨) が運用上無視された事例が **3 件以上** 蓄積された場合、orchestrator skill (`/track:full-cycle`) 内に「Blocked を受けたら必ず `/track:diagnose` を invoke」の規約を追加して hard 化する、または Alt A (Makefile.toml chain での自動 invoke) を再評価する。
- 診断 skill の routing 出力が LLM 性能要因で **誤判定された事例が複数累積** した場合、まず D8 の `rollback-diagnoser` capability の `provider` / `model` を変更して品質を再計測する。それでも改善しない場合は regex / keyword matching との hybrid (Alt C との折衷) を再考する。
- `/track:diagnose` の出力スキーマ (D6) と orchestrator dispatch (D7) の責任分界に運用上の問題 (例: orchestrator dispatch を skip する事例) が surface した場合、Alt B (skill が chain 起動) を再考する。
- **ref-verify が ③ impl-plan → ADR / ③' impl-plan → spec 方向をカバーするよう拡張された時点で、本 ADR の trigger 範囲を再評価。** 拡張により「Phase 1/2 で改訂された ADR / spec が Phase 3 impl-plan に伝播していない」型の食い違いが pre-impl で検出可能になれば、`/track:diagnose` の主要 trigger は外部 reviewer 指摘・複数 phase attribution・contract 違反 (本 Context 例 a/b) に絞られ、Context 動機の比重が変わる可能性がある。

## Related

- `knowledge/conventions/pre-track-adr-authoring.md` — pre-track ADR lifecycle と本 ADR の位置づけ (Phase 0 前段に置かれる ADR である)
- `knowledge/conventions/track-lifecycle.md` — track の Phase 0-3 と step 9 / step 10 の仕様
- `knowledge/conventions/branch-strategy.md` — track branch と back-and-forth の運用
- `knowledge/conventions/workflow-ceremony-minimization.md` — 人工的状態フィールド廃止と SoT Chain signal に基づく gate の原則
- `.claude/skills/` — 既存 `/track:plan` / `/track:full-cycle` / `/track:review` 等の skill 定義と新 skill (`/track:diagnose`) の配置先
- `.harness/config/agent-profiles.json` — capability resolution の SSoT (D8 で追加する `rollback-diagnoser` capability の配置先)
- `.claude/rules/08-orchestration.md` — capability-to-provider mapping の運用規約 (D8 が準拠)
- `knowledge/adr/` — 本 ADR を含む ADR 索引
