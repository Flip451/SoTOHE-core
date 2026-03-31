# Review Remediation Plan (v2)

> **作成日**: 2026-03-31
> **監査レポート**: [`knowledge/research/2026-03-31-2112-review-process-audit.md`](../research/2026-03-31-2112-review-process-audit.md)
> **対象**: review プロセスに関連する全 open issues（RVW, WF, CLI, ERR, SEC, MEMO, CLAUDE-BP, 無 ID 含む）
> **総数**: 58 件（CRITICAL 1, HIGH 16, MEDIUM 20, LOW 21）

---

## 設計方針

1. **小さいトラックに分割**（<500 行目安）
2. **テーマでまとめる**（同一関心事は同一トラック）
3. **即効性優先**（実害があるものから）
4. **legacy 除去はまとめて後回し**

---

## 全体マップ

```
Phase A (hotfix)           ─── 即時、トラック不要
Phase B (data fidelity)    ─── findings データ忠実性
Phase C (failure recovery) ─── reviewer 障害耐性
Phase D (PR review flow)   ─── push/commit ガード修正 + PR review 中断耐性
Phase E (infra cleanup)    ─── コード品質・分割
Phase F (escalation)       ─── エスカレーション review.json 移行 + model-tier 強制
Phase G (legacy removal)   ─── ReviewState 除去で 8+ 件一括解消
Phase H (workflow guards)  ─── verdict attestation + staging 漏れ + design escalation
Phase I (DX / testing)     ─── auto-record e2e テスト + テスト強化
Phase J (defense / future) ─── ACCEPTED 仕組み化、fallback、パーサー統合等
```

---

## Phase A: 即時修正（hotfix、トラック不要）

| ID | 内容 | 変更対象 | 規模 |
|---|---|---|---|
| RVW-37 | `infra` → `infrastructure` グループ名統一 | `.claude/commands/track/review.md` | XS |

Rust コード変更なし。review.md 内の `infra` を `infrastructure` に置換するだけ。

---

## Phase B: Review データ忠実性（1 トラック、M）

**Track**: `review-finding-fidelity`
**目的**: reviewer findings を review.json まで劣化なく保持

| # | ID | 内容 | レイヤー |
|---|---|---|---|
| T001 | RVW-38 | `StoredFinding` + `FindingDocument` に `category: Option<String>` 追加 | domain + infra |
| T002 | RVW-38 | schema `category` required → nullable 統一 + shape validator 修正 | usecase |
| T003 | RVW-34 | `RecordRoundProtocol` に `findings: Vec<StoredFinding>` 追加。元データ保持 | usecase + infra |
| T004 | RVW-08 | `ScopeFilteredPayload` 削除。全 findings を record-round に渡す | usecase + CLI |
| T005 | WF-45 | `render_review_payload()` の `category: null` 明示出力（T001 で自然解消） | — |
| T006 | — | テスト: review.json round-trip で全フィールド保持を検証 | infra |

**解消**: RVW-08, RVW-34, RVW-38, WF-45（4 件）

---

## Phase C: Reviewer 障害耐性（1 トラック、M）

**Track**: `review-failure-recovery`
**目的**: Codex タイムアウト/失敗時のレビュー継続性

| # | ID | 内容 | レイヤー |
|---|---|---|---|
| T001 | RVW-42 | `Invalid` 状態でも session log fallback を試行 | CLI |
| T002 | RVW-07 | stderr からの JSON 抽出 + session log パス永続化 | CLI |
| T003 | RVW-39 | timeout/failed の informational round を review.json に記録 | domain + CLI |
| T004 | — | テスト: timeout 後の次ラウンドが stale cascade を起こさないこと | CLI |

**解消**: RVW-07, RVW-39, RVW-42（3 件）

---

## Phase D: PR Review フロー修正（1 トラック、M）

**Track**: `pr-review-flow-fix`
**目的**: push/commit ガードが PR review の利用を阻害する問題を解消 + 中断耐性

| # | ID | 内容 | レイヤー |
|---|---|---|---|
| T001 | WF-66 | `check_task_completion_guard` を `push()` と `review_cycle()` から削除し `wait_and_merge()` に移動 | CLI (pr.rs) |
| T002 | WF-66 | `track-pr-merge` のみがタスク完了を要求するよう Makefile.toml / SKILL.md を更新 | CLI + docs |
| T003 | ERR-08 | `review_cycle()` の trigger state（PR 番号, comment ID, timestamp）を `tmp/pr-review-state/<track-id>.json` に永続化 | CLI (pr.rs) |
| T004 | ERR-08 | `track-pr-review --resume` サブコマンド追加（永続化 state から poll を再開） | CLI (pr.rs) |
| T005 | — | テスト: 未完了タスクがある状態で push + PR review が通ること | CLI |

**解消**: WF-66, ERR-08（2 件）
**依存**: なし（他 Phase と独立）

---

## Phase E: インフラ品質（1 トラック、S）

**Track**: `review-infra-cleanup`
**目的**: コード品質改善 + 保守性向上

