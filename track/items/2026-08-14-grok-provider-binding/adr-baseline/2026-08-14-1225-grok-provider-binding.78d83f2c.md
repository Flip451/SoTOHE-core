---
adr_id: "2026-08-14-1225-grok-provider-binding"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-14 + chat_segment:grok-tui:2026-08-14 Phase0 境界承認 再収束全文"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-14 + chat_segment:grok-tui:2026-08-14 Phase0 境界承認 再収束全文"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-14 + grok-tui:2026-08-14 Codex の sandbox 拡張に相当する key は grok-sandbox。値は grok の --sandbox 語彙。Codex の sandbox キーは流用しない。.agents/ 共用は前提 + chat_segment:grok-tui:2026-08-14 D3 未宣言は診断時 read-only、dispatch は宣言欠如でもファイル欠如でも拒否 + chat_segment:grok-tui:2026-08-14 Phase0 境界承認 再収束全文 + chat_segment:grok-tui:2026-08-16 D3 review_finding_ref 除去の根拠昇格承認"
    candidate_selection: "from:[reuse-codex-sandbox-key, grok-sandbox, grok-skills-overlay] chose:grok-sandbox"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:grok-tui:2026-08-14 grok==grok は常に独立プロセス / grok を orchestrator host として認める + chat_segment:grok-tui:2026-08-14 Phase0 境界承認 再収束全文"
    candidate_selection: "from:[always-subprocess, contract-preserving-in-host, always-in-host] chose:always-subprocess; from:[allow-host, capability-only, defer] chose:allow-host"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:grok-tui:2026-08-14 grok は capability exec と typed-pipeline の両方の provider。typed-pipeline は専用経路に arm を足し capability exec へは合流させない。対象宇宙は agent-profiles.json に委譲 + chat_segment:grok-tui:2026-08-14 Phase0 境界承認 再収束全文"
    candidate_selection: "from:[capability-exec-only, both-execution-modes, defer-pathway] chose:both-execution-modes"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:grok-tui:2026-08-14 grok adapter は互換で読める .agents/ を優先し、.grok/ への定義は残差だけ + chat_segment:grok-tui:2026-08-14 Phase0 境界承認 再収束全文"
    candidate_selection: "from:[project-.grok, harness-neutral, defer-path, reuse-agents-minimize-grok] chose:reuse-agents-minimize-grok"
    status: proposed
  - id: D7
    user_decision_ref: "chat_segment:grok-tui:2026-08-14 shipped default は grok を指さない。grok 向け sample profile と adapter の sandbox/権限宣言例を同梱する + chat_segment:grok-tui:2026-08-14 Phase0 境界承認 再収束全文"
    candidate_selection: "from:[default-untouched-no-sample, ship-sample-profile, defer-defaults] chose:ship-sample-profile"
    status: proposed
  - id: D8
    user_decision_ref: "chat_segment:grok-tui:2026-08-14 capability は開集合。網羅対象は名前ではなく既存 dispatch 契約（model / effort / resume）の grok 写像 + chat_segment:grok-tui:2026-08-14 Phase0 境界承認 再収束全文"
    candidate_selection: "from:[closed-current-capability-list, open-profile-universe-plus-dispatch-contract] chose:open-profile-universe-plus-dispatch-contract"
    status: proposed
  - id: D9
    user_decision_ref: "chat_segment:grok-tui:2026-08-14 grok host のガードは .grok/hooks を正規面とし、dispatch は grok 封筒と tool 名を既存契約へ写す。実装は後続 track + chat_segment:grok-tui:2026-08-14 Phase0 境界承認 再収束全文"
    candidate_selection: "from:[adr-and-track, adr-only, session-local-hooks] chose:adr-only"
    status: proposed
---
# grok を第三の provider binding として追加する

## Context

既存の provider 追加経路は、Codex の custom provider 設定を通す形であり API キーを要する。grok は消費者サブスクリプションで CLI にサインインする認証形態を持ち、この経路には乗らない。そのため provider adapter の追加として扱う。

実測（2026-08-14、headless 実行 2 回）で確認した性質は次のとおり。

- briefing はファイルで渡せる
- 返却は envelope に構造化出力の専用フィールドと、その失敗理由フィールドを持ち、スキーマ適合の値はそこに現れる
- 診断出力は標準エラーにほとんど出ず、envelope に集約される
- 共有プロセスへの接続を無効化する起動指定がある
- モデルは既定が実行時に変わりうる

## Decision

### D1: grok を provider として追加し、返却は構造化出力フィールドから受け取る

