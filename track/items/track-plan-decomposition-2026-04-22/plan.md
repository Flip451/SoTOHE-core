<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# /track:plan 構造的分解 (Phase 0-3 独立 command + adr-editor capability)

## Summary

ADR 展開フェーズ 4-6.5 を完了させる: /track:plan の Interim mode を廃止し、4 つの独立 phase command (/track:init / /track:spec-design / /track:type-design / /track:impl-plan) と薄い state-machine orchestrator (/track:plan) に分解する
v3 設計変更: command 定義本文は writer invocation + 結果受け取りのみ。sotp CLI 呼び出しを含む全処理 (spec.json write + render + signal 評価 / baseline capture + types.json write + render × 3 + signal 評価 / impl-plan.json + task-coverage.json write + gate 評価) は対応 subagent 内部で一括完結する
3 subagent (spec-designer / type-designer / impl-planner) の frontmatter tools に Write + Bash (sotp CLI 専用) を追加し、内部責務範囲を agent definition に明記する
activate.rs の activation_requires_clean_worktree 緩和 (bootstrap 先行済、未 commit) を T012 として先頭に置き、/track:init が dirty worktree でも branch create を呼べるようにする
変更対象は .claude/commands/track/*.md / .claude/agents/*.md / .claude/skills/track-plan/SKILL.md / docs + IN-10 の Rust ファイルのみ (OS-07)

## Tasks (12/12 resolved)

### S1 — S1 — Rust preflight 緩和 (activate.rs)

> bootstrap 中に先行実施済の activation_requires_clean_worktree 緩和を commit 対象として確定する
> 新仕様: BranchMode::Auto && already_materialized && resume_allowed のみ true を返す
> 関連 unit tests を新仕様に合わせて更新し、Auto-not-materialized が false (dirty 許容) であることを確認する
> cargo make ci pass を確認して T012 commit とする

- [x] **T012**: bootstrap 先行実施済の apps/cli/src/commands/track/activate.rs の activation_requires_clean_worktree 緩和 (現状: matches!(mode, BranchMode::Auto) && already_materialized && resume_allowed) を commit 対象として確定する。既存 activation_resume_requires_clean_worktree rstest 群を新セマンティクス (Auto-not-materialized が false を返す) に合わせて更新する。BranchMode::Auto かつ already_materialized=false で false を返すこと (新規 track materialization 時の dirty worktree 許容) を確認する unit test を追加する。cargo make ci pass を確認してから commit する (IN-10 / AC-19 / AC-20) (`9c64da5a9680d9593cd8107558b2f39b8d65d2ab`)

### S2 — S2 — 独立 phase command 新規作成 + rename

> /track:init — Phase 0 専任 command。writer = main のため command 定義が直接処理を実行する (branch create + metadata.json write + ADR pre-check + identity validation)。CN-13 の main 例外に該当
> /track:spec-design — Phase 1 専任 command。command 定義本文は spec-designer subagent invocation + 信号機評価結果の受け取りのみ。sotp CLI 呼び出しは subagent 内部 (AC-22)
> /track:impl-plan — Phase 3 専任 command。command 定義本文は impl-planner subagent invocation + gate 結果の受け取りのみ。sotp CLI 呼び出しは subagent 内部 (AC-22)
> /track:type-design — design.md rename + command 定義を type-designer subagent invocation + 信号機評価結果の受け取りのみに縮小。baseline capture / render / signal 評価はすべて subagent 内部 (AC-21 / AC-22)

- [x] **T001**: .claude/commands/track/init.md を Phase 0 独立 command として新規作成する (writer = main)。/track:init は唯一 writer=main の command なので CN-13 例外として command 定義本文が直接全 step を実行する: (a) track directory 作成 + metadata.json (branch=null 暫定) write、(b) sotp track branch create '<track-id>' で main から track/<id> branch を作成して switch、(c) metadata.json update (branch=track/<id>)、(d) ADR 存在 pre-check (knowledge/adr/ に参照 ADR がなければ停止してユーザーに整備を促す)、(e) identity schema binary 検証 (sotp verify track-metadata)。state machine 遷移 / back-and-forth / max_retry / 事前承認 ceremony 文言は含まない。単独 invoke 可能。注: AC-13 (schema validation ロジック不含検査) は CN-10 により /track:init を明示除外するため本 task の AC-13 適用対象外 (IN-01 / AC-01 / AC-10 / AC-15 / AC-16 / AC-18) (`4b7d32b2e03e0c47a96ee7e309b576a161b2c666`)
- [x] **T002**: .claude/commands/track/spec-design.md を Phase 1 独立 command として新規作成する。CN-10 v3 / CN-13 に従い command 定義本文は (a) spec-designer subagent を Agent tool (subagent_type: spec-designer) 経由で invoke、(b) subagent が返した signal 評価結果を受け取って /track:plan に surface する、の 2 要素のみ。spec.json の Write、spec.md render (sotp track views sync 等)、信号機評価 (sotp verify plan-artifact-refs 等) はすべて spec-designer 内部で完結。command 定義本文に sotp CLI 呼び出しを含めない (AC-22)。pre-check: metadata.json の存在確認。state machine 遷移 / ceremony 文言は含まない。単独 invoke 可能 (IN-02 / AC-02 / AC-10 / AC-13 / AC-15 / AC-16 / AC-18 / AC-22) (`9264a4a9be96dbd2380c5147d81667aa4ab91938`)
- [x] **T003**: .claude/commands/track/impl-plan.md を Phase 3 独立 command として新規作成する。CN-10 v3 / CN-13 に従い command 定義本文は (a) impl-planner subagent を Agent tool (subagent_type: impl-planner) 経由で invoke、(b) subagent が返した gate 結果を受け取って /track:plan に surface する、の 2 要素のみ。impl-plan.json + task-coverage.json の Write、gate 評価 (sotp verify plan-artifact-refs) はすべて impl-planner 内部で完結。command 定義本文に sotp CLI 呼び出しを含めない (AC-22)。pre-check: spec.json および <layer>-types.json の存在確認。state machine 遷移 / ceremony 文言は含まない。単独 invoke 可能 (IN-03 / AC-03 / AC-10 / AC-13 / AC-15 / AC-16 / AC-18 / AC-22) (`9264a4a9be96dbd2380c5147d81667aa4ab91938`)
- [x] **T004**: .claude/commands/track/design.md を .claude/commands/track/type-design.md に rename し、CN-10 v3 / CN-13 に従って command 定義本文を (a) type-designer subagent を Agent tool 経由で invoke、(b) subagent が返した signal 評価結果を受け取って /track:plan に surface する、の 2 要素のみに縮小する。以下はすべて type-designer 内部に移動し command 定義からは削除する: baseline capture (sotp track baseline-capture)、<layer>-types.json 直接 Write、graph md render (sotp track type-graph 等)、contract-map.md render (sotp track contract-map)、<layer>-type-signals.md render、信号機評価 (sotp track type-signals)。command 定義本文に sotp CLI 呼び出しを含めない (AC-21 / AC-22)。command frontmatter の description と内部参照を /track:design → /track:type-design に更新。state machine 遷移 / ceremony 文言は含まない。単独 invoke 可能 (IN-06 / AC-10 / AC-13 / AC-15 / AC-16 / AC-18 / AC-21 / AC-22) (`e03bd2295362aa256b590fe688f791b575cb22e4`)

### S3 — S3 — subagent 権限拡大 + adr-editor 契約確定

> spec-designer.md / type-designer.md / impl-planner.md の frontmatter tools に Write + Bash を追加
> 各 subagent の Scope Ownership / Rules に内部責務を明記 (Write + sotp CLI 実行の権限と実行義務)
> 'This agent is advisory' / 'orchestrator が transcribe する' 等の旧記述を削除・置き換え
> adr-editor.md の Invocation contract を /track:plan state machine の呼び出し経路と整合させる

- [x] **T005**: 3 subagent agent definition (.claude/agents/*.md) の frontmatter tools / Scope Ownership / Rules / Boundary table を IN-09 v3 に従って更新する: (a) spec-designer.md — frontmatter tools に Write + Bash (sotp CLI 専用、cat/grep/head 禁止) を追加。'This agent is advisory: the orchestrator synthesizes its output into spec.json' を 'spec-designer が spec.json を直接 Write し、spec.md render + 信号機評価 (sotp CLI) を内部実行する' に変更。Scope Ownership / Rules に内部 pipeline を明記。Boundary table の typical trigger を /track:spec-design に。(b) type-designer.md — frontmatter tools に Write + Bash (sotp CLI 専用、cat/grep/head 禁止) を追加 (type-designer は既存 frontmatter に tools 宣言がないため Write も明示追加)。'Output is advisory JSON that the orchestrator writes' を 'type-designer が <layer>-types.json を直接 Write し、baseline capture + graph md render + contract-map render + type-signals render + 信号機評価 (sotp CLI) を内部実行する' に変更。Scope Ownership を更新。Boundary table の typical trigger を /track:type-design に。(c) impl-planner.md — frontmatter tools に Write + Bash を追加。'This agent is advisory: the orchestrator writes the artifacts' を 'impl-planner が impl-plan.json + task-coverage.json を直接 Write し、gate 評価 (sotp CLI) を内部実行する' に変更。Scope Ownership / Rules を更新。Boundary table の typical trigger を /track:impl-plan に (IN-09 / CN-12 / AC-17) (`3f808c02c78e4b497432903c9cdac58ba851d945`)
- [x] **T006**: .claude/agents/adr-editor.md の Invocation contract セクションを /track:plan state machine の back-and-forth 呼び出し経路 (T007/T008 で確立) と整合させる: (a) 呼び出し条件 (Phase 1 gate 🔴 かつ ADR に commit 履歴あり → auto-invoke、履歴なし → user pause)、(b) briefing 必須項目 (🔴 を発火した spec 要素 + 対象 ADR パス + 'edit working tree only; do not commit inside the loop' 指示)、(c) working-tree-only 制約、(d) loop 中 no commit を確認または追記する。既に 4 項目をすべて満たしていれば confirmatory note のみ記録して変更なしで完了してもよい (IN-05 / AC-06 / AC-09 / AC-11) (`0d70a387522386e34ca27e1560883fa330d74ce3`)

### S4 — S4 — /track:plan state machine 再定義 (Interim mode 削除)

> T007: 構造スケルトン — TaskCreate preamble 追加、Interim mode 削除、phase invocation テーブルを新 command 名に更新、Sub-invocation details を 'subagent が sotp CLI まで内部実行し結果だけを返す' 契約に更新、writer ownership 表の transcribe 文言削除
> T008: ループ本体 — Phase 1/2/3 各ループブロック (gate 結果読み取り → 🔴/ERROR 時 escalation → max_retry guard → user pause)、adr-editor 呼び出し条件 (commit 履歴有無で分岐)、終端 ADR diff 提示 + accept/revert/manual/abort 判断

- [x] **T007**: .claude/commands/track/plan.md の書き直し Part 1 構造スケルトン: (a) 冒頭に TaskCreate preamble を追加 (Phase 0 / Phase 1 loop / Phase 2 loop / Phase 3 loop / 終端処理 + back-and-forth 逐次伝播を task として立てる明示指示)、(b) Interim mode セクション全体を削除 ('While /track:init, /track:spec, and /track:impl-plan are not yet implemented ...')、(c) phase invocation table を /track:init → /track:spec-design → /track:type-design → /track:impl-plan の順次 invoke に書き換え、(d) Sub-invocation details セクションを 'subagent が sotp CLI を含む全 pipeline を内部実行し signal/gate 結果だけを /track:plan に返す' 契約に書き換え、'orchestrator が transcribe する' / 'orchestrator が sotp CLI を呼ぶ' 系の記述を削除、(e) Writer ownership table を更新して advisory/transcribe 文言を spec.json / <layer>-types.json / impl-plan.json / task-coverage.json 行から削除、(f) ADR pre-check セクションが /track:init を branch create step として参照していることを確認。T008 で per-phase loop body を書く (IN-04 / AC-05 / AC-13 / AC-14 / AC-15) (`62e3ec69ff4cffff7b604908e4a78502c43c03ea`)
- [x] **T008**: .claude/commands/track/plan.md の書き直し Part 2 back-and-forth loop body (T007 skeleton 上に追加): (a) Phase 1 loop block — /track:spec-design invoke → 返却された signal 結果読み取り → 🔴 の場合 ADR commit 履歴確認 → 履歴あり → adr-editor invoke (briefing に 🔴 発火 spec 要素 + ADR パス + working-tree-only 指示) → loop-back、履歴なし → user pause、max_retry guard、(b) Phase 2 loop block — /track:type-design invoke → signal 結果読み取り → 🔴 の場合 /track:spec-design 再 invoke → Phase 1 gate 再評価 → 🔴 なら ADR loop に escalate、max_retry guard、(c) Phase 3 loop block — /track:impl-plan invoke → gate 結果読み取り → ERROR の場合同 phase 再 invoke、max_retry guard、(d) 終端処理 (成功 / max_retry 超過いずれも): git diff HEAD -- knowledge/adr/*.md で ADR working-tree diff 確認、diff あれば accept / revert / manual edit / abort を user に提示、(e) max_retry positional integer 引数 (default 5、flag 名なし、/track:plan 3 形式) の記述を確認 (IN-04 / IN-05 / AC-04 / AC-06 / AC-07 / AC-08 / AC-09 / AC-13) (`62e3ec69ff4cffff7b604908e4a78502c43c03ea`)

### S5 — S5 — SKILL.md + docs 更新

> /track:plan SKILL.md thin registry の command 名参照を新名称に更新する (IN-07)
> DEVELOPER_AI_WORKFLOW.md / CLAUDE.md / track/workflow.md / knowledge/WORKFLOW.md の phase コマンド名参照を差分更新する (IN-08)
> T001 (PR #107) で大枠は記載済のため差分の最小追記に限定する

- [x] **T009**: .claude/skills/track-plan/SKILL.md thin registry の frontmatter description を更新: /track:init / /track:spec-design / /track:type-design / /track:impl-plan を名前で参照。/track:spec → /track:spec-design、/track:design → /track:type-design に置換。SKILL.md 本体は plan.md へ defer する設計を維持し、frontmatter description と明示的な command 名参照の更新のみ (IN-07) (`690cf012c39eaaa86c20ab77c8a2cf4e01242ce4`)
- [x] **T010**: DEVELOPER_AI_WORKFLOW.md / CLAUDE.md / track/workflow.md / knowledge/WORKFLOW.md に新独立 phase command 名を反映する。T001 (PR #107) で Phase 0-3 の大枠構造は記述済みのため差分の最小追記のみ: (a) Phase 1 行の /track:spec を /track:spec-design に置換、(b) Phase 2 行の /track:design を /track:type-design に置換、(c) Phase 0 行に /track:init を追加 (未記載箇所)、(d) Phase 3 行に /track:impl-plan を追加 (未記載箇所)。セクション全体の書き直しは行わない (IN-08) (`690cf012c39eaaa86c20ab77c8a2cf4e01242ce4`)

### S6 — S6 — CI 回帰ゲート確認

> cargo make ci (fmt-check + clippy + nextest + deny + check-layers + verify-* 一式) を実行し全 task の変更後に pass することを確認する
> T012 の Rust 変更を含む全変更が CI を通ることを AC-12 / AC-20 の充足証拠とする

- [x] **T011**: cargo make ci (fmt-check + clippy + nextest + deny + check-layers + verify-* 一式) を実行し、すべての先行 task commit 後に全通過することを確認する。regression があれば修正。T012 の Rust 変更を含む全変更が CI を通ることが AC-12 の充足証拠。本 track 全体の最終 integration gate (AC-12) (`733ab26e0d89ba526998aac25cd4aa2e362f0324`)
