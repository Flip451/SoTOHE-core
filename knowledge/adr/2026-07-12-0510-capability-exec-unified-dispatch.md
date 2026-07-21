---
adr_id: 2026-07-12-0510-capability-exec-unified-dispatch
decisions:
  - id: D1
    user_decision_ref: "chat:session-777a39bb:2026-07-10 ユーザー指示「codex呼び出し経路が用意されていないキャパビリティを呼び出すためのコマンドを新設。実装はキャパビリティごとの再実装ではなく共通の経路を設けてフラグで呼び分け」+ ヒアリング回答（コマンド形状: sotp capability exec）"
    candidate_selection: "from:[A,B] chose:capability-exec"
    status: proposed
  - id: D2
    user_decision_ref: "chat:session-777a39bb:2026-07-10 ユーザー指示「呼び出せるキャパビリティはハードコーディングせず、agent-profiles.json に存在するキャパビリティであれば呼び出せるようにすること」"
    candidate_selection: "from:[C] chose:profile-as-universe"
    status: proposed
  - id: D3
    user_decision_ref: "chat:session-777a39bb:2026-07-10 ヒアリング回答（sandbox: profile に per-capability field を追加、未設定は read-only に fail-closed、CLI からの緩和不可）+ chat:session-019sbdU1kGbXYshKHBw4dxgt:2026-07-12 ユーザー指摘 3 連（「sandbox field は codex に寄りすぎ」「claude tool policy をコードにハードコードするのは駄目」「権限・tool 宣言は provider ごとの adapter ファイル上に定義するのが凝集度的に優れる」）→ profile の権限 field を廃し adapter 定義側宣言へ"
    candidate_selection: "from:[D,J,K] chose:adapter-file-permission-declaration"
    status: proposed
  - id: D4
    user_decision_ref: "chat:session-777a39bb:2026-07-10 ヒアリング回答（provider: profile 汎用解決 — codex / claude / gemini を共通入口で解決し、adapter arm を持つ provider へ dispatch、未対応 provider は fail-closed）"
    status: proposed
  - id: D5
    user_decision_ref: "chat:session-777a39bb:2026-07-10 ヒアリング回答「fast, final を使い分ける経路は高度に自動化・並列化が進んでいる経路ばかりだから fast, final を指定しなきゃいけない経路はないはず」（tier フラグ不採用）"
    candidate_selection: "from:[E] chose:single-model-resolution"
    status: proposed
  - id: D6
    user_decision_ref: "chat:session-777a39bb:2026-07-10 ユーザー運用指示「codex の呼び出しは一旦手で組み立てる」の機械化として承認（briefing-file 変換 + no-git discipline の自動注入は既存 guardrails 規約の適用）+ chat:session-019sbdU1kGbXYshKHBw4dxgt:2026-07-12 ユーザー指示「discipline 文面を Rust にハードコードせず、プロンプト外出しで注入する」"
    status: proposed
  - id: D7
    user_decision_ref: "chat:session-777a39bb:2026-07-10 ユーザー指示「フラグは『現在のオーケストレーターのプロバイダ』と『どのキャパビリティを呼び出したいか』で閉じる。オーケストレーターとキャパビリティのプロバイダが一致してたら『サブエージェントをよびだせ』という指示だけ出力する」+ chat:session-019sbdU1kGbXYshKHBw4dxgt:2026-07-12 ユーザー指示「per-capability sandbox が in-host で不能なら、codex が codex capability を呼ぶときは codex exec 経由の方がよい」→ codex==codex を subprocess dispatch に変更"
    candidate_selection: "from:[F,L] chose:required-host-flag-with-conditional-short-circuit"
    status: proposed
  - id: D8
    user_decision_ref: "chat:session-019sbdU1kGbXYshKHBw4dxgt:2026-07-10 ユーザー指示「プランナーの呼び出し経路を削除したほうが良い」"
    candidate_selection: "from:[G] chose:retire-planner-wrapper"
    status: proposed
  - id: D9
    user_decision_ref: "chat:session-019sbdU1kGbXYshKHBw4dxgt:2026-07-10 ユーザー判断「reviewer などの合流はリスクとコストが見合わない。やるとしたら fast/final フラグが必要になる上、これらのコマンドは返却スキーマが固定されているので合流は難しい」"
    candidate_selection: "from:[H] chose:keep-typed-pipelines-separate"
    status: proposed
  - id: D10
    user_decision_ref: "chat:session-019sbdU1kGbXYshKHBw4dxgt:2026-07-10 ユーザー指示「codex は md を読み込ませて従わせるよりスキルを介したほうが安定」+ D10 起草指示（claude 側 --agent 主経路の提案込みで承認）+ 2026-07-12 ユーザー指摘「skill 起動は $ mention を使う」"
    candidate_selection: "from:[I] chose:provider-native-adapter-conformance"
    status: proposed