capability の実行体として grok CLI を追加する。briefing はファイル指定で渡し、返却スキーマは起動時に渡して構造化出力を要求する。抽出は envelope の構造化出力フィールドのみを読み、値が無い場合は同 envelope の失敗理由を診断として fail-closed とする。envelope 内のテキスト欄は途中経過を含むため、返却の抽出には用いない。

### D2: セッション独立性と再現性を起動指定で担保する

共有プロセスへの接続を無効化して起動し、capability ごとに独立したセッションとする。モデルは profile の値を明示的に渡し、実行時の既定選択に委ねない。reasoning effort は既存の effort 語彙から供給する。

### D3: 権限は grok が解決した adapter 定義に宣言し、未宣言は fail-closed とする

grok 向け capability の権限（読み取り専用か書き込み可か）は、D6 で解決した adapter 定義の frontmatter に宣言し、dispatch が `grok --sandbox` へ機械変換する。

Codex の SoTOHE 拡張 `sandbox:`（`read-only` / `workspace-write`）は grok の sandbox 語彙（`read-only` / `workspace` / `strict` および project profile）と一致しない。同じキーに grok の値を混ぜず、Codex 値からの暗黙変換もしない。相当する独自拡張 key の名前は `grok-sandbox` とする。値は grok の `--sandbox` 語彙とする。`off`（制限なし）は全権相当として受理しない。

<!-- illustrative, non-canonical -->
```yaml
# .agents/skills/<name>/SKILL.md
---
name: example
description: ...
sandbox: workspace-write   # Codex
grok-sandbox: workspace    # grok
---
```

`grok-sandbox` が未宣言の場合、sandbox 値の解決結果は `read-only` に倒す。ただし、宣言を欠く adapter 定義への dispatch は fail-closed として拒否するため、この既定値は診断・検証時の解決値であり、実行の受理を意味しない。adapter 定義ファイル自体が存在しない場合も dispatch を拒否する。`.grok/skills/` に同名 skill を置いてこの宣言だけを分離しない — Grok の skill 発見は `.grok/skills/` を優先し、既存の `.agents/skills/` 本文を隠す。

### D4: grok==grok は常に独立プロセスとし、grok を orchestrator host として認める

grok は capability の実行体であると同時に orchestrator host にもなりうる。host が grok で capability の provider も grok のときは、in-host 委譲を行わず、常に独立した grok プロセスを起動する。

これは D2 の「共有プロセスを無効化し、capability ごとに独立セッションとする」を host 一致時にも例外なく適用するものである。同一セッションの subagent は D2 と衝突するため正規経路にしない。

既存の capability exec 規則（契約を保持できる一致だけ in-host、codex==codex は subprocess）は変更しない。grok は Codex と同じく「一致しても subprocess」側に置く。根拠は D2 のセッション独立性であり、in-host で契約を束縛できるかどうかではない。

### D5: grok は両 execution_mode の provider になり、capability 名は列挙しない

grok は `agent-profiles.json` の `execution_mode` が `orchestrator-output` の capability にも `typed-pipeline` の capability にも provider として載る。

呼び出せる capability の宇宙は `agent-profiles.json` である。名前をこの ADR にも実装にも列挙しない。profile に存在し、対象 provider の adapter 定義（D6）がある名前は、harness 組み込みかどうかを問わず grok に routing できる。存在しない名前、または adapter 定義を欠く名前は fail-closed とする。

`orchestrator-output` は既存の `capability exec` に grok adapter arm を 1 本追加して起動する。新しい `orchestrator-output` 名の追加に arm の増設は要らない。`typed-pipeline` は専用経路に grok の起動契約（D1・D8）を載せる。typed-pipeline を `capability exec` に合流させない。専用経路の種類を増やすのは新しい機械消費 pipeline を足すときだけであり、capability 名の追加ではない。

### D6: 互換で賄える定義は `.agents/` を使い、`.grok/` は残差だけにする

Grok はリポジトリの `.agents/skills/`（および `commands/`）を既に発見する。capability の存在確認・本文は、この面を grok の adapter 定義として使う。同じ skill を `.grok/skills/` に複製しない。

D3 の `grok-sandbox` は、この共有 skill の frontmatter に Codex の `sandbox` とは別に足す。本文は増やさない。

`.grok/` に置くのは、`.agents/` が担えない面だけである。現状の残差は D9 の hooks に限る。dispatch は解決した定義の存在を起動前に検証し、欠如は fail-closed とする。`.harness/capabilities/` の provider 非依存 SSoT は動かさない。

### D7: shipped default は grok を指さず、sample profile と宣言例を同梱する

shipped default の `agent-profiles.json` は grok を指さない。grok 向けの sample profile を既存の sample 群と同列に同梱し、adapter 定義には sandbox / 権限の宣言例を入れる。採否は consumer の責任であり、CI で grok を強制しない。

