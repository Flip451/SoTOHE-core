---
adr_id: 2026-06-13-0002-codex-orchestrator-settings-addition
decisions:
  - id: D1
    user_decision_ref: "chat_segment:codex-orchestrator-settings-addition:2026-06-13"
    candidate_selection: "from:[codex-root-orchestrator, claude-root-with-codex-specialists, deep-core-rewrite] chose:codex-root-orchestrator"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:codex-orchestrator-settings-addition:2026-06-13"
    candidate_selection: "from:[thin-agents-plus-skills, monolithic-agent-prompts, agents-only-no-skills] chose:thin-agents-plus-skills"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:codex-orchestrator-settings-addition:2026-06-13"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:codex-orchestrator-settings-addition:2026-06-13"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:codex-orchestrator-settings-addition:2026-06-13"
    status: proposed
---
# Codex を Claude と同等の SoTOHE オーケストレーターにする設定追加

## Context

現行の SoTOHE orchestration は、Claude Code を user-facing orchestrator とし、`.harness/config/agent-profiles.json` で `orchestrator` / `spec-designer` / `impl-planner` / `type-designer` / `adr-editor` を Claude provider に割り当てている。一方、`reviewer`、`review-fix-lead`、`dry-checker`、`dry-fix-lead` などは既に Codex provider を利用しており、provider 切替の基盤自体は存在する。

今回の判断対象は、Codex を reviewer/fixer の補助 provider に限定せず、Claude Code と同等に SoTOHE の root orchestration を担える恒久的な template 選択肢として追加するための設定である。これは Claude Code を廃止して Codex に一本化するための判断ではなく、template 利用者が root orchestrator として Claude Code か Codex を選べる状態を継続的にサポートする判断である。事前検討では、Codex project config、custom agent 定義、skill draft、hook adapter、provider map draft の構成を確認した。この構成は、主に設定ファイルと運用文書の追加・更新で成立し、SoTOHE core の phase command や TDDD signal 機構へ深く立ち入らずに実現できることを示している。

Codex の公開仕様上も、project-scoped `.codex/config.toml`、project-scoped `.codex/agents/*.toml`、`.agents/skills`、hooks はそれぞれ公式の拡張面として存在する。したがって、Codex 側の同等機能は Claude Code の subagent 定義をそのまま Rust 機構へ移植するのではなく、Codex の設定面に合わせて追加する方が自然である。

## Decision

### D1: Codex root orchestrator は `.codex` project config で有効化する

Codex を SoTOHE の user-facing root orchestrator として扱えるように、repository root に `.codex/config.toml` と `.codex/instructions.md` を追加する。`.codex/config.toml` は Codex root session の既定 model、reasoning effort、sandbox、approval policy、hooks、subagent concurrency を定義し、`.codex/instructions.md` は SoTOHE 固有の orchestration rules と specialist routing を定義する。

この `.codex` surface は Claude Code surface の廃止準備ではない。template は `.claude` と `.codex` の両方を正規の orchestration surface として保持し、選択された root provider に応じてどちらを使うかを決める。

この変更は、SoTOHE core の phase command を置き換えるものではない。Codex orchestrator は、既存の `bin/sotp` と `cargo make` wrapper を呼び出して Phase 0-3、review、DRY gate、track PR 操作を進める。root session の責務は、関連 ADR / track artifact / `AGENTS.md` を読んで specialist work を割り当て、出力を統合し、track を進められるか判断することである。

### D2: Claude subagent 相当の Codex custom agents + skills を追加する

Claude provider 向けの `.claude/agents/*.md` は残しつつ、Codex provider 向けには `.codex/agents/*.toml` と `.agents/skills/<name>/SKILL.md` の組み合わせで同等の specialist surface を追加する。

Codex custom agent TOML は、agent 名、説明、model、sandbox、所有 artifact、境界、最終 status contract などの薄い role definition を持つ。一方、実際の specialist workflow、必読資料、command discipline、self-verification、final response format は skill 側へ移す。agent 定義を薄くするだけでは specialist 能力が空になるため、skill の root 配置は agent の root 配置と同時に行う。

Codex 側で対応させる capability は次の通り:

- `orchestrator`: root Codex session + `.codex/agents/orchestrator.toml`
- `spec-designer`: `.codex/agents/spec-designer.toml` + `.agents/skills/spec-designer`
- `impl-planner`: `.codex/agents/impl-planner.toml` + `.agents/skills/impl-planner`
- `type-designer`: `.codex/agents/type-designer.toml` + `.agents/skills/type-designer`
- `adr-editor`: `.codex/agents/adr-editor.toml` + `.agents/skills/adr-editor`
- `review-fix-lead`: `.codex/agents/review-fix-lead.toml` + 既存 `.agents/skills/review-fix-lead`
- `dry-fix-lead`: `.codex/agents/dry-fix-lead.toml` + 既存 `.agents/skills/dry-fix-lead`

### D3: `.harness/config/agent-profiles.json` の `orchestrator` capability を Codex に切り替え可能にする

`.harness/config/agent-profiles.json` の現行 schema は `providers` と `capabilities` の対応表であり、別の設定階層を導入しない。Codex root orchestration の provider 解決は、既存の `capabilities.orchestrator.provider` を `codex` に設定できるようにすることで表現する。

本 ADR が要求するのは、`capabilities.orchestrator.provider` で `claude` と `codex` のどちらも正規に選べることである。

この decision は root orchestrator の host 選択を扱う。`spec-designer`、`impl-planner`、`type-designer`、`adr-editor` などの specialist capability を同時に Codex provider へ切り替えることは必須条件にしない。Codex root orchestrator が specialist work を行う場合は D2 の `.codex/agents` + `.agents/skills` を使えるが、`.harness/config/agent-profiles.json` 側で各 specialist provider をどう割り当てるかは、別途の provider assignment として扱う。

### D4: hook coverage は Codex hooks から既存 `sotp hook dispatch` へ接続する

Codex 用の hook coverage は `.codex/config.toml` の inline hooks から `.codex/hooks/sotp-hook.sh` を呼び、そこから既存の `bin/sotp hook dispatch <hook-id>` に接続する。これにより、direct git operation の抑止、保護された test file deletion の抑止、skill compliance など、既存の SoTOHE hook policy を再利用する。

ただし、Codex hooks は Claude Code hooks と完全に同じ実行 semantics ではないため、有効化前に hook stdin / output / trust / matcher semantics を検証する。hook adapter は設定面の bridge とし、policy 本体は `bin/sotp hook dispatch` 側に残す。

### D5: Codex orchestrator の有効化前提条件を明示する

`capabilities.orchestrator.provider` を `codex` に設定できることと、その状態を SoTOHE の active orchestration として扱えることは別である。Codex orchestrator を active と扱う前に、次の有効化前提条件を満たす。

1. `.codex/config.toml`、`.codex/instructions.md`、`.codex/agents/*.toml` が repository root に配置されている。
2. `spec-designer`、`impl-planner`、`type-designer`、`adr-editor` の skill が `.agents/skills` に promoted され、既存の `review-fix-lead` / `dry-fix-lead` skill と同じ検証対象になっている。
3. `.harness/config/agent-profiles.json` の `capabilities.orchestrator` が Codex provider を選択できる。
4. `track/workflow.md`、`.codex/instructions.md`、`.claude/rules/08-orchestration.md` 相当の運用文書が、Claude 固定ではなく Codex root orchestration を説明している。
5. `verify-orchestra` が Claude subagent 定義だけでなく Codex project config、custom agent、skill coverage、hook adapter を検証できる。
6. hook adapter の挙動が `bin/sotp hook dispatch` の期待と一致することを実行確認している。

これは SoTOHE の実行時 gate ではなく、Codex を root orchestrator として選んだ場合の運用を active と呼ぶための受け入れ条件である。この条件を満たす前は、未昇格のローカル設計資料や部分的な設定変更を active repository configuration として扱わない。条件を満たした後も、Claude root orchestration は同じ template の恒久的な選択肢として残る。

## Rejected Alternatives

### A. Claude Code orchestrator を維持し、Codex は reviewer/fixer のままにする

現状維持に近く、既存の安定性は高い。しかし、今回の目的である「Codex を Claude と同等の root orchestrator として設定可能にする」を満たさない。Codex の custom agents、skills、hooks、project config を使う設計検証にも進めないため却下する。

### B. SoTOHE core の orchestration 機構を大きく作り替える

Rust 側に provider abstraction や phase orchestration をさらに深く実装し、Codex / Claude の差を core mechanism で吸収する案。

却下理由: 既存の `bin/sotp` phase command と provider map は既に orchestration の土台として機能している。今回必要なのは Codex の project config、custom agent、skill、hook、運用文書、検証器認識の追加であり、core mechanism の大改造はスコープ過大である。