---

# capability exec: profile 駆動の汎用 capability dispatch コマンド

## Context

capability → provider の routing SSoT は `.harness/config/agent-profiles.json` だが、provider の呼び出し経路が repo-native に用意されているのは一部 capability に限られる（reviewer / review-fix-lead: `sotp review local` / `review fix-local`、dry 系: `sotp dry` 内蔵 dispatch、semantic verifier 系: interactor 内部の subprocess pipeline。`sotp plan codex-local` も現存するが、これは monolithic planner 時代の wrapper であり、Phase 1-3 writer 分割後の profile に `planner` capability は存在せず orphan 化している — D8 で撤去）。writer 系（spec-designer / type-designer / impl-planner / adr-editor / implementer）、researcher、rollback-diagnoser には専用経路がなく、orchestrator ホストが Claude のとき profile が `provider: codex` を指すと、briefing file の手組み + `codex exec` の直接実行でしか dispatch できない。

2026-07-10 に profile を codex-heavy + gpt-5.6 世代へ更新した際、この運用ギャップが顕在化した（それまで Claude subagent での代行が行われており、routing SSoT と実運用が乖離していた）。また同日 Gemini CLI（個人向け Code Assist）の提供終了により researcher も codex へ再 routing され、手組み dispatch の対象がさらに広がった。手組み運用は hook 制約（コマンド文字列への git キーワード混入禁止）・sandbox 規律（workspace-write 時の git 操作禁止指示）・model 解決を毎回人力で再現する必要があり、定型化・機械化が必要である。

## Decision

### D1: 汎用 dispatch コマンド `sotp capability exec` を新設する

`bin/sotp capability exec <capability-name> --host <provider> --briefing-file <path>` を新設する。実装は**共通経路 1 本**であり、呼び出し先 capability ごとにサブコマンドや runner を再実装しない — capability 名は引数（フラグ）であって実装の分岐単位ではない。`capability` コマンドグループとして新設し、将来の `capability list` / `capability describe` 等の同居余地を残す。

インターフェースは「現在のオーケストレーターのプロバイダ（`--host`）」と「呼び出したい capability（引数）」の 2 入力 + briefing で閉じる（D7）。

### D2: 呼び出せる capability の宇宙は agent-profiles.json（ハードコード禁止）

名前解決可能な capability の集合をコードにハードコードしない。`agent-profiles.json` の `capabilities` に存在する名前だけを解決し、存在しなければ fail-closed でエラーする。各 capability entry には実行経路を表す `execution_mode: "orchestrator-output" | "typed-pipeline"` を必須とし、`capability exec` が dispatch できるのは `orchestrator-output` の entry に限る。`typed-pipeline` は「固定返却スキーマを機械側が消費する専用経路」の宣言であり、本コマンドでは subprocess を起動せず fail-closed error を返す。field の欠如・未知値も fail-closed とする。これにより D9 の対象境界は profile data から機械判定でき、capability 名の whitelist / denylist や capability ごとの分岐を Rust コードに持たない。capability の追加・削除・routing 変更に CLI 実装の変更は不要である（provider authority の既存原則の帰結。コード内ホワイトリスト案 C の却下と対をなす）。

ただしこれは「config 編集だけで dispatch 可能」を意味しない: D10 により、`orchestrator-output` の subprocess dispatch は routing 先 provider の adapter 定義（claude: `.claude/agents/<name>.md`、codex: `.agents/skills/<name>/SKILL.md`）の存在を前提とし、欠如は fail-closed する。config は**宇宙と routing（provider / model / execution mode）**の SSoT、定義 md 群は**適合と権限**の SSoT（D3/D10）であり、capability 追加・provider 変更の完結条件は「config entry の execution mode を含む整備 + 対象 provider の定義面整備」の両方である（削除は config 除去だけで dispatch 不能になるが、残置 adapter 定義の掃除が保守事項として伴う）。

### D3: 実行権限は各 provider の adapter 定義ファイル上で宣言する（profile に権限 field を持たない）

書き込み権限・読み取り専用・呼び出し可能 tool の宣言は、capability の per-provider adapter 定義ファイルに置く。権限語彙は本質的に provider 固有であり、その provider の adapter ファイル内に閉じ込めるのが情報の凝集として正しい（中立 config への語彙漏れ [J] も、中立 field と実 enforcement の二重宣言 [K] も生じない）:

- **codex**: `.agents/skills/<name>/SKILL.md` の frontmatter に sandbox 宣言（`sandbox: workspace-write | read-only` — codex 語彙を codex adapter 内で使うのは漏れではない）を置き、dispatch が `--sandbox` フラグへ機械変換する。**未宣言は read-only に fail-closed**
- **claude**: `.claude/agents/<name>.md` の `tools:` frontmatter（claude 語彙）が tool 権限を宣言し、`--agent`（D10）がそのまま束縛する。write 系 tool の分類・列挙を dispatch のコードに持たない（コードが担うのは形式変換のみ）。claude の既定は tools 未宣言 = 全 tool 許可（fail-open）のため、**tools 未宣言の agent 定義への dispatch は fail-closed で拒否**する（宣言の存在チェックは tool 名の分類なしで機械化できる）。`claude -p` に kernel sandbox は無く、enforcement 強度は codex より弱い soft 制約になる
- 写像未定義の provider: fail-closed（D4 と同型）

`agent-profiles.json` に権限 field は設けない — profile は routing（provider / model / execution mode）の SSoT、adapter 定義ファイルは定義適合と権限の SSoT という分担になる（D2 の分担の精密化）。`execution_mode` は capability の実行経路だけを選び、権限や返却 schema 自体を記述しないため、この責任境界を崩さない。Claude agent frontmatter の `model:` は provider-native 起動に必要な profile model の投影であり、別の SSoT ではない（整合規則は D5/D10）。CLI 側に権限を緩和するフラグを設けない点は不変（宣言は tracked な定義ファイル一元、実行時緩和なし）。`danger-full-access` 級の全権語彙は許容しない。

<!-- illustrative, non-canonical -->
```yaml
# .agents/skills/type-designer/SKILL.md の frontmatter
---
name: type-designer
description: ...
sandbox: workspace-write   # SoTOHE 拡張 key — dispatch が --sandbox へ変換
---
```

### D4: provider は profile から汎用解決する（codex 専用にしない）

dispatch 経路は capability の `provider` を profile から解決し、provider ごとの adapter arm を同一の共通入口から呼び分ける（既存の semantic-verifier subprocess pipeline と同型の provider dispatch を再利用ないし共通化する）。この「共通経路」は provider identifier を共通に解決することを指し、`providers` に label があるだけで実行対応済みになることを意味しない。

現行の subprocess adapter arm は **claude と codex のみ**とする。`providers.gemini` は既知の identifier として登録されているが、capability routing は現在 1 件もなく、provider-native adapter registry、権限宣言から起動制約への写像、invocation contract も定義されていないため、**gemini への routing は subprocess 起動前に fail-closed** する。gemini を対応 provider に加えるには、D3/D10 と同等の adapter 定義面・権限 enforcement・invocation contract を先に定義し、その provider arm を追加しなければならない。未知 provider も同様に fail-closed とする。これにより本コマンドは「codex 呼び出し経路の欠落」という当初動機を超えて provider 移行に中立な入口を提供しつつ、未実装 provider を対応済みと誤認しない。

### D5: model tier フラグは設けない（単一 `model` 解決）

`--round-type fast|final` のような tier 選択フラグは設けない。常に `capabilities.<name>.model` を解決し、`model` が未定義の capability（例: クラウド側で dispatch される pr-reviewer）は fail-closed でエラーする。fast / final の使い分けが必要な capability（reviewer / dry-checker / ref-verifier / obligation・waiver verifier）は既に高度に自動化・並列化された専用経路を持っており、本コマンドの対象領域に tier 分岐は存在しない。

model の権威は全 provider・全 dispatch 分岐で `capabilities.<name>.model` **だけ**とする。provider-native adapter が実行 model を自身の定義面にも必要とする場合、その宣言は profile 値の投影として扱い、dispatch 前に文字列の完全一致を検証する。現行では Claude agent md の `model:` がこれに該当し、欠如または profile 値との不一致は in-host 委譲指示を返す前・subprocess を起動する前のどちらでも fail-closed とする。一致時だけ agent frontmatter による model 束縛を許可し、profile と adapter のどちらかを暗黙に優先して drift を隠す fallback は設けない。Codex skill は model を宣言せず、subprocess の `codex exec -m` に profile で解決した値を渡す。この規則により、in-host / subprocess のどちらでも実行 model は profile から一意に導出される。

### D6: briefing-file 入力と sandbox 規律の自動注入を経路に内蔵する

入力は `--briefing-file <path>` で受ける。flag の欠如または空の path は CLI 入力エラーとする。dispatch は D7 の分岐判定より先に briefing file を読み、path が存在しない、通常ファイルとして解決できない、読み取り不能、UTF-8 不正、または内容が空・空白のみのいずれかなら fail-closed error（非成功終了）とする。この場合は in-host 委譲指示を出力せず、subprocess も起動しない。検証に成功した場合だけ、内部で「Read {path} and perform the task」形式に変換して provider に渡す。権限宣言の値や D7 の分岐に依らず、briefing 内容に加えて git 操作禁止 discipline（`git add` / `commit` / `push` 直接実行の禁止、選択的 staging は `tmp/track-commit/add-paths.txt` 経由、guarded commit は wrapper 経由）を常時自動注入する（読み取り系には無害な冗長、書き込み系には必須 — 経路から権限 bit への依存を消し、D3 の adapter 側宣言と両立させる）。

