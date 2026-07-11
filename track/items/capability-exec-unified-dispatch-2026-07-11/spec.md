<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 29, yellow: 0, red: 0 }
---

# capability exec: profile 駆動の汎用 capability dispatch コマンド

## Goal

- [GO-01] `bin/sotp capability exec <capability-name> --host <provider> --briefing-file <path>` を、自由形式の成果物を orchestrator が消費する capability の単一かつ profile 駆動の dispatch 入口として提供する。呼び出し可能な capability、provider、model、実行経路は profile と provider-native adapter 定義から解決し、capability 名ごとの専用 runner を増設しない。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D1, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D2, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D4, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D5, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D10]
- [GO-02] 入力、profile、adapter 定義、および provider の不整合を dispatch 前に fail-closed で止め、契約を保持できる場合だけ in-host 委譲または subprocess 実行へ進める。これにより provider routing、model、権限、briefing 規律、および capability 定義への適合を一貫して保証する。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D2, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D3, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D6, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D7, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D10]
- [GO-03] orphan 化した `sotp plan codex-local` とその参照面を同一 track で撤去し、固定返却スキーマを持つ既存の専用 pipeline は新しい汎用 command に合流させず維持する。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D8, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D9]

## Scope

### In Scope
- [IN-01] `capability exec` command と、capability 名、必須の `--host`、必須の `--briefing-file` から成る共通の dispatch interface を追加する。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D1] [tasks: T001, T002, T006, T007, T008]
- [IN-09] 実行結果と in-host 委譲指示を、呼び出し側が機械判別できる判別子付きの出力として返す。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D7] [tasks: T002, T006, T008]
- [IN-02] dispatch 対象を profile の capability entry から解決し、`execution_mode: orchestrator-output` の entry だけを通す。存在しない capability、`typed-pipeline`、execution_mode の欠如、または未知の execution_mode は subprocess を起動せず reject する。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D2, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D9] [tasks: T001, T002, T003, T004]
- [IN-03] profile から provider と単一の model を解決し、claude と codex の provider arm だけを provider-generic な入口から dispatch する。未対応または未知の provider、model 欠如、または provider-native adapter 定義との model 不一致は dispatch 前に reject する。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D4, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D5, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D10] [tasks: T001, T002, T003, T004, T005]
- [IN-04] provider ごとの adapter 定義を、capability 定義への適合と実行権限の権威として扱う。codex は skill の sandbox 宣言を subprocess の sandbox 制約へ反映し、未宣言時は read-only とする。claude は agent 定義の `tools:` と profile と一致する `model:` の存在を preflight し、欠如または不一致を reject する。CLI と profile に権限を緩和または重複宣言する経路を設けない。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D3, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D10] [tasks: T003, T005, T008]
- [IN-05] briefing file と固定 discipline template を分岐判定より前に読み込み、通常ファイル性、読み取り可能性、UTF-8、非空内容を検証する。検証済み briefing と discipline を subprocess prompt に合成し、in-host 委譲では briefing path と読み込み済み discipline 本文の両方を必須 payload として返す。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D6, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D7] [tasks: T001, T002, T003, T004, T005]
- [IN-06] `--host` を runtime の呼び出し元 provider の必須自己申告として扱う。claude==claude かつ全 preflight が通る場合だけ subprocess を起動しない in-host 委譲指示を返し、codex==codex と cross-provider の場合は adapter による subprocess dispatch を実行する。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D7] [tasks: T001, T002, T005, T006, T007, T008]
- [IN-07] subprocess dispatch では provider-native adapter を使う。claude は検証済み agent 定義を `claude -p --agent <name>` で、codex は存在確認済み skill を明示的な `$<name>` mention と profile model で起動する。native adapter の非対話適合を確認できない場合や定義が欠ける場合に、raw definition の prompt 注入などへ fallback しない。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D5, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D10] [tasks: T002, T005, T008]
- [IN-08] `sotp plan codex-local` の CLI、driver、usecase、infrastructure adapter と、それを正規経路として示す規約・command・設定・skill の参照を撤去し、writer capability への案内を `capability exec` の経路へ差し替える。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D8] [tasks: T002, T005, T006, T007, T008, T009]

### Out of Scope
- [OUT-01] `review local`、dry 系、ref-verify、および obligation・waiver verifier のように固定返却スキーマを機械側が消費する typed-pipeline を `capability exec` へ合流させない。これらの既存 command、出力契約、auto-record、gate は維持する。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D9] [tasks: T001, T002, T008]
- [OUT-02] gemini または将来の provider を dispatch 対応にしない。provider-native adapter registry、権限宣言の enforcement、非対話 invocation contract が未定義の provider は、対応済みとして扱わず reject する。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D4, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D10] [tasks: T002, T005, T008]
- [OUT-03] fast/final tier または CLI での model 選択を `capability exec` に追加しない。profile model を持たない capability は本 command の対象にせず reject する。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D5] [tasks: T001, T002, T006, T007]
- [OUT-04] 権限を profile の provider 中立 field や `--sandbox` のような CLI flag で宣言・緩和する仕組みを導入しない。権限は provider ごとの adapter 定義に留める。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D3] [tasks: T003, T005, T006, T007]