| # | ID | 内容 | レイヤー |
|---|---|---|---|
| T001 | RVW-40 | `RecordRoundProtocolImpl::execute` 分割 + policy resolution キャッシュ | infra |
| T002 | RVW-41 | `check_approved` の `_writer` パラメータ削除 | usecase + CLI |
| T003 | RVW-43 | `unwrap_or_default()` → `expect(...)` | infra |
| T004 | — | `RecordRoundArgs` の型付き引数統一（`CodexLocalArgs` と整合） | CLI |
| T005 | RVW-19 | record-round リトライジッターを `rand` に置換 | infra |

**解消**: RVW-40, RVW-41, RVW-43, RVW-19（4 件）
**依存**: Phase B 完了推奨（execute 分割時に findings パラメータ変更を含められる）

---

## Phase F: エスカレーション移行 + model-tier 強制（1 トラック、L）

**Track**: `escalation-reviewjson-migration`
**目的**: エスカレーション二重管理の解消 + sequential escalation の機構化
**既存**: `tamper-proof-review-2026-03-26` (planned) を統合

| # | ID | 内容 | レイヤー |
|---|---|---|---|
| T001 | RVW-06 | `ReviewCycle` に `escalation: EscalationState` 追加 | domain |
| T002 | RVW-06 | `review_json_codec` にエスカレーション encode/decode | infra |
| T003 | RVW-06 | `RecordRoundProtocolImpl` のエスカレーションゲートを review.json に移行 | infra + usecase |
| T004 | RVW-06 | `check_approved` のエスカレーションゲートを review.json に移行 | usecase |
| T005 | RVW-06 | `resolve-escalation` CLI を review.json 対応に更新 | CLI |
| T006 | WF-36 | `record-round` に `--model-tier fast\|full` フラグ追加。domain で 2 段階追跡 | domain + CLI |
| T007 | WF-36 | `check-approved` が full model 未確認グループを拒否 | usecase |
| T008 | — | metadata.json から escalation フィールド群を削除 | infra |
| T009 | WF-47 | `findings_to_concerns()` を record-round CLI に自動配線 | CLI |

**解消**: RVW-06, WF-36, WF-47, `Review escalation enforcement`（4 件）
**依存**: Phase E 完了推奨

---

## Phase G: Legacy ReviewState 除去（1 トラック、M）

**Track**: `legacy-review-state-removal`
**目的**: production 未使用の legacy model を除去し 8+ 件を一括解消

| # | ID | 内容 | レイヤー |
|---|---|---|---|
| T001 | — | `ReviewState`, `ReviewStatus`, `ReviewGroupState`, `check_commit_ready`, `record_round`, `record_round_with_pending`, `update_status_after_record` を domain から削除 | domain |
| T002 | — | `ReviewState` の `with_fields` / codec 関連を infra から削除 | infra |
| T003 | — | integration test を cycle model テストに置換 | tests |
| T004 | — | `CodeHash::Pending` の除去（cycle model で未使用なら） | domain |
| T005 | WF-41 | `review_from_document` の偽 Fast ラウンド合成も除去 | domain |
| T006 | WF-51 | `ReviewRoundResult::new` のバリデーション問題も除去 | domain |

**一括解消**: RVW-24, RVW-09, WF-62, WF-44, WF-48, WF-49, WF-41, WF-51, C-3, C-5（10 件）
**依存**: Phase F 完了必須（metadata.json エスカレーションが不要になってから）

---

## Phase H: ワークフローガード強化（1 トラック、M）

**Track**: `review-workflow-guards`
**目的**: verdict 信頼性 + staging 整合性 + 設計エスカレーション

| # | ID | 内容 | レイヤー |
|---|---|---|---|
| T001 | WF-61 | verdict attestation: `record-round` が session log パスを受け取り、log の verdict と照合 | CLI + usecase |
| T002 | RVW-30 | `check-approved` に worktree/index 差分検出を追加。staging 漏れ時にブロック | usecase |
| T003 | WF-65 | `/track:implement` に scope guard 定義: 新 enum variant / error type / port 追加時は設計フェーズに戻す基準を SKILL.md に記載 | docs |
| T004 | WF-60 | `/track:review` に scope guard: finding が spec out_of_scope を指摘 → planner escalation の自動起動フロー追加 | docs + SKILL.md |
| T005 | RVW-12 | 既存コード品質問題の修正: guard/policy.rs バイパス(P0), spec_frontmatter.rs trim_end(P1) | infra |

**解消**: WF-61, RVW-30, WF-65, WF-60, RVW-12（5 件）
**依存**: Phase F 完了推奨

---

## Phase I: テスト強化（1 トラック、M）

**Track**: `review-test-hardening`
**目的**: review インフラのテストカバレッジ向上