discipline の文面は Rust コードにハードコードしない。canonical template を repo-root 相対の固定 path `.harness/prompts/capability-exec-discipline.md` に置き、すべての capability / provider / dispatch 分岐でこの 1 ファイルだけを選択する（capability ごとの選択や fallback template は設けない）。この path は profile に置かず（routing SSoT への関心混入 — K の却下理由に反する）、dispatch 実装が共通定数として保持する。dispatch は分岐判定や provider 起動より先に template を読み、欠如・読み取り不能・UTF-8 不正・空または空白のみの内容を fail-closed error として扱う。コードが保持するのはこの path 参照と機械的な変換 template（「Read {path} and perform the task」等の 1 行合成）のみで、政策的な文面を持たない。手組み運用で人力再現していた安全規律をコマンドが構造的に保証する。

briefing file は、D7 の dispatch 規則が生む 2 つの分岐 — provider 一致時の **in-host 委譲指示**と、それ以外の **subprocess 実行** — のどちらに落ちても同一の呼び出し規約とする: 呼び出し元は常に briefing を先に書いてから `capability exec` を呼び、「実行結果」（subprocess 分岐）か「その briefing と同じ discipline を使った self-invoke 指示」（in-host 分岐）のいずれかを受け取る。subprocess 分岐では読み込んだ template 本文を provider prompt に合成する。in-host 分岐では構造化された委譲出力に briefing path と**読み込み済みの discipline 本文**を別フィールドで含め、呼び出し元は両方を native adapter の task prompt に渡さなければならない（template path だけを再解決させたり、discipline を省略したりしてはならない）。したがって template 検証は in-host 指示を返す場合にも必須であり、呼び出し元は自分がどちらの分岐に落ちるかを事前に知る必要がない。権限宣言（D3）の効き方は、provider の enforcement 機構の性質により異なる。codex の sandbox は**プロセス起動時にのみ指定できる session 単位の属性**であるため、SKILL.md の sandbox 宣言が `--sandbox` フラグとして効くのは、新しい codex プロセスを起動する subprocess dispatch だけである（in-host の inline invoke では host session が起動時に持っていた sandbox がそのまま適用され、per-capability 宣言を適用できない）。この性質が、codex==codex を in-host 委譲せず subprocess で dispatch する D7 の分岐規則の根拠である。一方 claude の `tools:` 宣言は **agent 単位の属性**であるため、in-host subagent 起動と subprocess `--agent` 起動のどちらでも同一に束縛され、claude==claude の in-host 委譲が契約を損なわない根拠になっている。

### D7: `--host` は呼び出し元の自己申告（必須）とし、in-host 委譲は契約を保持できる provider の一致時に限る

「現在のオーケストレーターのプロバイダ」は runtime の事実であり config から導出しない — profile の `capabilities.orchestrator.provider` は「誰がオーケストレートすべきか」の宣言であって「誰が実際に呼んでいるか」ではない（両者は乖離しうることが 2026-07-10 の運用で実証済み: profile は codex、実ホストは Claude）。したがって `--host <provider>` は**呼び出し元が自己申告する必須フラグ**とする。各ホストの adapter 文書（`.claude/commands` / `.agents/skills`）が自ホストの身元を固定値で記載する — これは capability のハードコードではなく呼び出し元の自己同一性の宣言である。

dispatch 規則:

- **一致 かつ host の native adapter 機構が capability 契約を保持できる場合**（現行 provider では **claude のみ**: subagent は agent md の frontmatter で tools と、D5 に従い profile と一致検証済みの model を束縛できる）: サブプロセスを起動せず、**in-host 委譲指示を構造化出力**して終了する。委譲指示を返す前に agent md の `model:` と `tools:` の存在、および `model:` と profile model の完全一致を検証し、欠如・不一致なら fail-closed とする。その出力は capability 名と briefing path に加えて D6 で読み込み・検証済みの discipline 本文を必須 payload とし、host adapter は briefing と discipline の両方を Claude subagent の task prompt に渡す。discipline payload の欠落を許す別の in-host 呼び出し形は設けない。契約を保持できる in-host 委譲は、セッション文脈・ツール面・権限統合の点で subprocess wrap より優る。
- **codex==codex（一致だが契約を保持できない）**: subprocess dispatch を実行する。codex の inline skill は session の model と sandbox のまま走り、per-capability の model 解決（D5）も sandbox 宣言（D3）も適用できない。multi-agent の role config layer（`[agents.<name>].config_file`）で model 等は差し替えうるが、per-agent sandbox override は文書化されておらず、capability/skill を特定 role に束縛する機構も無い（codex-cli 0.144 / 公式 config reference、2026-07-12 確認）。subprocess なら `-m` と `--sandbox` で両契約を保証できるため、契約保持を session 統合より優先する。
- **不一致**: D3〜D6 の cross-provider subprocess dispatch を実行する。

