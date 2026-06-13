<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 32, yellow: 0, red: 0 }
---

# Codex を Claude と同等の SoTOHE オーケストレーターにする設定追加

## Goal

- [GO-01] SoTOHE template が Claude Code と Codex の両方を恒久的な root orchestrator 選択肢として持ち、利用者が `.harness/config/agent-profiles.json` の `capabilities.orchestrator.provider` で `claude` または `codex` を選べるようにする。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D3]
- [GO-02] Codex root orchestrator が SoTOHE の既存 `/track:*` workflow、phase ownership、review、DRY gate、hook policy を維持したまま運用できる project config、custom agents、skills、rules、hooks、運用文書、検証器 coverage を追加する。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D1, knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D2, knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D4, knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D5]

## Scope

### In Scope
- [IN-01] Codex project config surface を repository root に追加する: `.codex/config.toml` は Codex root session の既定 model、reasoning effort、sandbox、approval policy、hooks、subagent concurrency を定義し、`.codex/instructions.md` は SoTOHE root orchestration rules と specialist routing を定義する。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D1] [tasks: T001]
- [IN-02] Codex project-local rules を `.codex/rules/default.rules` に追加する。SoTOHE の canonical `cargo make` wrapper、read-only Git inspection、必要な Codex orchestration command prefix は repo として永続許可し、direct git mutation や destructive command は許可しない。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D1] [tasks: T001, T004, T006]
- [IN-03] Codex custom agent definitions を `.codex/agents/*.toml` に追加する。対象は `orchestrator`、`spec-designer`、`impl-planner`、`type-designer`、`adr-editor`、`review-fix-lead`、`dry-fix-lead` とし、agent TOML は role、sandbox、model、所有 artifact、境界、status contract を薄く定義する。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D2] [tasks: T002, T004]
- [IN-04] Codex specialist workflow を `.agents/skills` に配置する。`spec-designer`、`impl-planner`、`type-designer`、`adr-editor` の skill を root に追加し、既存 `review-fix-lead` / `dry-fix-lead` skill と同じ検証対象にする。agent TOML だけを置いて specialist workflow が空になる状態を禁止する。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D2, knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D5] [tasks: T002, T004]
- [IN-05] `.harness/config/agent-profiles.json` の既存 `capabilities.orchestrator.provider` で `codex` を選べることを template として明示する。新しい profile 階層や root-host 専用 schema は導入しない。必要に応じて Codex root 用 sample provider assignment を追加または更新する。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D3] [tasks: T003]
- [IN-06] Codex hooks coverage を `.codex/config.toml` から `.codex/hooks/sotp-hook.sh` へ接続し、hook adapter から既存 `bin/sotp hook dispatch <hook-id>` を呼ぶ。policy 本体は既存 `sotp hook dispatch` 側に残す。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D4] [tasks: T001, T004]
- [IN-07] 運用文書を Claude 固定から Claude/Codex 両対応へ更新する。対象は `track/workflow.md`、`.codex/instructions.md`、`.claude/rules/08-orchestration.md` 相当の root orchestration 説明であり、`/track:*` user-facing command surface は変更しない。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D1, knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D5] [tasks: T005]
- [IN-08] `verify-orchestra` を拡張し、Claude subagent 定義だけでなく Codex project config、project-local rules、custom agents、skill coverage、hook adapter を検証できるようにする。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D5] [tasks: T004]

### Out of Scope
- [OS-01] Claude Code orchestration surface の削除または非推奨化。template は `.claude` と `.codex` の両 surface を恒久的に保持する。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D1, knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D3] [tasks: T003, T005]
- [OS-02] SoTOHE core の phase command、TDDD signal 機構、review/DRY gate を Codex 専用に作り替えること。Codex root orchestrator は既存 `bin/sotp` / `cargo make` 経路を使う。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D1] [tasks: T001, T004]
- [OS-03] `spec-designer`、`impl-planner`、`type-designer`、`adr-editor` など全 specialist capability の provider を同時に Codex へ切り替えること。root orchestrator provider 選択と specialist provider assignment は別問題として扱う。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D3] [tasks: T003]
- [OS-04] temporary scratch、cache、未tracked draft を active repository configuration として扱うこと。ADR、spec、運用文書、verifier は tracked repository paths だけを durable reference とする。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D5] [tasks: T005, T007]
- [OS-05] Codex provider の認証情報、provider endpoint、個人 profile、machine-local notification/telemetry 設定を project `.codex/config.toml` に固定すること。project config は repository-shared defaults に限定する。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D1] [tasks: T001]