| # | ID | 内容 | レイヤー |
|---|---|---|---|
| T001 | RVW-13 | `--auto-record` e2e 実戦テスト（並列レビュー実運用確認） | e2e |
| T002 | RVW-15 | `GitDiffScopeProvider` 契約テスト（tempdir git fixture） | infra |
| T003 | RVW-16 | escalation block (exit 3) 統合テスト | CLI |
| T004 | WF-52 | CLI review コマンド統合テスト（record-round, exit-code-3, resolve-escalation） | CLI |
| T005 | RVW-14 | path normalization 改善: 絶対パス→repo-relative, renamed file DiffScope 追加 | infra |

**解消**: RVW-13, RVW-14, RVW-15, RVW-16, WF-52（5 件）
**依存**: Phase F 完了後（escalation テストに review.json ベースが必要）

---

## Phase J: 防御強化・将来対応（個別トラック化、選択的実施）

| ID | 難易度 | 内容 | 優先度 |
|---|---|---|---|
| RVW-20 | L | ACCEPTED finding 仕組み化 + dispute adjudication | HIGH |
| RVW-36 | M | Codex 上限 fallback（claude-heavy profile での reviewer 代替） | HIGH |
| RVW-25 | S | `CodeHash::Computed(String)` → `ReviewHash` newtype | HIGH |
| CLI-01 | M | `pr.rs` (1432 行) の review polling/parsing を usecase に移動 | HIGH |
| RVW-26 | S | fast model false positive 対策 | MEDIUM |
| MEMO-01 | S | 並列レビューのブリーフィングファイル競合（セッション ID 分離） | MEDIUM |
| RVW-17 | S | Agent hook empty stdin 根本対策 | MEDIUM |
| CLAUDE-BP-02 | M | Writer/Reviewer 分離 | MEDIUM |
| WF-55 (Phase 2-3) | L | metadata.json SSoT 一貫化（verification.md + spec.md の JSON 化） | HIGH |
| SEC-14 | M | shell `-c` payload の再帰パース（guard バイパス防止） | HIGH |
| RVW-35 | S | group_scope_hash volatile field 正規化 | LOW |
| RVW-27 | XS | codec ロード時 format warning | LOW |
| RVW-28 | XS | check_approved single-process 前提の明文化 | LOW |
| WF-50 | XS | file_path_to_concern 中間ディレクトリ | LOW |
| WF-53 | XS | file_path_to_concern absolute path 誤マッチ | LOW |
| WF-38 | XS | frontmatter パーサー duplicate key 未検出 | LOW |
| WF-42-residual | — | zero-findings commit scope 制約（GitHub API 制約、根本解決不可） | LOW |
| RVW-04 | S | syn による is_inside_test_module 置換 | LOW |
| RVW-05 | XS | skip_command_launchers フラグモデリング（RVW-02 で解消） | LOW |
| RVW-18 | XS | codex-reviewer tools: 制限の動作検証 | LOW |
| (無 ID) | S | track-local-review 出力改善（round 別 JSON 保存） | MEDIUM |
| (無 ID) | XS | reviewer subagent Bash timeout 10 分引き上げ | MEDIUM |
| (無 ID) | S | レビュー結果蓄積 + track-review-history | LOW |
| (無 ID) | M | /track:hotfix コマンド（軽微変更のレビュースキップ） | MEDIUM |
| (無 ID) | S | /track:review の provider 移譲強制 hook | MEDIUM |
| RVW-01 | M | 共通 Frontmatter パーサー抽出 | HIGH |
| RVW-02 | M | conch-parser AST 直接走査 | HIGH |
| RVW-03 | M | typed deserialization convention | MEDIUM |

---

## 実行順序と依存関係

```
Phase A (hotfix, 即時)
    │
    ├── Phase B (data fidelity)
    │       │
    │       ├── Phase C (failure recovery)     Phase D (PR review flow) ← 独立
    │       │       │
    │       └── Phase E (infra cleanup)
    │               │
    │          Phase F (escalation migration)
    │               │
    │          Phase G (legacy removal)
    │               │
    │          Phase H (workflow guards)
    │               │
    │          Phase I (test hardening)
    │
    └── Phase J (選択的、各項目独立)
```

Phase D（PR review フロー修正）は他と独立で、即座に着手可能。

---

## 見積もり

| Phase | 難易度 | 推定日数 | 解消 TODO 数 |
|---|---|---|---|
| A | XS | 即時 | 1 |
| B | M | 1 | 4 |
| C | M | 1 | 3 |
| D | M | 1 | 2 |
| E | S | 0.5 | 4 |
| F | L | 1.5 | 4 |
| G | M | 0.5 | 10 |
| H | M | 1 | 5 |
| I | M | 1 | 5 |
| **A-I 合計** | | **~7.5 日** | **38 件** |
| J (選択的) | 各 XS-L | 5-8 日 | 20 件 |
| **全体** | | **~13-16 日** | **58 件** |

---

## 推奨の着手順序（最初の 3 トラック）

1. **Phase A**: RVW-37 即時修正（レビューが通らなくなる CRITICAL）
2. **Phase D**: PR review フロー修正（ユーザーが直面している push ブロック問題）
3. **Phase B**: findings データ忠実性（レビューデータ品質の基盤）