「in-host が契約を保持できるか」は provider の native 機構の性質であり、capability 単位の設定ではなく provider ごとの dispatch arm（D4）が持つ判断とする。

出力は「実行結果」と「委譲指示」が機械判別できる構造（判別子付き）とする。

### D8: orphan 化した planner 専用経路 `sotp plan codex-local` を撤去する

`sotp plan codex-local` とその hexagonal stack 一式（cli `commands/plan`、cli-driver `PlanInput` / `PlanDriver`、usecase `planner`（`PlannerPort` / `PlannerService` / `PlannerInteractor`）、infrastructure `CodexPlannerAdapter`）を、`capability exec` を導入する同一 track で削除する。撤去根拠:

- **serve する capability が存在しない**: `planner` capability は Phase 1-3 writer 分割（spec-designer / type-designer / impl-planner）により profile から消滅済み。経路が `--model` を呼び出し側の手動解決に頼っているのはその症状である
- **機能が完全に包含される**: briefing→prompt 変換・codex spawn・timeout・session log はすべて D4/D6 の共通経路が担う。reviewer / dry 系が持つ fast/final tier・auto-record のような固有自動化は planner 経路には存在しない
- **sandbox 不整合の解消**: 経路は `--sandbox read-only` 固定だが、fallback 文書（`.claude/commands/track/plan.md` の Codex path 列）が routing する先は spec-designer / impl-planner という writer であり、read-only では SoT ファイルを書けない（2026-07-10 の実運用でも手組み workspace-write dispatch で迂回されていた）。D3 の adapter 定義側の権限宣言が正しい置き換えである
- **A の却下理由との整合**: 本経路は Rejected Alternative A（capability ごとの専用 wrapper）パターンの現存個体であり、併存は A の却下を形骸化させる

参照面の更新も撤去に含める: `.claude/rules/07-dev-environment.md`（native subcommands 一覧・briefing path 例）、`.claude/rules/10-guardrails.md`（fallback 記述と sandbox 推奨リストの planner 言及）、`.claude/commands/track/plan.md`（Codex path 列）、`.claude/settings.json`（PermissionRequest hook 文言と `Bash(bin/sotp plan:*)` allowlist）、`.codex/rules/default.rules`（prefix_rule）、`.claude/skills/codex-system/SKILL.md`（capability 例示）を `capability exec` ベースに差し替える。撤去を同一 track で行うことで、codex-routed writer 系の dispatch 経路が存在しない期間を作らない。

### D9: 固定返却スキーマを持つ専用 pipeline は合流させない（対象境界の確定）

本コマンドの対象は「briefing を入力に自由形式の成果物を返し、**orchestrator がそれを消費する** dispatch」に限り、profile entry の `execution_mode` を `orchestrator-output` と宣言する。`review local` / `dry` 系、および ref-verify / obligation・waiver verifier の内蔵 pipeline のように、**返却スキーマが固定され、機械側（verdict envelope parse・auto-record・check-approved / dry gate・verdict lane・fulfillment / waiver cache）が出力を直接消費する経路**は `typed-pipeline` と宣言し、合流対象外として維持する。`capability exec reviewer` のように `typed-pipeline` の名前が入力された場合は、profile の宣言だけを根拠に dispatch 前に拒否する。対象名の列挙をコードに持たず、欠如・未知値も拒否する具体的な判定規則は D2 に従う。

合流させるには (a) 汎用経路に per-capability の出力契約と tier フラグを持ち込む（D1「capability 名は実装の分岐単位ではない」・D5 と衝突）か、(b) 出力契約を捨てて parse・記録責務を orchestrator に戻す（review protocol の機械化の退行）かのいずれかになり、リスク・コストが見合わない。infrastructure 層での subprocess plumbing の共有（D4 の「再利用ないし共通化」）はこの境界と両立し、むしろ推奨される — 共有するのは spawn / timeout / log の配管であって、コマンド面と出力契約ではない。

### D10: capability 定義への適合は provider-native adapter 経由で行う（claude = `--agent` / codex = skill 名指し）

subprocess 分岐で dispatch された agent が「自らを定義する md」に適合する機構を、provider-native な adapter registry に統一する。in-host 分岐（D7）が使うのと同じ定義面を subprocess でも使う:

- **claude provider**: `claude -p --agent <name>` でセッションを `.claude/agents/<name>.md` の agent として実行する（本文 system prompt + `tools:` 制約 + model を束縛）。agent md の `model:` は profile model の provider-native な投影であり、独立した選択権を持たない。D5 と同じ preflight（`model:` 必須、profile model と文字列で完全一致）を subprocess 分岐にも適用し、一致後にのみ `--agent` を起動する。headless（`-p`）で本文・tools・一致済み model の 3 要素が束縛されることを実測で確認し、確認できなければ subprocess を起動せず fail-closed する。`--append-system-prompt-file` は本文を注入できても agent frontmatter の `tools:` と model を束縛しないため fallback に使用しない。profile model を `--model` で渡すだけでも `tools:` 契約は復元できず、同等 enforcement にはならない。また、不一致を `--model` override で覆い隠すこともしない。claude 側の tool 権限は agent 定義の `tools:` frontmatter が担う（権限宣言そのものが adapter 定義側にある — D3）
- **codex provider**: `codex exec` にスキル選択フラグは存在しない（codex-cli 0.144.1 で確認）ため、prompt 合成でスキルを起動する。model は skill 定義から解決せず、D5 で profile から解決した値を `codex exec -m` に渡す。その際は自然言語の名指しではなく **`$<name>` の skill mention 構文**を使う（explicit invocation — 「`$<name>` Briefing: `<path>`」型）。implicit invocation（description マッチの自動選択）は `allow_implicit_invocation` policy で無効化されうるため、explicit mention だけが決定的な起動契約である。`$` mention が非対話の exec prompt でも capability skill を起動することを実測で確認し、確認できなければ raw 定義 md の prompt 注入へ fallback せず fail-closed する。`.agents/skills/<name>/SKILL.md` の存在を dispatch 前に検証し、無ければ fail-closed。repo skills の読み込みには trusted checkout が前提（既存の orchestration 規約に記載済み）

briefing 変換（D6）はこの adapter 起動文と合成する — 定義適合は adapter 面が、作業入力は briefing が担う分業になる。生の定義 md を prompt / system prompt へ直接注入して native adapter の代替にする方式は使用しない（運用経験上、skill / agent 機構経由の方が安定で、codex 側には skill-compliance hook による準拠監視も既にある）。`.harness/capabilities/<name>.md` は両 adapter の背後にある provider 非依存 SSoT のまま変わらない。adapter registry を持たない provider への subprocess dispatch は定義適合を検証できないため fail-closed する（D4 の未対応 provider fail-closed と同型）。

導入時に不足している adapter カバレッジを authoring して埋める: `.agents/skills/implementer/SKILL.md`（profile は implementer → codex）と researcher の定義面一式（`.harness/capabilities/researcher.md` + 両 adapter）。「定義なし capability は briefing-only で通す」という escape hatch は設けない（fail-closed 一貫性）。

## Rejected Alternatives

### A. capability ごとの専用 wrapper を増設する

`sotp plan codex-local` と同型のサブコマンドを spec-design / type-design / impl-plan / adr-edit / implement / research ごとに追加する案。dispatch 本体・briefing 変換・sandbox 規律がサブコマンド数ぶん複製され、capability 追加のたびに CLI 実装が必要になる。ユーザー指示（共通経路 + フラグ呼び分け）により却下。

### B. Claude subagent での代行を継続する

profile が `provider: codex` を指す capability を Claude subagent adapter で代行する現状運用の追認案。routing SSoT（profile）と実運用が恒常的に乖離し、「provider は config が権威」という原則が形骸化する。2026-07-10 のユーザー是正指示（設定にあわせて動く）により却下。

### C. 呼び出し可能 capability のホワイトリストをコードに持つ

dispatch 対象をコード内 enum / 定数リストで管理する案。capability 追加のたびにコード変更が必要になり、profile の SSoT 性を破る。ユーザー指示（ハードコーディング禁止）により却下。

### D. sandbox を CLI フラグで指定する

`--sandbox workspace-write` のような実行時フラグで sandbox を決める案。呼び出し側の裁量で書き込み権限を緩和できる面がコマンドラインに露出し、fail-closed 原則に反する。権限は per-provider adapter 定義ファイルに宣言し、CLI から緩和するフラグを設けない D3 の方式を採択したため却下。

### E. `--round-type fast|final` tier フラグを持つ

既存 `review local` と同型の tier 選択を持つ案。本コマンドの対象 capability はいずれも単一 `model` のみで、fast / final を持つ capability には既存の専用自動化経路がある。ヒアリングにより却下。

### F. `--host` を省略可とし profile の `orchestrator.provider` を既定値に使う

呼び出し元ホストを config から推定する案。profile の orchestrator 宣言は「あるべき姿」であって runtime の実ホストと乖離しうる（2026-07-10 に実証: profile は codex 宣言、実ホストは Claude）。非適合状態で静かに誤ルーティング（一致と誤判定して in-host 指示を返す等）するため、fail-closed 原則に反し却下。`--host` は必須の自己申告とする。