### D8: grok は既存の dispatch 契約（model / effort / resume）を写像する

grok の起動は、既存の provider 非依存な dispatch 契約を grok の起動指定へ写す。capability ごとの例外表は持たない。

- **model**: profile の当該 capability の `model` だけを権威とする。起動時に明示し、実行時の既定選択に落とさない（D2）。adapter 定義が model を宣言する場合は profile 値の投影として完全一致を検証し、欠如・不一致は fail-closed。
- **effort**: profile から解決した reasoning effort を起動指定へ渡す。解決できない dispatch は fail-closed とする。grok が受理する effort 語彙は `agent-profiles.json` の provider 宣言に置き、未対応の組み合わせは既存の provider × effort 検証で拒否する。
- **resume**: 既存の session 再開契約を grok でも使う。同一作業の再入では session を再開し、初回と関心事の切り替えは新規 session とする。再開は独立した grok プロセスとして起動し、共有プロセスへは接続しない（D2・D4）。再開時も model / effort / 権限の起動指定を毎回明示し、前回 session の引き継ぎに依存しない。再開失敗・期限切れ・provider または model の不一致は新規 session へ fallback し、dispatch 自体は止めない。

既存契約が検査対象外としている経路（hosted 側実行で dispatch から model / effort を注入できないもの）は対象外のままとする。例外を grok 側で増やさない。

### D9: grok host のガードは `.grok/hooks/` を正規面とし、dispatch は grok の封筒と tool 名を既存契約へ写す

grok を orchestrator host として使うときの PreToolUse / UserPromptSubmit ガードは、プロジェクトの `.grok/hooks/` に grok 固有語彙で宣言する。宣言が起動する hook 名は既存の `sotp hook dispatch` と同じ（hooks-path-setup / block-direct-git-ops / block-test-file-deletion / skill-compliance）である。policy 本体は既存 handler であり、grok 用に別 policy を持たない。

`sotp hook dispatch` は grok の封筒（camelCase の tool 名と入力、Grok の tool 識別子）を、既存の Claude 封筒と同じ内部契約へ写してから handler に渡す。写像不能な封筒は fail-closed とする。

`.claude/settings.json` は Claude 面として残す。

### Existing decision relationship

本 ADR の D3 / D6 は `2026-07-12-0510-capability-exec-unified-dispatch.md` D3（権限は provider ごとの adapter 定義に置く）に grok の写像を加えるものであり、同文書の D2 の routing 分担と、同文書 D3 の写像未定義時 fail-closed は変更しない。Grok が `.agents/skills/` を発見する範囲では、その面を grok の adapter 定義として再利用する。本 ADR の D4 は同文書 D7（契約を保持できる一致だけ in-host、codex==codex は subprocess）に grok の分岐を加えるものであり、同文書 D7 自体は変更しない。本 ADR の D5 は同文書 D2（呼び出せる宇宙は `agent-profiles.json`、名前のハードコード禁止）と D9（typed-pipeline を `capability exec` に合流させない）を変更しない。D5 はまた `2026-07-24-0326-consumer-convention-ownership-and-harness-decoupling.md` D5（capability ID は open-ended）を変更しない。本 ADR の D8 は `2026-07-12-0510-capability-exec-unified-dispatch.md` D5（model の権威は profile）と `2026-07-13-2217-agent-dispatch-cost-reduction.md` D1 / D2 / D4（effort 明示・session 再開・再開時の flag 再指定）および `2026-08-02-0151-codex-reasoning-effort-max.md` D1（effort 語彙と provider × effort 検証）の grok 写像であり、それらの決定は変更しない。本 ADR の D7 は `2026-08-02-0151-multi-provider-capability-routing.md` D2（設定例の同梱）と D3（既定は外部プロバイダーを指さない、採否は consumer）と同型である。本 ADR の D9 は `2026-06-12-1518-hooks-path-setup-fail-closed.md` D1〜D3（agent 実行面の hooksPath preflight、handler 分離、installed sotp 解決）を変更せず、grok host への写像を足す。本 ADR は `2026-08-02-0151-multi-provider-capability-routing.md` D1（Codex custom provider 経路）を置き換えず、認証形態の異なる provider のための併存経路を加える。

## Rejected Alternatives