## Constraints
- [CN-01] dispatch は fail-closed とする。必須引数、briefing、discipline template、profile entry、execution mode、provider、model、adapter definition、adapter preflight のいずれかが無効・欠如・不一致なら、in-host 委譲指示も subprocess 起動も行わず非成功で終了する。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D2, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D4, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D5, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D6, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D7, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D10] [tasks: T001, T002, T003, T004, T005, T006, T007, T008]
- [CN-02] profile は capability の universe、routing、model、execution mode の唯一の権威とし、adapter 定義は capability への適合と provider 固有権限の唯一の権威とする。capability 名の whitelist、個別 runner、model fallback、または profile と adapter の暗黙優先によってこの分担を迂回しない。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D2, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D3, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D5, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D10] [tasks: T001, T002, T003, T004, T005]
- [CN-03] すべての dispatch 分岐で、検証済みの briefing と同一の discipline を適用する。discipline の政策文面は Rust に複製せず固定 template から読み込み、native adapter を使えないときに raw definition を prompt 又は system prompt へ注入する代替経路を設けない。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D6, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D10] [tasks: T001, T002, T003, T004, T005, T008]
- [CN-04] 新 command の導入と `plan codex-local` の撤去によって、profile が routing する writer capability の正規 dispatch 経路が存在しない状態を作らない。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D8] [tasks: T002, T005, T006, T007, T008, T009]

## Acceptance Criteria
- [ ] [AC-01] 有効な `orchestrator-output` capability、`--host`、非空の briefing file を与えると、`sotp capability exec` が profile から provider と model を解決して dispatch outcome を返す。profile に新しい有効 entry を追加した場合に capability 名ごとの CLI 実装を追加せず同じ解決規則で扱われ、profile に存在しない名前は非成功で reject されることを検証する。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D1, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D2, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D4] [tasks: T001, T002, T004, T006, T007, T008]
- [ ] [AC-02] `typed-pipeline`、execution_mode 欠如、未知の execution_mode、model 欠如、未対応又は未知の provider、未対応 provider の adapter registry 不在を与えた場合、command が非成功で終了し、in-host 委譲も provider subprocess も発生しないことを検証する。`capability exec reviewer` のような typed-pipeline の拒否が capability 名の個別判定に依存しないことも確認する。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D2, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D4, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D5, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D9] [tasks: T002, T004, T005, T006, T008]
- [ ] [AC-03] `--host` 又は `--briefing-file` が欠ける場合、briefing path が通常の読み取り可能な UTF-8 非空 file でない場合、又は固定 discipline template が欠如・読取不能・UTF-8 不正・空白のみの場合、command が分岐判定より前に非成功で終了することを検証する。この失敗時に subprocess 起動又は in-host 委譲指示が出ないことも確認する。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D6, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D7] [tasks: T001, T002, T004, T006, T007, T008]
- [ ] [AC-04] claude adapter definition の `tools:` 又は `model:` が欠ける、又は agent model が profile model と一致しない場合に、in-host と subprocess のどちらも非成功で reject することを検証する。codex skill が存在しない場合、又は provider-native adapter の非対話起動契約を確認できない場合にも非成功で reject し、raw definition の prompt 注入へ fallback しないことを確認する。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D3, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D10] [tasks: T003, T005, T008]
- [ ] [AC-05] 有効な claude==claude dispatch は provider subprocess を起動せず、capability 名、briefing path、検証済み discipline 本文を含む判別可能な in-host 委譲指示を返すことを検証する。有効な codex==codex dispatch は profile model と skill の sandbox 宣言を適用した subprocess dispatch となり、cross-provider dispatch も各 provider adapter で subprocess dispatch となることを確認する。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D3, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D5, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D6, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D7] [tasks: T002, T005, T006, T007, T008]
- [ ] [AC-06] subprocess dispatch が claude では検証済み agent を `claude -p --agent <name>` で、codex では profile model と明示的な `$<name>` skill mention を用いて起動することを検証する。いずれの provider subprocess に渡す prompt にも、検証済み briefing path から変換した instruction と、読み込み・検証済みの固定 discipline template 本文の両方が含まれることを確認する。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D6, knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D10] [tasks: T005, T008]
- [ ] [AC-07] `sotp plan codex-local` が CLI の command 一覧から除去され、実行しても成功しないことを検証する。旧 planner の CLI・driver・usecase・infrastructure stack が残らず、D8 で列挙された規約、command、設定、rule、skill の live reference surfaces が `capability exec` を案内していることを確認する。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D8] [tasks: T002, T005, T006, T007, T008, T009]
- [ ] [AC-08] `review local`、dry、ref-verify、obligation・waiver verifier の既存専用 pipeline が `capability exec` の出力契約へ移行せず、従来の固定返却スキーマを機械消費する command と gate が引き続き利用可能であることを検証する。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D9] [tasks: T002, T008]
- [ ] [AC-09] codex skill の sandbox 未宣言時は read-only が適用され、CLI 引数による権限緩和はできないことを検証する。 [adr: knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md#D3] [tasks: T003, T005, T008]

## Related Conventions (Required Reading)
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/shell-parsing.md#Single Parser Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 29  🟡 0  🔴 0