### G. `plan codex-local` を capability exec と併存させる

新経路導入後も既存 planner wrapper を残す（本 ADR 初稿の「当面併存」）案。serve すべき `planner` capability が profile に存在しない orphan であり、`--sandbox read-only` 固定は fallback 文書が routing する writer 系（spec-designer / impl-planner）と矛盾し、dispatch 面が二重化して参照文書のドリフト源になる。reviewer / dry 系と異なり残す価値のある固有自動化を持たないため、ユーザー指示により撤去（D8）を採択し却下。

### H. reviewer / dry 系の専用経路も capability exec に合流させる

`review local` / `dry` 系（および ref-verify / obligation・waiver verifier pipeline）まで本コマンドへ統合し、dispatch 面を完全に 1 本化する案。これらは fast / final tier 選択（D5 で不採用としたフラグが必要になる）に加えて固定返却スキーマの機械消費者を持ち、合流には per-capability 出力契約の持ち込み（D1 崩壊）か出力契約の放棄（review gate 群の退行）が必要。リスク・コストが見合わないためユーザー判断により却下（D9）。

### I. 定義 md を prompt / system prompt へ直接注入して適合させる

`.harness/capabilities/<name>.md` を claude では `--append-system-prompt(-file)`、codex では prompt 先頭への合成で読み込ませて従わせる案。claude 側はフラグが実在するが、本文注入だけでは agent frontmatter の `tools:` と model を束縛できず、D3/D5 の実行契約と同等にならない。codex 側には instructions / system-prompt 系フラグ自体が存在せず prompt 合成のみとなり、生の md 追従は文脈内の他要素と競合して安定性で劣る（運用経験）。in-host 分岐が使う provider-native adapter（subagent / skill）と非対称になり、同一 capability が分岐によって別の定義解決を持つことにもなる。ユーザー判断により provider-native adapter 経由（D10）を採択し、native adapter の起動契約を確認できない場合は定義注入へ fallback せず fail-closed するため、本案を却下。

### J. 宣言 field に codex の sandbox 語彙をそのまま使う

profile の宣言 field を `sandbox: "workspace-write" | "read-only"`（codex CLI の `--sandbox` 値の逐語コピー）とする案（本 ADR 初稿）。provider 中立であるべき routing SSoT に特定 provider の flag 語彙が漏れ、codex の語彙変更に config schema が連動する。また claude 側の enforcement は kernel sandbox ではなく tool policy 写像（D10）であり、「sandbox」という名は claude provider に対して実態より強い保証を示唆する。ユーザー指摘により却下（経緯: いったん provider 中立 field `workspace_access` へ改名した [K] 後、宣言自体を各 provider の adapter 定義ファイルへ移した [D3]。最終形でも「中立 config に特定 provider の語彙を置かない」という本却下の理由は保存されており、codex 語彙は codex adapter ファイル内でのみ使う）。

### K. 権限を profile の provider 中立 field（`workspace_access`）で宣言する

`agent-profiles.json` に `workspace_access: "write" | "read"` を持たせ、dispatch が provider ごとに enforcement へ写像する案（本 ADR 第 2 稿）。J の語彙漏れは解消するが、(a) claude 側の実 enforcement は agent 定義の `tools:` frontmatter に置かざるを得ず、profile の中立 field は宣言意図に留まって二重宣言・drift 面になる、(b) 権限は tool 粒度の provider 固有情報を含み、中立 bit に落とすと表現できない、(c) routing SSoT に routing 以外の関心が混入する。ユーザー指摘（権限・tool 宣言は provider ごとの adapter ファイルに置くほうが情報の凝集度が高い）により、adapter 定義側宣言（D3）を採択し却下。

### L. provider 一致時は codex==codex を含め常に in-host 委譲する

本 ADR 初稿の D7 分岐規則（in-host 委譲は subprocess wrap より常に優るという前提）。しかし codex の inline skill は session の model・sandbox を継承するため、codex==codex の in-host 実行は per-capability の model 解決（D5）と sandbox 宣言（D3）を両方失う（例: workspace-write の codex ホストが read 系 capability を host の書き込み権限・host の model の下で走らせる）。per-agent sandbox override は codex に文書化されておらず（`[agents.<name>]` role config に sandbox field 無し）、skill→role の束縛機構も無い。ユーザー判断（「不能なら codex exec 経由の方がよい」）により codex==codex を subprocess dispatch へ変更して却下。in-host 委譲は契約を保持できる provider（現行 claude のみ）に限る。

## Consequences

### Positive

