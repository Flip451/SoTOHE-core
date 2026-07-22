---
adr_id: 2026-06-30-0425-harness-workflow-ssot-adapters
decisions:
  - id: D1
    user_decision_ref: "chat:2026-06-30:harness-workflow-ssot-adapters"
    status: proposed
  - id: D2
    user_decision_ref: "chat:2026-06-30:harness-workflow-ssot-adapters"
    status: proposed
  - id: D3
    user_decision_ref: "chat:2026-06-30:harness-workflow-ssot-adapters"
    status: proposed
  - id: D4
    user_decision_ref: "chat:2026-06-30:harness-workflow-ssot-adapters"
    status: proposed
  - id: D5
    user_decision_ref: "chat:2026-06-30:harness-workflow-ssot-adapters"
    status: proposed
  - id: D6
    user_decision_ref: "chat:2026-06-30:adr-review-harness-workflow-ssot-adapters"
    status: proposed
  - id: D7
    user_decision_ref: "chat:2026-06-30:adr-review-harness-workflow-ssot-adapters"
    status: proposed
  - id: D8
    user_decision_ref: "chat:2026-06-30:adr-review-harness-workflow-ssot-adapters"
    status: proposed
  - id: D9
    user_decision_ref: "chat:2026-06-30:adr-review-harness-workflow-ssot-adapters"
    status: proposed
  - id: D10
    user_decision_ref: "chat:2026-06-30:adr-review-harness-workflow-ssot-adapters"
    status: proposed
---
# Claude/Codex 運用文書の .harness SSoT 化

## Context

Claude 側 `.claude/commands` が現在の運用 SSoT だが、Codex 移植で `.agents/skills` に同じ内容を持つと二重管理になるため、provider 非依存の文書を `.harness` に移したい。

既に `2026-06-13-0002-codex-orchestrator-settings-addition.md` は、capability 単位の運用契約を `.harness/capabilities/` に置き、Claude subagent と Codex skill を薄い wrapper にする方針を定めている。本 ADR はその SSoT pattern を workflow 単位へ拡張する。

初期対象に `/track:adr2pr` を選ぶ理由は、同 command が ADR から PR review までの lane を横断し、`/track:init`、`/track:review`、`/track:commit`、Phase 1-3 writer、`/track:full-cycle`、`/track:pr-review` を順に踏むためである。`/track:adr2pr` を SSoT 化できれば、主要な track workflow と capability adapter の境界を一度に検証できる。

## Decision

### D1: provider 非依存 workflow を `.harness` に集約する

Claude/Codex 共通の運用 workflow は `.harness` 配下の provider 非依存文書を SSoT とし、`.claude/commands` と `.agents/skills` はその SSoT を参照する thin adapter にする。

### D2: workflow SSoT は `.harness/workflows/` 配下に配置する

provider 非依存の workflow SSoT は `.harness/workflows/` 配下に配置する。track 系 workflow は `.harness/workflows/track/` に置き、Claude command と Codex skill はこの文書を参照する。

### D3: adapter 側に workflow logic を重複記述しない

Claude command と Codex skill には workflow logic、状態遷移、gate 条件、失敗時の復旧手順を重複記述しない。provider 固有の呼び出し方法、ツール制約、報告形式だけを adapter 側に残す。

### D4: `/track:adr2pr` の実行経路を 1 track で芋づる式に移植する

1 つの track で `/track:adr2pr` と、それが参照するすべての Claude command / skill を芋づる式に移植する。移植対象は `/track:adr2pr` 単体に限定せず、実行時に参照される下位 workflow と capability adapter も同じ SSoT 化 track の範囲に含める。

### D5: provider ごとの呼び出し面は維持する

移植後も Claude 側 slash command 名は維持する。Codex 側は同名 slash command の複製ではなく、対応する Codex skill 名で呼び出せる adapter を提供する。

### D6: workflow SSoT は markdown 章構成と明示参照で始める

`.harness/workflows/**/*.md` は、既存の `.harness/capabilities/*.md` と同じく front-matter なしの markdown 章構成 prose として作成する。workflow SSoT は少なくとも `Mission`、`Inputs`、`Sequence`、`Gates`、`Failure / recovery`、`Outputs` に相当する章を持ち、provider 非依存の状態遷移と停止条件を本文に記述する。

