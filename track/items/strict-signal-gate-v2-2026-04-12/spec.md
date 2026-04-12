<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: approved
approved_at: "2026-04-12T02:24:53Z"
version: "1.0.0"
signals: { blue: 51, yellow: 0, red: 0 }
---

# Strict Spec Signal Gate v2 — Yellow blocks merge (fail-closed)

## Goal

CI では interim mode で Yellow を許容 (warning として可視化) し開発者の iteration を保護しつつ、merge gate では strict mode で Yellow をブロックして Blue 昇格を強制する二層モード設計を導入する。
シグナル評価ルールを domain 層に、オーケストレーションを usecase 層に、I/O adapter を infrastructure 層に、薄い composition を CLI 層に配置し、ヘキサゴナル原則に沿った層分離を完成させる。
既存 check_tasks_resolved も同じ TrackBlobReader port を共有する usecase 配置に consolidate し、CLI 層から git 直呼び出しを完全に排除する。
既存 reject_symlinks_below を再利用して CI 経路で symlink を拒絶し、merge gate 経路では git ls-tree mode 検査で symlink/submodule を fail-closed でブロックする。
ADR 先行 + planner cross-validation + fail-closed 真理値表を事前確定した上で実装することにより、前回試行 (PR #92) の 10+ レビューラウンドを 1-2 回に抑える。

## Scope

### In Scope
- SignalBasis::Feedback を Blue から Yellow に降格し、永続的なファイル参照を持たないソースを strict gate で Blue 扱いしないようにする [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D1] [tasks: T001]
- check_spec_doc_signals / check_domain_types_signals を domain 層の純粋関数として追加し、strict/non-strict モードで Yellow の扱いを切り替える (strict=error, non-strict=warning) [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D2, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D5.1, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D8.6] [tasks: T002, T003]
- validate_branch_ref + RefValidationError を domain 層に新設し、危険文字 (.., @{, ~, ^, :, 空白, 制御文字) を pure function で拒否する [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D2.0, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D4.2] [tasks: T004]
- verify_from_spec_json を domain 純粋関数への thin wrapper にリファクタし、Stage 2 NotFound を BLOCKED から skip に変更する (TDDD opt-in model) [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D2.1] [tasks: T005]
- CI 経路 (verify_from_spec_json) の std::fs::read_to_string 直前に reject_symlinks_below を呼び、ローカル fs の symlink/parent symlink を拒絶する [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D4.3] [tasks: T005]
- libs/usecase/src/merge_gate/ 新設: TrackBlobReader port + BlobFetchResult + check_strict_merge_gate orchestration (strict=true 固定, domain 依存のみ) [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D5.2] [tasks: T006]
- libs/usecase/src/task_completion.rs 新設: check_tasks_resolved_from_git_ref を同じ TrackBlobReader port で実装し、既存 check_tasks_resolved のロジックを usecase に consolidate する [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D9] [tasks: T007]
- libs/infrastructure/src/git_cli/show.rs 新設: BlobResult + TreeEntryKind + git_show_blob + git_ls_tree_entry_kind + fetch_blob_safe + is_path_not_found_stderr を pub(crate) で実装し、LANG=C を Command の env に固定する [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D4.1, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D5.3] [tasks: T008]
- fetch_blob_safe は git_ls_tree_entry_kind で tree entry mode 検査 (120000 symlink / 160000 submodule / 100644 regular) を行い、symlink/submodule を fail-closed で拒絶する [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D4.3] [tasks: T008]
- libs/infrastructure/src/verify/merge_gate_adapter.rs 新設: GitShowTrackBlobReader が TrackBlobReader port を実装 (fetch_blob_safe → BlobFetchResult<T> 変換 + UTF-8 decode + JSON decode) [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D5.3] [tasks: T009]
- apps/cli/src/commands/pr.rs::wait_and_merge + check_tasks_resolved を GitShowTrackBlobReader を注入する薄い composition wrapper に書き換え、CLI 層から std::process::Command::new("git") を排除する [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D5.4, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D9.3] [tasks: T010]
- knowledge/conventions/source-attribution.md に Signal 列 (Blue/Yellow) を追加し、feedback を Yellow に再分類、upgrade ガイダンス (ADR/convention への昇格) を追記する [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D1] [tasks: T011]
- Makefile.toml に verify-spec-states-current-local タスク新設 (branch-bound な track 解決, interim mode で --strict なし, plan/main/その他は skip) + ci-local / ci-container の dependencies に追加 [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D8.1, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D8.2, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D8.4] [tasks: T012]
- CI 統合回帰テスト I1–I11 を手動実行し、verification.md に記録する (cargo make ci が yellow では PASS + warning 可視化 / red では BLOCKED / 他ブランチでは skip 動作を確認) [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §Test Matrix] [tasks: T013]
- 実装完了後に ADR knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md を Proposed から Accepted に更新する [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md] [tasks: T014]

### Out of Scope
- wait-and-merge の race condition (ポーリング前 1 回のみ検査、merge 直前に再検証なし) — gh pr merge --match-head-commit 導入と全体再設計が必要。別 track で対応し、実装時に knowledge/strategy/TODO.md に新規 SEC エントリ (次の空き番号) を追加する [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D7]
- キャッシュされた signals と merge gate 実行時の fresh 評価の比較 — content_hash で改ざん検出済みのため out of scope [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D7]
- cargo make ci-rust (高速内側ループ) への strict gate 組み込み — iteration 速度優先のため現時点では対応しない [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D8.1]
- プロジェクト全体での TDDD 強制化 (domain-types.json 必須) — opt-in model を採用するため対応しない [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §Rejected Alternatives I]
- 本 track 自身の domain-types.json 作成 — SignalBasis の値再マッピングは signature 変更ではないため domain-types.json を作らず Stage 2 skip で通過させる [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §Consequences 本 track 自身の TDDD 位置付け]

## Constraints
- check_strict_merge_gate は strict: bool 引数を持たず、関数名と内部実装で strict=true を固定する (primitive obsession 回避)。shared helpers / verify_from_spec_json は両モード対応のため strict: bool を保持する [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D5.2, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §Rejected Alternatives E]
- git_cli::show プリミティブは全て pub(crate) とし、usecase 層からは見えないようにする。外部露出は GitShowTrackBlobReader adapter のみ [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D5.3]
- fetch_blob_safe は必ず git_ls_tree_entry_kind → git_show_blob の 2 段呼び出しとし、symlink/submodule/その他の tree entry mode を regular file より前に検査する [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D4.3, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D5.3]
- git サブプロセスは LANG=C LC_ALL=C LANGUAGE=C を Command の env に常に設定する (親 process env を継承しない)。stderr 英語固定により path-not-found パターンマッチを安定させる [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D4.1]
- CI interim mode の verify-spec-states-current-local は --strict フラグを付けない。strict 検査は merge gate (sotp pr wait-and-merge) のみで行う [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D8.0, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D8.2]
- Red (source 欠落) / None (未評価) / all-zero / empty entries / incomplete coverage は strict モードに関わらず常に Finding::error を返す。Yellow のみが strict=true で error / strict=false で warning に切り替わる [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D2, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D8.6]
- Finding::warning / Severity::Warning / print_outcome の既存 API を使用し、新規 API 拡張を行わない [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D8.6]
- CI 経路では既存 libs/infrastructure/src/track/symlink_guard.rs::reject_symlinks_below を use するだけで symlink 拒絶を実装する。新規 symlink 判定コードを書かない (既存コード再利用原則) [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D4.3]
- 本 track 内で cargo make ci / cargo make deny / cargo make check-layers が通ること。層依存ルール (domain ← usecase ← infrastructure ← cli) が守られること [source: convention — .claude/rules/07-dev-environment.md, convention — track/tech-stack.md §依存ルール]
- ユニットテストは domain (pure fn) / usecase (mock reader) / infrastructure (git init fixture) / cli (thin wrapper) の 4 レベルで分離する。usecase テストは git init 不要、domain テストは I/O 不要とする [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §Test Matrix]
- 前回試行ブランチ track/strict-signal-gate-2026-04-12 (PR #92 close 済み) は参照用に残す。当該ブランチ内にのみ存在する古い ADR ファイル 2026-04-12-2304-strict-spec-signal-gate.md は main にマージしない [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §Context 前回の試行]

## Acceptance Criteria
- [ ] SignalBasis::Feedback.signal() が ConfidenceSignal::Yellow を返すこと [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D1] [tasks: T001]
- [ ] domain::check_spec_doc_signals(&doc, strict=false) が signals.yellow>0 に対して Finding::warning を emit し、VerifyOutcome::is_ok() が true を返すこと [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D2, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D8.6] [tasks: T002]
- [ ] domain::check_spec_doc_signals(&doc, strict=true) が signals.yellow>0 に対して Finding::error を emit し、VerifyOutcome::has_errors() が true を返すこと [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D2] [tasks: T002]
- [ ] domain::check_spec_doc_signals が signals.red>0 / signals=None / signals all-zero に対して strict パラメータに関わらず Finding::error を emit すること [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D2] [tasks: T002]
- [ ] domain::check_domain_types_signals が entries=[] / signals=None / coverage 不完全 / Red signal に対して strict に関わらず Finding::error を emit し、declared Yellow に対して strict=false で warning + 型名リスト を emit すること [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D2, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D8.6] [tasks: T003]
- [ ] domain::validate_branch_ref が "..", "@{", "~", "^", ":", 空白, 制御文字, 空文字列を Err(RefValidationError) で拒否し、"track/strict-signal-gate-v2-2026-04-12" を Ok(()) で受け入れること [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D4.2] [tasks: T004]
- [ ] infrastructure::verify::spec_states::verify_from_spec_json が domain-types.json 不在に対して VerifyOutcome::pass() (Stage 2 skip) を返すこと [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D2.1] [tasks: T005]
- [ ] verify_from_spec_json が spec.json が symlink の場合に reject_symlinks_below 経由で BLOCKED を返すこと (CI 経路) [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D4.3] [tasks: T005]
- [ ] usecase::merge_gate::check_strict_merge_gate が MockTrackBlobReader を使った 18 ケース (U1–U18) 全てで期待結果を返すこと [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §Test Matrix U] [tasks: T006]
- [ ] usecase::merge_gate::check_strict_merge_gate が plan/ プレフィックスのブランチに対して VerifyOutcome::pass() を返すこと (D6 スキップ) [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D6] [tasks: T006]
- [ ] usecase::task_completion::check_tasks_resolved_from_git_ref が K1–K7 全ケース (未解決/NotFound/FetchError/危険文字 branch 含む) で期待結果を返すこと [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D9, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §Test Matrix U2] [tasks: T007]
- [ ] infrastructure::git_cli::show::fetch_blob_safe が symlink (mode 120000) と submodule (mode 160000) commit に対して BlobResult::CommandFailed を返し、regular file (mode 100644/100755) に対して git_show_blob 結果を返すこと [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D4.3, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D5.3] [tasks: T008]
- [ ] git_show_blob / git_ls_tree_entry_kind が LANG=ja_JP.UTF-8 環境でも LANG=C 強制により英語 stderr を返し、is_path_not_found_stderr の substring マッチが安定動作すること [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D4.1] [tasks: T008]
- [ ] GitShowTrackBlobReader が実 git repo fixture で A1–A16 全ケース (symlink / submodule commit 含む) で期待結果を返すこと [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §Test Matrix A] [tasks: T009]
- [ ] apps/cli/src/commands/pr.rs から std::process::Command::new("git") の直呼び出しが完全に消え、wait_and_merge と check_tasks_resolved が usecase 関数への thin wrapper として実装されていること [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D5.4, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D9.3] [tasks: T010]
- [ ] knowledge/conventions/source-attribution.md に Signal 列が追加され、feedback が Yellow 分類、upgrade ガイダンス (ADR/convention 作成) が明記されていること [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D1] [tasks: T011]
- [ ] cargo make ci が本 track ブランチで PASS し、新設 verify-spec-states-current-local タスクが ci-local / ci-container の依存に含まれていること [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D8.4] [tasks: T012]
- [ ] CI 統合回帰テスト I1–I11 が全て期待通り動作し、verification.md に結果が記録されていること (yellow では PASS + [warning] 行出力 / red では BLOCKED / main/plan/* では skip ログ / merge gate strict 側では yellow を BLOCKED) [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §Test Matrix I] [tasks: T013]
- [ ] knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md の Status が Accepted に更新されていること [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md] [tasks: T014]
- [ ] cargo make ci / cargo make deny / cargo make check-layers が全て通ること [source: convention — .claude/rules/07-dev-environment.md] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012, T013, T014]

## Related Conventions (Required Reading)
- knowledge/conventions/source-attribution.md
- knowledge/conventions/hexagonal-architecture.md

## Signal Summary

### Stage 1: Spec Signals
🔵 51  🟡 0  🔴 0

