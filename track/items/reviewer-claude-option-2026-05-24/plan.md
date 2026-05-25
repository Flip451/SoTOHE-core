<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# reviewer capability の provider を選択可能にする (Codex デフォルト、Claude オプション)

## Tasks (6/6 resolved)

### S1 — Infrastructure: ClaudeReviewer adapter (Reviewer port implementation)

> Add ClaudeReviewer struct to libs/infrastructure/src/review_v2/claude_reviewer.rs, implementing the Reviewer usecase port (review / fast_review) via a claude -p subprocess.
> This is the Phase-2 type-signal Blue transition task: ClaudeReviewer action=add turns Blue when this task is committed.
> Follows the CodexReviewer pattern: subprocess management, stdout envelope parsing (structured_output field), parse_review_final_message codec reuse, timeout polling, fail-closed design (CN-01 / CN-05 / CN-06). stderr is captured in-memory (no session log file, consistent with CN-05's read-only intent). The invocation uses best-effort permission-based read-only for the reviewer role: --permission-mode dontAsk auto-denies non-whitelisted tool calls, --disallowedTools Edit Write removes write tools from context, --allowedTools pre-approves the read-only inspection set, and --bare skips auto-discovery of hooks/skills/MCP/CLAUDE.md. In standard environments (no permissive host permissions.allow) write/edit tools are denied. This is NOT a kernel sandbox (claude -p has no sandbox flag) and NOT symmetric with CodexReviewer's --sandbox read-only; host settings could in principle broaden the permission surface (CN-05).

- [x] **T001**: ClaudeReviewer infrastructure adapter を新設する (IN-01 / CN-01 / CN-05 / CN-06 / AC-02 / AC-09)。

実装先: libs/infrastructure/src/review_v2/claude_reviewer.rs。

(1) `ClaudeReviewer { model: String, timeout: Duration, base_prompt: String, scope_label: String }` struct を定義する (infrastructure-types.json で指定された plain_struct に対応)。コンストラクタは名前付きジェネリクスを使う: `new<M: Into<String>, B: Into<String>>(model: M, timeout: Duration, base_prompt: B) -> Self` および `with_scope_label<S: Into<String>>(self, label: S) -> Self`。型シグネチャのコーデックが同名の `impl Into<String>` 引数を区別できないため、型変数は別名 (M / B / S) で定義する。

(2) `build_full_prompt` ヘルパを `CodexReviewer` と同一パターンで実装する (base_prompt + scope file list の結合)。

(3) `run_review` 内部メソッドを実装する: 以下のコマンドを subprocess として起動する (CN-01)。

```
claude -p --bare --permission-mode dontAsk \
  --allowedTools Read Grep Glob "Bash(git diff:*)" "Bash(git show:*)" "Bash(git log:*)" "Bash(git ls-files:*)" \
  --disallowedTools Edit Write \
  --output-format json --json-schema '<REVIEW_OUTPUT_SCHEMA_JSON>' --model <model> <prompt>
```

reviewer role に対してベストエフォートの permission-based read-only を提供する: `--permission-mode dontAsk` (ホワイトリスト外のツールを自動拒否)、`--disallowedTools Edit Write` (書き込みツールをコンテキストから除去)、`--allowedTools` (読み取り専用の検査ツールセットを事前承認)、`--bare` (hooks/skills/MCP/CLAUDE.md の自動検出をスキップ)。標準的な環境 (ホストの permissions.allow が許容的でない場合) では書き込み・編集ツールは拒否される。これはカーネルサンドボックスではなく (claude -p にサンドボックスフラグはない)、`codex exec --sandbox read-only` と同等ではない; ホスト設定が原則として permission surface を拡大しうる点に注意 (CN-05)。`--allowedTools` のトークンは個別に渡す (スペース区切り、引用符なし ― ただし `Bash(...)` 形式は引用符付きで渡す)。stdout を読んで JSON エンベロープを serde_json で parse し、`structured_output` フィールドを取り出す。stderr は in-memory で収集する (ファイル書き込みなし)。タイムアウトはポーリングループで管理する。

(4) `Reviewer` port の両メソッド (`review` / `fast_review`) を実装する (CN-06)。内部で `run_review` を呼び、`structured_output` 文字列を `parse_review_final_message` に渡し、`convert_raw_to_final` / `convert_raw_to_fast` で domain 型に変換する。`ReviewVerdict` の分類には `classify_review_verdict(timed_out, exit_success, &final_message_state)` を使用する。

(5) review_v2/mod.rs に `pub use claude_reviewer::ClaudeReviewer;` を追加する。

(6) unit tests: プロンプト構築 (空 target でも base_prompt を返す)、fake claude スクリプトを使った正常系テスト (`zero_findings` JSON を stdout エンベロープで返す偽バイナリ)。フェイクバイナリは #[cfg(unix)] 前提で可。test-only bin override は `SOTP_CLAUDE_BIN` 環境変数 + cfg(test) フィールドで `CodexReviewer` と対称なパターンを踏む。

### S2 — Infrastructure + CLI: sotp review claude-local subcommand

> Add the run_claude_review_str composition function in infrastructure/cli_composition.rs and the claude-local CLI subcommand in apps/cli/src/commands/review/claude_local.rs.
> Mirrors codex-local: same auto-record arguments (--track-id / --group / --round-type), same write-first / fail-closed verdict recording via FsReviewStore (CN-02 / AC-03).
> Exposed as sotp review claude-local; remains a low-level ad-hoc override target (the skill will call sotp review local instead).

- [x] **T002**: sotp review claude-local サブコマンドと infrastructure 組み立て関数を新設する (IN-02 / CN-02 / AC-03 / AC-09)。

(1) infrastructure 層: libs/infrastructure/src/review_v2/cli_composition.rs に `ReviewV2CompositionWithClaude` struct および `build_review_v2_with_claude_reviewer` / `build_review_v2_with_claude_reviewer_str` を追加する。シグネチャは `CodexReviewer` 版と対称にする (型パラメータが `ClaudeReviewer` に変わるだけ)。`run_claude_review_str(track_id_str, items_dir, group_str, round_type_str, reviewer: ClaudeReviewer) -> Result<CodexReviewOutcome, String>` を追加する。実装は `run_codex_review_str` を `ClaudeReviewer` に差し替えた同一パターンで動作する。

(2) CLI 層: apps/cli/src/commands/review/claude_local.rs を新設する。`ClaudeLocalArgs` struct (CodexLocalArgs と同一フィールド構造、ただし `model` の doc comment を調整)。`execute_claude_local(args: &ClaudeLocalArgs) -> ExitCode` を `execute_codex_local` と対称に実装する: Step 1 validate_auto_record_args 呼び出し (validate ロジックは共通化)、Step 2 briefing 確認、Step 3 base_prompt 組み立て、Step 4 `ClaudeReviewer::new` でコンポーズ、Step 5 `infrastructure::review_v2::run_claude_review_str` 呼び出し、Step 6 outcome match。fail-closed 契約: write 失敗 → エラー返却 → verdict 未表示 (CN-02 / AC-03)。

(3) apps/cli/src/commands/review/mod.rs を更新する: `ReviewCommand` enum に `ClaudeLocal(ClaudeLocalArgs)` を追加し、`execute` の match アームに `ReviewCommand::ClaudeLocal(args) => execute_claude_local(&args)` を追加する。`mod claude_local;` と `use claude_local::execute_claude_local;` を追加する。

(4) validate_auto_record_args が CodexLocalArgs 以外にも使えるよう引数型を trait object か macro か、あるいは ClaudeLocalArgs でも同一ロジックを呼べる構造にする (コードが 100 行超の重複にならないよう注意)。

### S3 — CLI: sotp review local unified entry point (provider auto-resolution)

> Add the Local subcommand to apps/cli/src/commands/review/local.rs.
> Loads agent-profiles.json, calls resolve_execution("reviewer", round_type) to obtain (provider, model), and dispatches internally to CodexReviewer or ClaudeReviewer (CN-03 / CN-04).
> The skill passes only --round-type / --group / --track-id / --briefing-file; no --model / --provider in the skill document (AC-01 / AC-04 / AC-07).
> Mixed-provider ladder (fast_provider fallback) is handled automatically by resolve_execution (AC-04).

- [x] **T003**: sotp review local 統合エントリポイントを新設する (IN-03 / CN-03 / CN-04 / AC-01 / AC-04 / AC-09)。

(1) apps/cli/src/commands/review/mod.rs: `ReviewCommand` enum に `Local(LocalArgs)` variant を追加する。

(2) apps/cli/src/commands/review/local.rs を新設する。`LocalArgs` struct: `--briefing-file`, `--prompt` (mutually exclusive with briefing-file), `--track-id`, `--round-type`, `--group`, `--items-dir` を持つ。`--model` はオプション上書き引数として残す (ad-hoc 用、通常 None)。`--timeout-seconds` も引き継ぐ。

(3) `execute_local(args: &LocalArgs) -> ExitCode` を実装する:
  - Step 1: `AgentProfiles::load(AGENT_PROFILES_PATH)` でプロファイル読み込み。
  - Step 2: `profiles.resolve_execution("reviewer", round_type)` で `Option<ResolvedExecution>` を取得。None なら fail-closed エラー終了 (CN-03)。
  - Step 3: `args.model` が Some なら resolved.model を上書き。
  - Step 4: resolved.provider に応じて内部 dispatch:
    - `"codex"` → `CodexLocalArgs` を組み立てて `execute_codex_local` と同等のロジックを呼ぶ (または `run_codex_review_str` を直接呼ぶ)。
    - `"claude"` → `ClaudeLocalArgs` を組み立てて `execute_claude_local` と同等のロジックを呼ぶ (または `run_claude_review_str` を直接呼ぶ)。
    - その他 → fail-closed エラー。
  - 解決した provider/model は eprintln でログ出力する (AC-01 / AC-04 デバッグ可視性)。

(4) mixed-provider ladder の動作: `--round-type fast` のとき `fast_provider` / `fast_model` フォールバックが `resolve_execution` 内部で処理されるため、`Local` は round_type を素直に渡すだけで AC-04 が自動充足される。

(5) `ReviewCommand::Local(args) => execute_local(&args)` を execute match に追加する。

(6) apps/cli/src/commands/make.rs の `dispatch_track_local_review` 関数を更新する: `vec!["review", "codex-local"]` を `vec!["review", "local"]` に変更し、`cargo make track-local-review` が `sotp review local` (自動解決・dispatch) を呼ぶ構造にする。これにより T005 で更新する review.md Step 4/5 の reviewer invocation (`cargo make track-local-review -- --round-type ... --group ... --track-id ... --briefing-file ...`) が正しく `sotp review local` に到達する (AC-01 / AC-07)。

### S4 — Config + CLI: pr-reviewer capability and pr.rs re-point

> Add pr-reviewer capability to .harness/config/agent-profiles.json (default provider=codex).
> Update pr.rs trigger_review / review_cycle to resolve_execution("pr-reviewer", ...) instead of ("reviewer", ...), and clarify validate_reviewer_provider semantics to PR-only scope.
> Ensures reviewer.provider=claude does not break /track:pr-review (D5 / AC-05 / AC-06).

- [x] **T004**: pr-reviewer capability を agent-profiles.json に新設し、pr.rs を差し替える (IN-04 / AC-05 / AC-06 / AC-09)。

(1) .harness/config/agent-profiles.json に `"pr-reviewer"` capability を追加する:
```json
"pr-reviewer": {
  "provider": "codex"
}
```
デフォルト provider=codex。既存 `reviewer` エントリはそのまま維持する (Codex デフォルト保持、CN-04)。

(2) apps/cli/src/commands/pr.rs を更新する:
  - `trigger_review` 関数の `profiles.resolve_execution("reviewer", RoundType::Final)` を `profiles.resolve_execution("pr-reviewer", RoundType::Final)` に差し替える。
  - エラーメッセージも `"pr-reviewer capability not defined in agent-profiles.json"` に更新する。
  - `review_cycle` 関数の同じ箇所を同様に差し替える。
  - `validate_reviewer_provider` の doc comment を「PR ベースレビューが Codex Cloud 互換 provider を使っているか検証する。ローカルレビュー provider (reviewer.provider) は検証しない」と明記する。

(3) pr-reviewer が未定義の場合 (resolve_execution が None を返す場合) は既存の fail-closed エラーパターンで `"pr-reviewer capability not defined"` を返す (AC-06)。

(4) verify-orchestra など検証系への影響がないことを確認する (agent-profiles.json のスキーマが `#[serde(deny_unknown_fields)]` を使っている場合は `pr-reviewer` キーが `CapabilityConfigDto` の既存フィールドと互換かを確認)。

### S5 — Docs: review.md update (reviewer provider/model literal removal)

> Update .claude/commands/track/review.md to remove manual reviewer provider/model resolution logic (Step 1) and update reviewer invocation strings to use sotp review local (no --model argument).
> After this change, review.md contains no reviewer provider names or reviewer model name literals in the reviewer-invocation paths (AC-07). The review-fix-lead dispatch condition labels (provider: codex / provider: claude) and review-fix-lead.model references remain in Step 4/5 — these are the fix-lead dispatch mechanism, not reviewer invocation literals, and are explicitly out of AC-07's scope.

- [x] **T005**: .claude/commands/track/review.md を更新する (IN-05 / AC-07)。

(1) Step 1 の reviewer 解決ロジックを完全に削除する: `capabilities.reviewer.provider`, `capabilities.reviewer.fast_model`, `capabilities.reviewer.model` を手動読み取りして `--model` 引数に渡す記述をすべて削除する。

(2) Step 1 に代わる説明を追記する: `sotp review local` コマンドが `agent-profiles.json` の `reviewer` capability を自動読み取りして provider/model を解決するため、skill が provider 名やモデル名のリテラルを直接渡す必要がないことを記述する。`capabilities.review-fix-lead` の解決 (fix-lead provider/model) は引き続き Step 1 で説明する。

(3) Step 3 の制約節「The CLI auto-injects scope file list...」の `sotp review codex-local` への言及を `sotp review local` に修正する。

(4) Step 4 / Step 5 の reviewer invocation (fixer が reviewer を呼ぶコマンド文字列) を `cargo make track-local-review -- --round-type {round_type} --group {scope} --track-id {track-id} --briefing-file tmp/reviewer-runtime/briefing-{scope}.md` 形式に更新する (`--model {fast_model}` を削除し、`track-local-review` が内部で `sotp review local` を呼ぶ構造)。

(5) "Codex CLI is the only supported provider; `claude` is unsupported" 相当の記述を削除する (AC-07)。

(6) reviewer invocation 箇所 (Step 3 制約節・Step 4/5 レビュー呼び出し文字列) に reviewer provider 名・reviewer モデル名のリテラル (`"codex"`, `"claude"`, `"gpt-*"` 等) が現れないことを確認する (AC-07)。review-fix-lead の dispatch 条件ラベル (`provider: codex` / `provider: claude`) と `review-fix-lead.model` の参照は reviewer invocation の対象外であり、Step 4/5 に残ることが正しい。

### S6 — Docs: pr-review.md + 10-guardrails.md update

> Update .claude/commands/track/pr-review.md to reference pr-reviewer capability instead of reviewer.
> Update .claude/rules/10-guardrails.md Reviewer Capability Constraint section: remove claude-heavy / Explore subagent substitute, add official Claude reviewer path via D3 auto-dispatch (AC-08).

- [x] **T006**: .claude/commands/track/pr-review.md と .claude/rules/10-guardrails.md を更新する (IN-06 / AC-08)。

(1) .claude/commands/track/pr-review.md:
  - `reviewer` capability への言及を `pr-reviewer` capability に変更する (D5 / AC-08)。
  - 「structured provider set は codex のみ」という制約は維持する (pr-reviewer の Codex Cloud 専用制約)。
  - `reviewer.provider=claude` の設定が `/track:pr-review` に影響しないことを注記する (D5 による構造的担保)。

(2) .claude/rules/10-guardrails.md の "Reviewer Capability Constraint" 節を更新する:
  - `claude-heavy` プロファイル / `subagent_type: "Explore"` による reviewer 代替の言及を削除する (AC-08)。
  - `provider: claude` の場合の公式経路は `sotp review local` (D3 自動解決・dispatch) が `ClaudeReviewer` を起動する経路であると明記する。
  - Explore subagent による self-review はどの profile でも `zero_findings` の代替として認められないことを維持する。
  - provider が未定義の場合の fail-closed 動作を残す (retry 指示)。
  - `claude` provider の場合にも「外部 provider の実行失敗なら retry (最大 2 回)、失敗継続ならユーザーへ報告」という原則を適用することを明記する (外部 subprocess 経路は hook 適用外だが、claudeReviewer の stdout 解析失敗は同様に retry 対象)。