adapter は冒頭で参照先 SSoT path を明示する。Claude command では `/track:*` のコマンド本文冒頭に `.harness/workflows/<path>.md` を operational SSoT として示し、Codex skill では `SKILL.md` の本文冒頭で同じ path を読むよう指示する。初期移行では `agent-profiles.json` に workflow 参照 schema を追加せず、path 参照は adapter 本文に置く。

### D7: workflow SSoT と capability SSoT の境界を定義する

capability は「専門家」の持つ文脈を定義し、workflow はオーケストレーターや専門家が従う手順を定義する。

`.harness/capabilities/` には、単一 capability (= 専門家) が保有する文脈を置く。対象は所有する artifact、責務範囲、入力 briefing の前提、内部 pipeline、書き込み境界、出力 contract など、専門家がその役割を果たすうえで前提とする情報である。手順そのものは置かない。

`.harness/workflows/` には、オーケストレーターや専門家が従う手順を置く。対象は入力、前提条件、状態遷移、複数 capability を順序付ける invocation 連鎖、gate、停止条件、復旧方針、全体の output contract である。workflow が capability を呼ぶときは、capability 内部の文脈を複製せず `.harness/capabilities/<name>.md` に委譲する。

`.harness/briefings/` は provider / scope 固有の review prompt や一時的な briefing injection を置く補助面として維持する。workflow SSoT や capability SSoT に昇格できる恒久手順が見つかった場合は、当該 workflow / capability 文書へ移す。

### D8: adapter に残せる provider 固有情報を whitelist 化する

adapter に残せる provider 固有情報は、(1) 呼び出し面の名前と起動方法、(2) provider 固有 tool / 権限制約、(3) subagent / skill / command の起動形態、(4) provider ごとの報告形式に限る。step 番号、gate 条件、状態遷移、失敗時復旧手順、commit / PR までの全体 sequence は workflow SSoT 側に置く。

Claude 側は既存 slash command 名を維持する。Codex 側は `/track:<slug>` を `track-<slug>` の skill 名へ写像する。たとえば `/track:adr2pr` の Codex adapter は `track-adr2pr` とし、Codex の skill mention surface が使える環境では `$track-adr2pr` として呼べる前提で文書化する。このリポジトリ自体は `$` prefix parser を実装しない。

`.claude/skills/` は今回の主要 adapter 面ではない。Claude の track workflow adapter は `.claude/commands/track/*.md` を維持し、`.claude/skills/` は既存の Claude skill 用途に残す。

### D9: `/track:adr2pr` 移植 track の閉包を明示する

`/track:adr2pr` SSoT 化 track の移植閉包は、(a) `/track:adr2pr` が直接列挙する下位 command、(b) その実行 plan を作るために直接読むよう指示される command section、(c) それらの command が直接 invoke / delegate する capability、(d) 当該 capability の Claude / Codex adapter である。

この閉包に含まれる capability で `.harness/capabilities/<name>.md` が存在しない場合、同じ track で capability SSoT を補う。既存 `.harness/capabilities/` にある capability は、その文書を正本として参照し、workflow 側へ内部手順をコピーしない。

### D10: adapter と SSoT の同期は review scope 用の専用 check で検証する

移植 track では、adapter と SSoT の整合性を、既存 `harness-policy` review scope の専用 check として検証する。

修正対象ファイルは `.harness/custom/review-prompts/harness-policy.md` のみで、ここに「adapter-SSoT 同期 check」項目を追加する。最小要件は以下である:

- adapter (`.claude/commands/track/*.md` および `.agents/skills/track-*/SKILL.md`) が冒頭で明示する `.harness/workflows/<path>.md` が実在すること。
- adapter 本文に workflow logic (step 番号、gate 条件、状態遷移、失敗復旧手順) が長文複製されていないこと。
- workflow SSoT (`.harness/workflows/**/*.md`) に provider 固有の起動細部 (subagent / skill 起動方法、provider 固有の sandbox / 権限 flag 等) が漏れ込んでいないこと。

`.harness/config/review-scope.json` の `harness-policy` グループは既に `.harness/**` / `.claude/commands/**` / `.agents/**` を patterns に含むため、scope 定義の変更は不要である。`bin/sotp` の一般的な workflow verifier や既存 architecture check への追加は行わない。一般機構として組むと運用文書の柔軟性を損なう枷になりやすく、また人間の注意だけを D3 の enforcement として扱わないために必要な最小範囲に絞るためである。