- routing SSoT（agent-profiles.json）と実運用が一致する。orchestrator ホストが Claude でも codex-routed writer 系を正規経路で dispatch できる
- capability の追加・provider 変更・権限宣言に CLI 実装の変更が不要になる（config + 対象 provider の定義 md の整備で完結 — D2/D3/D10。コードに手が入るのは provider 自体の追加時のみ）
- 手組み運用で毎回人力再現していた hook 安全規律（briefing file 変換・no-git 注入）が経路に構造的に内蔵される
- provider 中立の dispatch 面ができ、将来の provider 追加・移行のコストが下がる
- dispatch 面が capability exec に一本化され、orphan 化していた planner wrapper（存在しない capability・手動 model 解決・writer と矛盾する read-only 固定 sandbox）が撤去される（D8）
- 定義適合が全分岐（in-host / subprocess × provider）で provider-native adapter に統一され、in-host と subprocess で同一の定義面（`.claude/agents` / `.agents/skills`）が効く（D10）

### Negative

- 実装コスト: 新 usecase / driver / composition / CLI 面と型カタログ・spec の整備、および planner 経路の撤去（D8 — 4 層の削除 + 参照文書の差し替え）が必要（1 track 相当）
- dispatch が adapter 定義ファイルの frontmatter 解析を持つ必要がある（codex SKILL.md の sandbox 宣言の読み取り / claude agent md の tools・model 宣言の存在チェックと profile model との一致検証）。profile の model 変更時は Claude agent md の投影も同時更新しなければ dispatch が fail-closed する。codex が SKILL.md frontmatter の SoTOHE 拡張 key を許容し続けるかは実装時に実測する。`agent-profiles.json` は経路選択用 `execution_mode` の schema 拡張を要するが、権限 field の追加は不要である（D2/D3）
- 書き込み権限で動く subprocess は Claude Code hook の適用範囲外で動く（既知の hook coverage 制約）。D6 の discipline 注入は緩和策であり、hook と同等の強制力ではない
- codex ホストからの codex capability dispatch は nested subprocess になる（host session の sandbox / approval 設定の下で `codex exec` が走る — network 到達性等は host 設定に依存）。本経路の対象 capability（writer 系 / researcher / rollback-diagnoser）は内部で provider subprocess を起動しないため、本経路が作るネストは 1 段に留まる（1 段ネストは実績上安定）
- adapter カバレッジの整備が必要: `.agents/skills/implementer` と researcher の定義面（capabilities spec + 両 adapter）を導入時に authoring する（D10）。また claude `--agent` の headless 挙動と codex `$` mention の非対話 exec prompt での挙動は実測が必要で、いずれかが不成立なら該当 provider の subprocess dispatch は fail-closed する。このため native adapter 側の非対話対応が整うまで capability を dispatch できない可能性がある
- `providers.gemini` は identifier として存在しても adapter registry・権限 enforcement・invocation contract が未定義なため、現時点では dispatch 非対応である。将来の対応には provider arm の追加が必要になる（D4）
- 同一 capability の権限宣言が provider adapter 間で乖離しうる（codex 側 write / claude 側 read-only 相当の tools、等）。provider 中立の単一宣言を持たないこと、および write 系 tool の分類語彙をコードに持たないこと（D3）のトレードオフで、整合はレビュー lane / 保守事項に残る

### Neutral

- 既存の専用経路のうち `review local` / `dry` 系は併存を維持する（固定返却スキーマの機械消費者を持つ pipeline であり、合流は H で却下 — D9。infrastructure 層の plumbing 共有は D4 の範囲で行う）。`plan codex-local` は D8 で撤去する
- pr-reviewer のような model 未定義 capability は本経路の対象外（fail-closed エラー）のまま

## Reassess When

- 本経路で fast / final tier の使い分けが必要な capability を dispatch する必要が生じたとき（D5 の tier フラグ再検討）
- provider の追加、または codex CLI の invocation 形態（フラグ・sandbox 語彙・構造化出力）が大きく変わったとき
- codex exec にスキル選択フラグが追加されたとき、または claude `--agent` の headless 挙動が変わったとき（D10 の adapter 起動形態を更新）
- Claude の native agent 機構が profile の model を直接受け取れるようになり、agent md の `model:` 投影を重複なく廃止できるとき（D5 の一致検証を再検討）
- 構造化出力を capability 非依存に扱える出力契約面（汎用 schema 検証等）が整い、専用 pipeline の合流（H / D9）を再検討する価値が生じたとき
- 権限宣言の語彙拡張（network 制御・path scope 等）が必要になったとき、または codex が SKILL.md frontmatter 拡張 key の扱いを変えたとき

## Related

- `knowledge/adr/` — ADR 索引
- `.harness/config/agent-profiles.json` — capability → provider / model routing の SSoT（権限 field は設けない — 権限宣言は adapter 定義側、D3）
- `knowledge/conventions/responsibility-boundary.md` — 設定所有権の責任境界
