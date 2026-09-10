---
adr_id: "2026-07-23-0111-composition-root-pure-di-realignment"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session_67282137-412e-4cc0-b687-505dd036e653:2026-07-23"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session_67282137-412e-4cc0-b687-505dd036e653:2026-07-23"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:current-task:2026-07-23:phase0-refinement-approved"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:session_67282137-412e-4cc0-b687-505dd036e653:2026-07-23"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:current-task:2026-07-23:phase0-refinement-approved"
    status: proposed
---
# composition root 規範を純 DI に確定し、実践側の逸脱を解消する

## Context

composition root の役割について、規範と実践が食い違っている。

規範側の 3 文書は純 DI で一貫している。

- `type-designer-kind-selection.md` R1: `CompositionRoot` は「object graph を組む純 DI の住所」。FreeFunction 行は「cli_composition は配線を CompositionRoot のメソッドとして書くため pub free function が生じない」と規定する。
- `.harness/custom/review-prompts/cli_composition.md`: 「It must **only wire** … hands the fully-wired drivers to `apps/cli`」。
- `.harness/custom/review-prompts/cli.md`: bin の仕事は「parse arguments into typed Input structs, obtain a wired Driver from cli_composition, and call `driver.handle(input)`」。

実践側の 2 つがこれに逸脱している。

- 出荷 placeholder（`overlay/apps/cli-composition/src/lib.rs`）: `pub fn run_greeting(raw_name: &str) -> Result<CommandOutcome, UsernameError>` が構築・検証・実行までを一体で行う。pub free function（R1 FreeFunction 行違反）、composition face への usecase error 型露出、内部で呼ぶ `GreetDriver::run(&Username)` の domain 型シグネチャ（R1 PrimaryAdapter 行の NoRoleInMethodSignature 違反）と、規範に三重に抵触する。
- sotp 本体（`apps/cli-composition/src/lib.rs`）: bounded context ごとの composition root が実行 command メソッド群を公開し CommandOutcome を返すファサード様式で書かれている。cli_driver crate は実質 CommandOutcome の置き場になっている。

逸脱は下流に再生産されることが実走で実証された。
テンプレート利用プロジェクト（mini-repomix）は composition に `run(PackDriverRequest) -> CommandOutcome` を生成した。
これは PackDriver::run と同一シグネチャの素通し 1 段で、構築の隠蔽にも型の隠蔽にもならない中継である。
reviewer はこれを報告できなかった。
当時の briefing はカテゴリ閉列挙で、invoke leak の定義が interactor 呼び出しに限定されていたためである。

## Decision

### D1: 規範は純 DI を維持・確定する

composition root の仕事は「secondary adapter・interactor・driver の構築と配線、および配線済み driver の引き渡し」のみとする。
composition root の公開面に実行メソッド（リクエストを受けて結果を返すメソッド）を置くことを、interactor 呼び出し・driver 呼び出しの別を問わず禁止と明文化する。
R1 の現行文言はこの決定の正本として維持し、「実行メソッド禁止」を追記して強化する。

### D2: 出荷 placeholder を規範の正例に書き換える

- `run_greeting` free function を廃止し、CompositionRoot struct のメソッドが配線済み `GreetDriver` を返す形にする。
- 入力検証は driver の内部（usecase 呼び出し経由）に移し、driver の公開シグネチャを `handle(raw 入力) -> CommandOutcome` にして domain 型（Username）を面から除去する。
- composition の公開面から usecase error 型（UsernameError）を除去する。
- bin は parse → composition から driver 取得 → `handle` → emit の 3 手に揃える（cli.md の記述どおり）。

### D3: レビューカテゴリに逸脱クラスを追加する

`cli_composition.md` のカテゴリに「composition 上の実行メソッド（interactor / driver いずれの invoke も）」「composition 公開面への domain / usecase 型および内部 role 型の露出」を追加する。純 DI の成果物として返す `PrimaryAdapter` は許可する。
briefing の記述形式（閉列挙か半開か）には依存せず、現行形式のままでも機能する。

### D4: sotp 本体のファサード様式は既知乖離として登記し、opportunistic に移行する

23 の bounded-context composition root を一斉改修する big-bang は行わない。
当該 context を実質的に改修する track が発生した時点で、その context の driver 抽出と composition の純 DI 化を同 track に含める。
移行完了までの間、本体の様式は既知乖離として本 ADR に登記された状態とする。

### D5: 公開面の規律は catalogue lint で強制する

composition root の公開面規律は、新規の source-level 検査ではなく、既存の catalogue lint で強制する。
具体的な lint schema・設定・検査運用は関連する規約・設定 SSoT に委ね、本 ADR では固定しない。

## Rejected Alternatives

### A. ファサード様式を公認する（実践に規範を合わせる）

規範 3 文書の書き換えだけで済み sotp 本体は無改修という費用優位がある。
しかし cli_driver 層が空洞化し、6 crate delivery 分割の存在意義（PrimaryAdapter = invoke + render の席、NoRoleInMethodSignature）が崩れる。
R1 PrimaryAdapter 行および architecture-rules.json の cli_driver 定義と連鎖的に矛盾するため却下。

### B. 現状放置しレビューカテゴリ追加のみ行う

規範と実践の乖離が残る限り、scaffold 利用側は目の前の placeholder を写して逸脱を再生産する（実証済み）。

### C. sotp 本体の big-bang 移行

23 root の一斉改修はリスクとコストが過大で、得られる挙動差分はない。
opportunistic 移行で十分。

### D. 公開面検査を source-level の verify として新設する

rustdoc 走査の新規実装が必要になり、発火が実装後の CI まで遅れ、sotp 本体の既存 root 群を即時に叩くため grandfather リストの整備も要る。
catalogue lint 案（D5）が発火時期・実装コスト・移行整合の全てで上回る。

## Consequences

### Positive

- 規範 3 文書・placeholder・レビューカテゴリが一致し、下流プロジェクトが正例だけを見る状態になる。
- 逸脱が名前を持ち（実行メソッド / 面への禁止対象型露出）、機械検査で止まる。
- placeholder の潜在違反 3 件（free fn / face の usecase 型 / driver face の domain 型）が同時に解消する。

### Negative

- sotp 本体に長期の既知乖離が残る（登記により可視化はされる）。
- placeholder 改修・面検査の実装・briefing 改訂の co-update 幅が広い。

## Reassess When

- opportunistic 移行が長期間（目安 1 年）進捗しないとき（big-bang の再検討または乖離の恒久公認）
- 第 2 の delivery（TUI / daemon 等）を追加するとき
- 公開面検査の誤検知が運用負荷になったとき

## Related

- `knowledge/conventions/type-designer-kind-selection.md` R1 — CompositionRoot / PrimaryAdapter / FreeFunction 行
- `.harness/custom/review-prompts/cli_composition.md` / `cli.md`
- `overlay/apps/cli-composition/src/lib.rs` — 出荷 placeholder
- `architecture-rules.json` — cli_driver / cli_composition の依存定義
- `.harness/catalogue-lint/config.json` / `presets/ddd-strict.json` — D5 のルール追加先（`NoRoleInMethodSignature` / `KindLayerConstraint` の既存前例）
