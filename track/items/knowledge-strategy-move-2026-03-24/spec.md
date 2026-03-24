<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0"
---

# knowledge/strategy/ 移動 — 戦略文書を git 管理下に

## Goal

tmp/（gitignore 済み）にある戦略的 SSoT 文書を knowledge/strategy/ に移動し git 管理下に置く。
clone した人にも戦略文書が見え、ADR 自動導出のスキャン対象になる状態を実現する。

## Scope

### In Scope
- knowledge/strategy/ ディレクトリ新設 + README.md [source: discussion] [tasks: T001]
- TODO.md を knowledge/strategy/ に移動 [source: discussion] [tasks: T002]
- TODO-PLAN を日付サフィックス除去して移動 [source: discussion] [tasks: T002]
- vision を日付サフィックス除去して移動 [source: discussion] [tasks: T002]
- progress-tracker を日付サフィックス除去して移動 [source: discussion] [tasks: T002]
- TODO-PLAN-v4-draft を移動 [source: discussion] [tasks: T002]
- .gitignore に knowledge/ 除外ルール追加（knowledge/ は git 管理対象） [source: discussion] [tasks: T002]
- 全 git tracked ファイルの旧 tmp/ 戦略文書パス参照を修正（knowledge/adr/*.md, project-docs/conventions/*.md 含む。track/items/ は除外） [source: discussion] [tasks: T003]
- CLAUDE.md 参照追加 [source: discussion] [tasks: T004]
- memory 参照更新 [source: discussion] [tasks: T004]
- knowledge/adr/README.md の旧パス参照更新 [source: discussion] [tasks: T004]

### Out of Scope
- 分析レポート（sdd-comparison, adoption-candidates 等）の移動 [source: inference — フル knowledge/ 再編で対応]
- sotp adr suggest 実装 [source: inference — CC-SDD-01 完了後に着手]
- tmp/ の完全整理 [source: inference — 別トラック]
- .claude/docs/ → knowledge/ の移動 [source: inference — フル knowledge/ 再編で対応]

## Constraints
- knowledge/ は git 管理対象（.gitignore の除外ルールで明示） [source: discussion] [tasks: T002]
- 旧版ファイルは tmp/ に残す（gitignore のまま） [source: discussion] [tasks: T002]
- 日付サフィックスを除去し SSoT 化（git 履歴で変更日を管理） [source: discussion] [tasks: T002]

## Domain States

| State | Description |
|-------|-------------|
| StrategyDocument | 戦略文書（TODO, TODO-PLAN, vision, progress-tracker）。移動前は tmp/（gitignore）、移動後は knowledge/strategy/（git 管理） |
| CrossReference | ファイル間の相互参照リンク。旧パス（tmp/）→ 新パス（knowledge/strategy/）に更新 |

## Acceptance Criteria
- [ ] knowledge/strategy/README.md が存在する [source: discussion] [tasks: T001]
- [ ] knowledge/strategy/ に 5 ファイルが存在する（TODO.md, TODO-PLAN.md, vision.md, progress-tracker.md, TODO-PLAN-v4-draft.md） [source: discussion] [tasks: T002]
- [ ] ファイル名に日付サフィックスがない [source: discussion] [tasks: T002]
- [ ] git ls-files --error-unmatch で knowledge/strategy/ の全ファイルが tracked と確認できる [source: discussion] [tasks: T002]
- [ ] git tracked ファイル内に旧 tmp/ 戦略文書パスへの参照が残っていない（Grep で検証、track/items/ 配下の計画文書は除外） [source: discussion] [tasks: T003]
- [ ] CLAUDE.md に knowledge/strategy/ への参照がある [source: discussion] [tasks: T004]
- [ ] knowledge/adr/README.md 内の旧 tmp/ パス参照が新パスに更新されている [source: discussion] [tasks: T004]
- [ ] cargo make ci が全テスト通過する [source: convention — hexagonal-architecture.md] [tasks: T001, T002, T003, T004]

