# Phase 進捗管理表 v3

> **作成日**: 2026-03-22
> **前版**: `tmp/archive-2026-03-20/progress-tracker-2026-03-20.md` (v2)
> **計画**: [`knowledge/strategy/TODO-PLAN.md`](TODO-PLAN.md)
> **ビジョン**: [`knowledge/strategy/vision.md`](vision.md)

---

## 全体サマリー

| Phase | 状態 | 項目 | 完了 | 残 | 推定日数 |
|---|---|---|---|---|---|
| 0 | ✅ | 1 | 1 | 0 | — |
| 1 | ✅ | 10 | 10 | 0 | — |
| **1.5** | good enough | 30 | 16 | 12 (延期) + 2 planned | 4 日 |
| **2** | **▶** | 7 | 6 | 1 (planned) | 0.5 日 |
| 3 | — | 12 | 0 | 12 | 5 日 |
| 4 | 一部 | 8 | 2 | 6 | 3 日 |
| 5 | — | 7 | 0 | 7 | 3 日 |
| **合計** | | **75** | **35** | **40** | **~15.5 日** |

---

## Phase 1.5: ハーネス自身のコード品質改善 (good enough)

| Track | ID | 規模 | 状態 | 完了日 | PR |
|---|---|---|---|---|---|
| A0 | `remove-file-lock-system` | M | ✅ | 03-19 | #41 |
| A | `domain-type-hardening` | M | ✅ | 03-19 | #42 |
| B | `ci-guardrails-phase15` | S | ✅ | 03-20 | #46 |
| C | `review-usecase-extraction` | L | ✅ | 03-22 | #47 |
| C' | `cli-review-module-split` | S | ✅ | 03-22 | #49 |
| D | `pr-usecase-extraction` | L | — | | |
| E | `activate-module-split` | M | — | | |
| F | `parser-consolidation` | M | — | | |
| G | `structural-lockdown` | S | — | | |
| H | `usecase-purity-ci` (INF-15) | S | ✅ | 03-22 | #50 |
| H' | `pr-review-hexagonal` (INF-16) | S | ✅ | 03-22 | #51 |
| H'' | `usecase-purity-error` (INF-17) | S | ✅ | 03-23 | #52 |
| I | `domain-purity-ci` (INF-19) | S | ✅ | 03-23 | #53 |
| J | `conch-parser-infra-move` (INF-20) | M | ✅ | 03-23 | #54 |
| K | `review-verdict-autorecord` (RVW-10/11) | M | ✅ | 03-25 | #63 |
| L | `review-infra-quality` (RVW-13/15/17) | M | ✅ | 03-25 | #64 |
| M | `autorecord-stabilization` (WF-59) | M | planned | | |
| N | `tamper-proof-review` (WF-43) | L | planned | | |

**推奨実行順**: ~~A0~~ → ~~A~~ → ~~B~~ → ~~C~~ → ~~C'~~ → ~~H~~ → ~~H'~~ → ~~H''~~ → ~~I~~ → ~~J~~ → ~~K~~ → ~~L~~ → M → N → E → D → F → G

---

## Gantt

```mermaid
gantt
    title SoTOHE-core Roadmap v3
    dateFormat YYYY-MM-DD
    axisFormat %m/%d

    section Phase 1.5 Harness Quality
    A0 remove-file-lock        :done, a0, 2026-03-19, 1d
    A domain-type-hardening    :done, a, 2026-03-19, 1d
    B ci-guardrails            :done, b, 2026-03-20, 1d
    C review-usecase-extract   :done, c, 2026-03-22, 1d
    C' cli-review-module-split :done, c2, 2026-03-22, 1d
    H usecase-purity-ci        :done, h, 2026-03-22, 1d
    H' pr-review-hexagonal     :done, h2, 2026-03-22, 1d
    H'' usecase-purity-error   :done, h3, 2026-03-23, 1d
    I domain-purity-ci         :done, i, 2026-03-23, 1d
    J conch-parser-infra-move  :done, j, 2026-03-23, 1d
    K review-verdict-autorecord :done, k, 2026-03-25, 1d
    L review-infra-quality     :done, l, 2026-03-25, 1d
    M autorecord-stabilization :m, after l, 2d
    N tamper-proof-review      :n, after m, 3d
    E activate-module-split    :e, after n, 1d
    D pr-usecase-extract       :d, after e, 2d
    F parser-consolidation     :f, after d, 1d
    G structural-lockdown      :g, after f, 1d

    section Phase 2 Spec Quality
    Signals and Traceability   :p2, after g, 3d

    section Phase 3 Test Generation (Moat)
    BRIDGE-01 and Templates    :p3a, after p2, 3d
    Coverage and Signals       :p3b, after p3a, 2d

    section Phase 4 Infrastructure
    Security and Worktree      :p4, after p3b, 3d

    section Phase 5 Workflow
    UX and Observability       :p5, after p4, 3d

    section Milestones
    Phase 1.5 done             :milestone, after g, 0d
    Phase 3 done (Moat)        :milestone, after p3b, 0d
    All Phases done            :milestone, after p5, 0d
```

---

## トラック完了ログ