### C. `.codex/agents/*.toml` だけに全 workflow を詰め込む

Claude subagent 定義の長い本文を Codex custom agent TOML の `developer_instructions` にそのまま移す案。

却下理由: Codex custom agent は role definition と実行設定に向くが、SoTOHE specialist の詳細 workflow、必読資料、command discipline、self-verification は skill として分離した方が再利用・検証・保守しやすい。agent だけを薄く置くと specialist 定義が空洞化するため、skills を同時に置く必要がある。

### D. 単一の Codex root prompt だけで全 specialist workflow を運用する

`.codex/instructions.md` だけに全ルールを集約し、custom agents や skills を作らない案。

却下理由: spec authoring、type catalogue design、implementation planning、ADR editing、review-fix、DRY fix は所有 artifact と failure mode が異なる。単一 prompt に集約すると境界が曖昧になり、Phase writer の専属性、generated artifact discipline、status contract が弱くなる。

### E. Codex の import flow だけで Claude setup を自動取り込みする

Codex の import flow に任せて `.claude` 定義を取り込み、手動設計を最小化する案。

却下理由: import flow は定義取り込みの入口として有用だが、SoTOHE の SoT Chain、Phase 1-3 の writer ownership、ADR escalation gate、TDDD catalogue checklist、hook policy は repository 固有の厳密な意味を持つ。自動 import 結果をそのまま active にするのではなく、repository 固有の ADR と有効化前提条件で明示的に固定する必要がある。

## Consequences

### Positive

- Codex を root orchestrator として起動しても、Claude Code と同じ SoTOHE phase ownership を保てる。
- 変更の中心が `.codex`、`.agents/skills`、`.harness/config/agent-profiles.json`、運用文書、`verify-orchestra` に限定され、SoTOHE core mechanism への侵襲を抑えられる。
- Claude subagent の濃い workflow と同等の内容を Codex skills として表現するため、agent TOML が薄くなっても specialist 能力は失われない。
- Codex hooks は既存 `sotp hook dispatch` policy を再利用するため、provider ごとに guardrail logic を重複実装しない。
- `review-fix-lead` / `dry-fix-lead` は既存 Codex skills を再利用でき、追加対象を Phase writer と ADR editor の skill root 配置に絞れる。

### Negative

- Template として Claude と Codex の両 orchestration surface を恒久的にサポートするため、運用文書と verifier が両対応になり、保守対象が増える。
- `verify-orchestra` は Claude subagent 前提から、Codex config / agents / skills / hooks も理解する形へ拡張が必要になる。
- Codex hook semantics と Claude hook semantics の差分確認が必要で、trust / matcher / stdin-output 契約の検証を省けない。
- agent TOML と skill の対応が崩れると、形式上は agent が存在しても実質的な specialist workflow が空になる。

### Neutral

- `/track:*` の user-facing command surface は変更しない。root host が Claude か Codex かに関わらず、SoTOHE phase command は既存の `bin/sotp` / `cargo make` 経路を使う。
- `researcher` を Gemini provider に残すことは Codex root orchestration と矛盾しない。
- 未昇格のローカル設計資料は有効化まで active config ではない。

## Reassess When

- Codex custom agents、skills、hooks、project config の仕様が変わり、本 ADR の mapping が公式仕様と合わなくなったとき。
- Codex subagent workflow が自動 delegation やより強い workflow packaging を提供し、`.codex/agents` + `.agents/skills` の分割を見直せるようになったとき。
- 両 orchestration surface の保守コストが高くなり、検証責務の分割や共通化を見直す必要が出たとき。
- Codex root orchestration の dogfooding で、Phase writer の境界違反、hook coverage gap、skill compliance gap が繰り返し発生したとき。

## Related

- `.harness/config/agent-profiles.json` — capability/provider routing SSoT。
- `.claude/agents/` — 現行 Claude subagent 定義。
- `.codex/` — Codex project config、instructions、custom agent 定義の配置先。
- `.agents/skills/` — Codex skill の配置先。
- `knowledge/conventions/adr.md` — ADR front-matter と decision lifecycle の規約。
- `knowledge/conventions/pre-track-adr-authoring.md` — ADR の pre-track authoring と track artifact からの参照方向。
- `knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md` — review-fix-lead provider 選択可能化。
- `knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md` — Codex review-fix-lead の nested session / sandbox 方針。