## Constraints
- [CN-01] `.codex/config.toml` は project-local config として安全に共有できるキーだけを使う。provider auth、base URL、machine-local notification/telemetry、profile selector など project-local config で無視されるか個人環境に属する設定は入れない。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D1] [tasks: T001, T004]
- [CN-02] `.codex/rules/default.rules` は least-privilege にする。`cargo make ci`、`cargo make track-commit-message`、`cargo make track-pr*` など SoTOHE canonical wrapper と read-only Git inspection は allow 対象にできるが、direct `git add` / `git commit` / `git push` / `git reset`、destructive shell、broad `bash -lc` allow は許可しない。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D1, knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D4] [tasks: T001, T004, T006]
- [CN-03] Codex custom agent TOML は薄い role definition に限定し、詳細 workflow は `.agents/skills/<name>/SKILL.md` に置く。`spec-designer`、`impl-planner`、`type-designer`、`adr-editor` の agent が存在する場合は対応 skill も必須とする。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D2] [tasks: T002, T004]
- [CN-04] root orchestrator provider 解決は `capabilities.orchestrator.provider` だけで表現する。`profile` という別階層、root-host 専用 schema、あるいは Codex 専用の parallel config map は導入しない。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D3] [tasks: T003]
- [CN-05] Codex hook adapter は既存 `bin/sotp hook dispatch` へ委譲する。direct git operation 抑止、test file deletion 抑止、skill compliance などの policy logic を Codex hook script 側へ重複実装しない。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D4] [tasks: T001, T004]
- [CN-06] `verify-orchestra` は fail-closed にする。Codex config/rules/agents/skills/hooks の必須 artifact が欠ける、agent と skill の対応が崩れる、rules が危険な allow を含む、hook adapter が `sotp hook dispatch` へ接続していない場合は error finding を出す。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D5] [tasks: T004]
- [CN-07] 運用文書と ADR は git history に残らない file path、scratch path、cache path を durable reference として書かない。未昇格 draft は実装の入力に使えても、tracked decision/reference にはしない。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D5] [tasks: T005, T007]
- [CN-08] Project-local Codex rules/config/hooks は trusted project でのみ読み込まれる前提を運用文書に残す。repo設定が存在しても未trusted checkout では user/system layer だけが有効になり得る。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D1] [tasks: T005]

## Acceptance Criteria
- [ ] [AC-01] `.codex/config.toml`、`.codex/instructions.md`、`.codex/rules/default.rules`、`.codex/hooks/sotp-hook.sh`、`.codex/agents/{orchestrator,spec-designer,impl-planner,type-designer,adr-editor,review-fix-lead,dry-fix-lead}.toml` が tracked repository files として存在する。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D1, knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D2] [tasks: T001, T002]
- [ ] [AC-02] `.codex/rules/default.rules` が SoTOHE canonical wrapper の command prefix を allow し、direct git mutation と broad/destructive shell command を allow していないことを `codex execpolicy check` または verifier test で確認できる。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D1] [tasks: T001, T004, T006]
- [ ] [AC-03] `.agents/skills/{spec-designer,impl-planner,type-designer,adr-editor}/SKILL.md` が tracked repository files として存在し、`.codex/agents/*.toml` の specialist agent から対応 workflow が空洞化していないことを確認できる。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D2, knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D5] [tasks: T002, T004]
- [ ] [AC-04] `.harness/config/agent-profiles.json` または tracked sample config で `capabilities.orchestrator.provider = codex` を表現できる。表現は既存 `capabilities` map 上の provider 値変更であり、新しい profile layer を必要としない。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D3] [tasks: T003]
- [ ] [AC-05] `track/workflow.md`、`.codex/instructions.md`、`.claude/rules/08-orchestration.md` が Claude 固定ではなく Claude/Codex の恒久的な root orchestrator 選択を説明している。`/track:*` command surface は既存のまま維持される。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D1, knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D5] [tasks: T005]
- [ ] [AC-06] Codex hook adapter の各 configured hook が `.codex/hooks/sotp-hook.sh` から `bin/sotp hook dispatch` へ接続している。hook adapter の smoke test または `verify-orchestra` が `block-direct-git-ops`、`block-test-file-deletion`、`skill-compliance` の接続を検証する。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D4] [tasks: T001, T004]
- [ ] [AC-07] `cargo make verify-orchestra` が Codex project config、project-local rules、custom agents、skill coverage、hook adapter を検証し、全必須 artifact が揃った状態で pass する。欠落 artifact または危険な rules entry に対する unit test が存在する。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D5] [tasks: T004, T006]
- [ ] [AC-08] tracked ADR、spec、運用文書、Codex config/rules/agents/skills に git history に残らない scratch path や cache path を durable reference として含めていない。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D5] [tasks: T005, T007]
- [ ] [AC-09] 最終的に `cargo make ci`、`cargo make verify-orchestra`、`bin/sotp review check-approved`、`bin/sotp dry check-approved` が pass し、review/DRY/PR workflow が既存 SoTOHE gate を通過する。 [adr: knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md#D5] [tasks: T006, T007]

## Related Conventions (Required Reading)
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/enforce-by-mechanism.md#Rules
- .claude/rules/10-guardrails.md#Git Policy

## Signal Summary

### Stage 1: Spec Signals
🔵 32  🟡 0  🔴 0