## Rejected Alternatives

### A. Claude command と Codex skill に workflow logic をそれぞれ記述する

Claude command と Codex skill に同じ workflow logic をそれぞれ記述する案。短期的には移植が早いが、gate 条件や復旧手順が分岐し、運用変更時に二重更新が必要になるため却下する。

### B. `.claude/commands` を引き続き SSoT とし、Codex skill が Claude command 文書を直接読む

`.claude/commands` を引き続き SSoT とし、Codex skill が Claude command 文書を直接読む案。既存文書を活かせるが、provider 非依存の運用文書が Claude 固有ディレクトリに残り、Codex 移植後も ownership が曖昧になるため却下する。

### C. `agent-profiles.json` に workflow 参照 schema を追加する

`agent-profiles.json` に workflow 参照フィールドを追加し、adapter から SSoT への mapping を設定ファイルへ集約する案。capability provider routing との一体感はあるが、今回必要なのは provider 選択ではなく workflow 本文の移設であり、schema 変更は初期移行の blast radius を広げる。まずは adapter 本文の明示 path 参照と review scope 用の専用 check で十分なため却下する。

### D. adapter-SSoT 同期 verifier を一般機構として広範に導入する

adapter と SSoT の path 参照や workflow logic 長文複製の検出を、`bin/sotp` の一般的な workflow verifier や既存 architecture check に組み込む案。設計時点で広範に組むと運用文書の柔軟性を損なうだけの枷になりやすい。最小要件は D10 で review scope 用の専用 check として絞り込み、それ以外の一般機構化は採用しない。一般機構化が必要になった時点で Reassess する。

## Consequences

### Positive

- Claude/Codex の運用手順が `.harness/workflows` に集約され、workflow logic の分岐や更新漏れを減らせる。
- Codex 移植時に adapter 側の記述を薄くでき、provider ごとの差分が呼び出し方法・承認・報告形式に限定される。
- `/track:adr2pr` のような複合 workflow で、参照される下位 command / skill も同じ track 内で整理できるため、移植後の実行経路を検証しやすい。
- capability SSoT が未整備の capability を、`/track:adr2pr` 移植の実行経路に沿って必要な分から補える。

### Negative

- 初期移行では `.claude/commands` から `.harness/workflows` へ手順を抽出する作業が必要になり、単純な Codex skill 追加より時間がかかる。
- 移行中は一時的に Claude command、Codex skill、`.harness` SSoT の整合を確認するレビュー負荷が増える。
- provider 固有の実行都合と provider 非依存 workflow の境界を誤ると、SSoT 側に Claude/Codex 固有の事情が漏れるリスクがある。
- review scope 用の adapter-SSoT 同期 check が配備されるまでは、D3 の重複禁止は review discipline に依存する期間が残る。

### Neutral

- Claude 側 slash command 名は維持されるため、既存の呼び出し体験は大きく変わらない。
- Codex 側は slash command の同名複製ではなく skill adapter として提供されるため、呼び出し面は provider ごとに異なる。
- `.harness/workflows` の導入後も、実際の検証や状態遷移は既存の `bin/sotp` / `cargo make` / track artifact に委ねる。
- `.harness/briefings/` と `.claude/skills/` はこの ADR の主要移行対象ではなく、必要に応じて後続 track で整理する。

## Reassess When

- Codex が provider 非依存 workflow 文書を安定して参照できず、adapter 側への重複記述が実運用で避けられないと判明したとき。
- `.harness/workflows` が大きくなりすぎ、workflow SSoT と capability SSoT の境界を再設計する必要が出たとき。
- Claude/Codex 以外の provider を追加し、現在の adapter 方式では routing や承認モデルを表現しきれなくなったとき。
- `bin/sotp` 側で workflow orchestration を機械化できる範囲が広がり、文書 SSoT より CLI SSoT のほうが安全になると判断したとき。
- provider 固有用語の混入を review scope check や review discipline で抑えきれず、workflow SSoT の provider 非依存性が維持できないと判明したとき。

## Related

- `knowledge/conventions/adr.md`
- `knowledge/conventions/pre-track-adr-authoring.md`
- `knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md`
- `.claude/commands/track/adr2pr.md`
- `.claude/commands/track/plan.md`
- `.agents/skills/`
- `.harness/capabilities/`
- `.harness/briefings/`