| 日付 | トラック | PR | Phase | 備考 |
|---|---|---|---|---|
| 03-19 | `phase1-safety-hardening` | #39 | 1 | GAP-05, GAP-06 |
| 03-19 | `review-escalation-threshold` | — | WF-36 | 10 tasks |
| 03-19 | `remove-file-lock-system` (A0) | #41 | 1.5 | ~2,100行削減 |
| 03-19 | `domain-type-hardening` (A) | #42 | 1.5 | DM-01/02/03, GAP-01 |
| 03-20 | `nutype-migration` | #43 | — | MEMO-04 相当 |
| 03-20 | `pr-task-completion-guard` | #44 | — | PR push guard |
| 03-20 | `done-hash-backfill` | #45 | — | WF-40, domain cleanups |
| 03-20 | `ci-guardrails-phase15` (B) | #46 | 1.5 | STRAT-04/06, WF-54, WF-55-Ph1 |
| 03-22 | `review-usecase-extraction` (C) | #47 | 1.5 | CLI-02, domain/usecase/infra module split |
| 03-22 | `cli-review-module-split` (C') | #49 | 1.5 | CLI directory split, hexagonal port placement, architecture rules |
| 03-22 | `usecase-purity-ci` (H) | #50 | 1.5 | syn AST lint, std I/O 網羅ブロック, Codex effort=high |
| 03-22 | `pr-review-hexagonal` (H') | #51 | 1.5 | resolve_reviewer_provider I/O 除去, warning ゼロ |
| 03-23 | `usecase-purity-error` (H'') | #52 | 1.5 | warning → error 昇格, CI ブロック化 |
| 03-23 | `domain-purity-ci` (I) | #53 | 1.5 | domain 層 I/O purity CI, 共通 check_layer_purity エンジン |
| 03-23 | `conch-parser-infra-move` (J) | #54 | 1.5 | conch-parser を domain → infrastructure に移動, ShellParser port |
| 03-23 | `signal-evaluation` (2-1) | #55 | 2 | Stage 1 spec 信号機, ConfidenceSignal/SignalBasis/SignalCounts |
| 03-23 | `adr-introduction` | #56 | — | knowledge/adr/ 新設, 17 ADR, DESIGN.md 分解 |
| 03-23 | `spec-json-ssot` (2-1b) | #57 | 2 | spec.json SSoT 化, spec.md rendered view 降格, verifier 移行 |
| 03-23 | `domain-state-signals` (2-2) | #58 | 2 | Stage 2 per-state signal, syn AST 2-pass, transitions_to 検証 |
| 03-24 | `review-escalation-enforcement` | #59 | WF-36 | planning-only allowlist + review guard 強化 |
| 03-24 | `req-task-traceability` (2-3) | #60 | 2 | CC-SDD-01 要件-タスク双方向トレーサビリティ |
| 03-24 | `knowledge-strategy-move` | #61 | — | knowledge/strategy/ 再編成 |
| 03-24 | `spec-approval-gate` (2-4) | #62 | 2 | CC-SDD-02 明示的承認ゲート |
| 03-25 | `review-verdict-autorecord` (K) | #63 | 1.5 | RVW-10/11 verdict auto-record + diff scope filtering |
| 03-25 | `review-infra-quality` (L) | #64 | 1.5 | RVW-13/15/17 GitDiffScope テスト, agent 検証, auto-record e2e |
| 03-27 | `activate-vcsfix-plan-infra` | #66 | — | gitignore 修正 + track-local-plan planner infra |

**実績ベロシティ**: 9 日間で 26 トラック (2.9/日)

---

## バーンダウン

| 時点 | 残項目 | 完了トラック (累計) | 備考 |
|---|---|---|---|
| 開始 (03-19) | 40 | 0 | Phase 1.5 着手 |
| 03-22 朝 | 32 | 3 (A0, A, B) | + 5 トラック (Phase 1.5 外) |
| 03-22 夜 | 30 | 5 (A0, A, B, C, C') | + hexagonal convention, INF-15 追加 |
| 03-23 朝 | 27 | 8 (+ H, H', H'') | INF-15/16/17 完了。usecase purity CI ブロック化 |
| 03-23 夜 | 25 | 10 (+ I, J) | INF-19/20 完了。domain purity CI + conch-parser 移動 |
| **03-24 朝** | **21** | **14 (+ 2-1, ADR, 2-1b, 2-2)** | Phase 1.5 good enough 宣言。Phase 2: Stage 1+2 + spec.json SSoT 完了 |
| 03-24 夜 | 17 | 18 (+ escalation-enforcement, CC-SDD-01, strategy-move, CC-SDD-02) | Phase 2: 2-3, 2-4 完了。残り TSUMIKI-03 のみ |
| 03-25 夜 | 15 | 20 (+ RVW-10/11, RVW-13/15/17) | Phase 1.5 review infra 完了 |
| **03-27 夜** | **14** | **21 (+ activate-vcsfix-plan-infra)** | planner infra + gitignore bugfix。WF-59/WF-43 planned |
| Phase 2 完了 | 13 | 22 | |
| Phase 3 完了 | 1 | 34 | テスト生成パイプライン完成 |
| Phase 4 完了 | — | — | |
| Phase 5 完了 | 0 | — | |

---

## 凡例

- — 未着手
- ▶ 進行中
- ✅ 完了
- ✗ スコープ除外
- ⛔ ブロック中