- **API キーを取得して既存の custom provider 経路に載せる**: サブスクリプションと API は別課金であり、サブスクリプションを活かす動機が失われる。
- **envelope のテキスト欄から返却を抽出する**: テキスト欄は turn ごとの途中経過を連結して含み、最終値の特定規則を新たに定める必要がある。構造化出力フィールドを読めば規則は不要。
- **モデル指定を既定に委ねる**: 実行ごとに異なるモデルが選ばれ、verdict の再現性と telemetry の比較軸が失われる。
- **契約を保てるときだけ in-host**: D2 の独立セッションに対する例外になり、host 一致時だけ共有プロセスが残る。
- **常に同一セッションの subagent**: D2 を host 一致時に無効化する。
- **この ADR では capability 実行体に限り、orchestrator host 化は別 ADR にする**: 今回の裁定で grok を host として認めるため、呼び方の本番経路をこの ADR に書く。
- **capability exec だけに載せる**: typed-pipeline の機械消費を grok で再現できず、D1 の抽出契約が片方の経路にしか効かない。
- **どのコマンド面に載せるかを実装時に残す**: host 化と両 mode 対応が未決のまま実装へ落ち、経路選択が実装者判断になる。
- **adapter 定義を `.harness` の provider 非依存面に置く**: grok 固有の権限語彙が中立面へ漏れる。
- **全 capability の skill を `.grok/skills/` に複製する**: Grok が既に読む `.agents/skills/` と二重保守になる。
- **Codex の `sandbox` キーと値を grok に流用する**: 語彙が違い（`workspace-write` は grok の profile ではない）、暗黙変換が drift する。
- **`.grok/skills/` に権限だけの同名 skill を置く**: Grok の発見順で既存 `.agents/skills/` 本文を隠す。
- **adapter の配置を決めない**: D3 の「定義ファイル」がどこを指すか不定のまま残る。
- **写像だけ用意し sample も同梱しない**: consumer が grok を選ぶための出発点がなく、D7 の「選択肢を提供する」が空になる。
- **既定 profile / sandbox を実装時に残す**: 同梱物の範囲が実装者判断になる。
- **現行 snapshot の capability 名を閉じた実装範囲として列挙する**: 宇宙は profile であり、未想定の custom capability を範囲外にしてしまう。
- **grok では resume しない**: 既存の再入契約を grok だけ欠き、同一作業の再 dispatch が毎回新規文脈になる。
- **effort 未指定を grok 既定に fail-open する**: 推論深度が実行時既定に隠れ、D2 の明示指定と衝突する。
- **resume 時に model / effort / 権限の再指定を省略する**: 前回 session の引き継ぎに依存し、provider や版で sandbox 逸脱が起きうる。
- **Grok では Claude 互換 hooks だけを使い、`.grok/hooks/` を置かない**: 未設定 `${VAR}` で実行前失敗し、ガードが fail-open のまま残る。
- **grok 用に別の git / テスト削除 policy を書く**: 既存 handler と drift する。

## Consequences

- 良: サブスクリプション契約のまま capability を実行できる。返却の抽出が envelope の専用フィールドで閉じ、自由文からの探索が不要になる。
- 良: 診断出力が標準エラーへ大量に流れないため、出力量に起因する実行の不安定さを持ち込まない。
- 負: provider adapter と権限写像が 1 つ増え、保守対象になる。
- 負: grok==grok でもプロセスが増え、同一 UI セッションの文脈共有は捨てる。
- 負: typed-pipeline の専用経路ごとに grok の起動契約を載せる必要があり、保守面が `capability exec` 1 本より広い。
- 良: 新しい `orchestrator-output` 名は profile と既存の `.agents/skills/` 定義だけで grok に載る。共有定義で足りない残差だけ `.grok/` に足す。
- 良: grok host でも同じ git / テスト削除ガードが効く。
- 負: hook 封筒と tool 名の写像が grok CLI 契約に束縛され、CLI 変更時に直す。
- 中立: サブスクリプションのレート制限は未検証であり、艦隊運用での位置づけは実運用の計測で定める。
- 中立: shipped default は grok を指さない。sample を選んだ consumer だけが grok を使う。

## Reassess When

- 構造化出力フィールドの契約（失敗時の表現を含む）が変わったとき。
- サブスクリプションのレート制限が艦隊運用の制約として顕在化したとき。
- grok が既存の custom provider 経路でも同等に扱えるようになったとき（経路統合の是非を再検討する）。
- grok の in-host 起動が、共有プロセスを使わずに D2 の独立セッションを満たせるようになったとき。
- grok の project-local 定義面の配置契約が変わったとき。
- typed-pipeline の返却契約が、構造化出力フィールドだけでは足りなくなったとき。
- grok の session 再開契約、または受理する effort 語彙が変わったとき。
- grok の hook 封筒または tool 識別子が変わり、既存契約への写像が壊れたとき。
- grok の `--sandbox` 語彙または profile 意味が変わり、`grok-sandbox` が書けなくなったとき。
