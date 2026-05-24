---
adr_id: 2026-05-23-2236-reviewer-provider-selectable-claude-option
decisions:
  - id: D1
    user_decision_ref: "chat_segment:reviewer-provider-selectable-claude-option:2026-05-23"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:reviewer-provider-selectable-claude-option:2026-05-23"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:reviewer-provider-selectable-claude-option:2026-05-23"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:reviewer-provider-selectable-claude-option:2026-05-23"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:reviewer-provider-selectable-claude-option:2026-05-23"
    status: proposed
---
# reviewer capability の provider を選択可能にする (Codex デフォルト、Claude オプション)

## Context

SoTOHE の reviewer capability は現在 Codex (`.harness/config/agent-profiles.json` の `capabilities.reviewer`) に固定されており、`/track:review` の Step 1 は「Codex CLI が唯一の対応 provider、`claude` は未対応」と明記している。

一方、レビュー用途に効く軸 (コード理解・多言語・Rust ドメイン知識) では Claude が優位なシナリオがある。テンプレートユーザーが手元の契約・好み・モデル性能の変化に応じて reviewer provider を選べるようにすることは、`review-fix-lead` を選択可能にした流れ (`knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md`) と対称であり、自然な次のステップである。

**現在の capability 構成** (`.harness/config/agent-profiles.json` 参照):

- `reviewer`: `provider: codex`, `model: gpt-5.5`, `fast_model: gpt-5.4-mini`
- `review-fix-lead`: `provider: claude` (デフォルト), `model: claude-opus-4-7` (sibling ADR 参照)

**既存の reviewer メカニズム** (`sotp review codex-local`):

`CodexReviewer` は `codex exec --model <model> --sandbox read-only --config model_reasoning_effort="high" --output-schema <schema.json> --output-last-message <msg.txt> <prompt>` を subprocess として起動し、`--output-last-message` ファイルに書き込まれた JSON verdict (`{"verdict": "zero_findings"|"findings_remain", "findings": [...]}`) を `parse_review_final_message` で解析する。この verdict codec (`usecase::review_workflow` の `REVIEW_OUTPUT_SCHEMA_JSON` + `parse_review_final_message`) はすでに provider 非依存な JSON schema として定義されており、`claude -p` が同一 JSON を出力すれば無変更で流用できる。

`Reviewer` usecase port (`usecase::review_v2::ports::Reviewer`) は `&ReviewTarget` を受け取る 2 つのメソッドを持つ provider 中立インタフェースとして設計されている: `review` は final round 用で `Result<(Verdict, LogInfo), ReviewerError>` を返し、`fast_review` は fast round 用で `Result<(FastVerdict, LogInfo), ReviewerError>` を返す。いずれの型も `domain::review_v2` に属し provider 固有の依存を持たない。`ClaudeReviewer` を並列実装として追加することが構造的に可能である。

**本 ADR の位置づけ**: `review-fix-lead-codex-migration` ADR が fixer の provider を選択可能にしたのに対し、本 ADR は reviewer の provider を選択可能にする。両 ADR を合わせることで reviewer × fixer の 2×2 (Codex/Claude 独立) の組み合わせが設定可能になる。

## Decision

### D1: Claude オプションのスコープ — ローカル reviewer (`/track:review`) のみ

`/track:review` が呼び出すローカルレビュー経路 (`sotp review codex-local` 相当) に Claude を選択肢として追加する。`/track:pr-review` が使う Codex Cloud GitHub App (`@codex review`) は本 ADR の対象外とし、引き続き Codex Cloud 専用とする。

理由:

- ローカル reviewer は `claude -p` subprocess として呼び出せる (read-only、Reviewer port を実装する構造)。`/track:pr-review` は GitHub App のトリガー機構 (`@codex review` コメント + Codex Cloud ポーリング) を前提としており、`claude -p` 呼び出しと機構が根本的に異なる。
- `reviewer.provider` (ローカルレビュー専用) と `/track:pr-review` の provider は D5 で分離されるため、`reviewer.provider = claude` を設定しても `/track:pr-review` の動作は変わらない。

Codex をデフォルトとして従来動作を維持しつつ、Claude を追加の選択肢として提供する。

### D2: Claude reviewer の実装経路 — `ClaudeReviewer` + `sotp review claude-local`

`ClaudeReviewer` 実装を新設する。`ClaudeReviewer` は `CodexReviewer` と並列に `Reviewer` usecase port の両メソッド (`review` / `fast_review`) を実装する。これにより fast round は `fast_provider ?? provider` で解決された provider の `fast_review` に、final round は `provider` の `review` にそれぞれ dispatch される (D3 の自動解決・dispatch 機構の前提)。

**呼び出し形式**:

<!-- illustrative, non-canonical -->
```
claude -p --bare --output-format json --json-schema '<REVIEW_OUTPUT_SCHEMA_JSON>' \
  --model <model> <prompt>
```

- `--output-format json`: stdout に JSON エンベロープを出力する。verdict は そのエンベロープ内の `structured_output` フィールドに格納される。recorder はエンベロープから `structured_output` を取り出して `parse_review_final_message` に渡す。
- `--json-schema '<schema>'`: API レベルの制約付きデコーディング (grammar-compiled) により、`structured_output` が指定 JSON Schema に必ず適合することをハード保証する。briefing 内のインライン指示には依存しない。
- `--bare`: hooks / MCP / CLAUDE.md の自動検出を無効化し、CI/スクリプト用途での再現性を確保する。
- 渡すスキーマは既存の `REVIEW_OUTPUT_SCHEMA_JSON` (Codex 側が `--output-schema` に渡しているものと同一)。スキーマ定義は変更しない。

`sotp review claude-local` サブコマンドを新設する。これは `sotp review codex-local` と対称な低レベルな実装ターゲットであり、D3 で新設する統合エントリポイント (`sotp review local`) がプロバイダー解決後に内部 dispatch する先のひとつである。スキルが直接呼び出すのは統合エントリポイントであり、`codex-local` / `claude-local` は ad-hoc 上書き目的のみに使う低レベルパスとして残す。どちらのサブコマンドも同一の auto-record 引数 (`--track-id`, `--group`, `--round-type` 等) と `CodexReviewOutcome` 相当の戻り値を持つ。

**auto-record と fail-closed 契約**: `claude-local` は `codex-local` と同一の auto-record 契約に従う。verdict JSON の解析後、CLI プロセス内で `FsReviewStore::write_verdict` / `write_fast_verdict` を呼び出して `track/items/<id>/review.json` に書き込んでから、verdict の表示を行う。`?` 伝播による fail-closed 設計: write 失敗 → コマンドエラー → verdict 未表示 → `check-approved` はそのスコープを未完了扱い。この記録はオーケストレーター・スキルのプロンプトによる parsing に依存せず CLI が自律的に行う。`--track-id` / `--round-type` / `--group` は必須引数であり opt-out 不可。Codex subprocess の stdout は捨てられ verdict は `--output-last-message` ファイルから読む codex-local と異なり、claude-local は Claude の stdout エンベロープの `structured_output` フィールドから読む。この違い以外の auto-record / fail-closed の動作は同一である。

verdict codec (`parse_review_final_message` / `REVIEW_OUTPUT_SCHEMA_JSON`) は provider 中立のまま変更しない。Codex 側は `--output-schema` → `--output-last-message` ファイル経由でスキーマ適合 JSON を受け取り、Claude 側は `--json-schema` → stdout エンベロープの `structured_output` フィールド経由でスキーマ適合 JSON を受け取る。受け取り経路は異なるが、どちらも同じ `parse_review_final_message` codec を通る。

`claude -p` は read-only 相当 (ファイル編集操作を行わない reviewer role) で起動する。`review-fix-lead` が fix 役、reviewer が review 役という役割分担を維持する。`ReviewFinalMessageState::Invalid` パスは引き続き防衛的フォールバックとして残るが、`--json-schema` 制約付きデコーディングによりハード保証が主要保護になる。

### D3: Mixed-provider ladder — コマンド側自動解決・dispatch (既存 schema を活用)

**既存の config schema** (`agent-profiles.json`):

`CapabilityConfigDto` はすでに `fast_provider: Option<String>` / `fast_model: Option<String>` / `provider: String` / `model: Option<String>` を持っており (`#[serde(deny_unknown_fields)]`)、mixed-provider ladder はスキーマ変更なしに既存フィールドで表現できる。fast round と final round で異なる provider / model を使う例:

<!-- illustrative, non-canonical -->
```json
"reviewer": {
  "provider": "claude",
  "model": "claude-opus-4-7",
  "fast_provider": "codex",
  "fast_model": "gpt-5.4-mini"
}
```

解決ルール — 既存の `AgentProfiles::resolve_execution("reviewer", round_type)` が `Option<ResolvedExecution>` を返す (`ResolvedExecution { provider: String, model: Option<String> }`):

- `RoundType::Fast` → `(fast_provider ?? provider, fast_model ?? model)`
- `RoundType::Final` → `(provider, model)`

`fast_provider` 未指定時は `provider` にフォールバック。`pr.rs` はすでに `resolve_execution("reviewer", RoundType::Final)` で provider を取得するパターンを採用している。

**本 ADR の貢献: コマンド側自動解決・dispatch (両 provider に共通)**

現状では呼び出し側 (`/track:review` skill) が `agent-profiles.json` を直接読んで `--model` を手動解決し、`sotp review codex-local --model <model>` のようにコマンドに渡している。本 ADR はこの解決をコマンド内部に移す。これは Claude 追加だけでなく **Codex 経路にも同様に適用される変更**である — 統合エントリポイントが導入されたことで、Codex の場合も skill は `--model` を渡さなくなる。

統合エントリポイント `sotp review local` を新設する。スキルはこの単一コマンドを呼び出す。コマンドは呼び出し時に次の手順で provider / model を決定し、内部 dispatch する:

1. `AgentProfiles::load(AGENT_PROFILES_PATH)` で設定を読み込む
2. `profiles.resolve_execution("reviewer", round_type)` で `Option<ResolvedExecution>` を取得する (`round_type` は `--round-type` 引数から `infrastructure::agent_profiles::RoundType` に変換; 未定義なら fail-closed エラー)
3. `resolved.provider` の値に応じて内部 dispatch する:
   - `"codex"` → `CodexReviewer` (`sotp review codex-local` の実装経路; `codex exec --sandbox read-only --output-schema ... --output-last-message ...`)
   - `"claude"` → `ClaudeReviewer` (`sotp review claude-local` の実装経路; `claude -p --bare --output-format json --json-schema ...`)

`sotp review codex-local` / `sotp review claude-local` は低レベルの実装ターゲットとして残るが、スキルは呼び出さない。ad-hoc 上書き用途 (`--model` 直接指定等) でのみ使用する。

呼び出し側 (`/track:review` skill) は `--round-type` / `--group` / `--track-id` / `--briefing-file` のみを渡す。`--model` / `--provider` は渡さない。`--model` はオプションの上書き引数として `sotp review local` に残し、ad-hoc 用途にのみ使う。

sequential-escalation ladder (fast tier が `zero_findings` を宣言したら final tier に escalation) の動作は変わらない。provider dispatch はコマンド 1 箇所に集約されるため、skill 文書に provider 名・モデル名のリテラルが現れない — これは Codex がデフォルトの場合も同様である。

### D4: ドキュメント更新

以下のドキュメントを implementation 時に更新する:

- `.claude/commands/track/review.md` Step 1: reviewer モデル・provider の手動解決ロジックを削除する。`sotp review codex-local`/`claude-local` を直接呼ぶ記述があれば `sotp review local` に統一する。コマンドが `agent-profiles.json` を読んで自動解決するため、skill は provider 名・モデル名を直接読まない。「Codex CLI is the only supported provider; `claude` is unsupported」という記述を削除する。この変更は **Codex がデフォルトの場合も含む** — Codex 経路も `sotp review local` 経由の自動解決になり、skill が `--model` を手動解決して渡すパスはなくなる。
- `.claude/commands/track/review.md` Step 4/5: ローカルレビューコマンドの `--reviewer-model` / `--model` 引数を削除し、`--round-type` / `--group` / `--track-id` / `--briefing-file` のみを渡す形に更新する。provider の具体名・モデル名のリテラルは skill 文書から完全になくなる (Codex / Claude どちらが設定されていても同一のコマンド形式)。
- `.claude/commands/track/pr-review.md` Step 0: `reviewer` ではなく `pr-reviewer` capability を参照するよう更新する (D5)。structured provider set は引き続き `codex` のみ。
- `.claude/rules/10-guardrails.md` の "Reviewer Capability Constraint" 節: `claude-heavy` プロファイル / `subagent_type: "Explore"` を用いた reviewer 代替の言及を削除または置き換え、`provider: claude` の場合は D3 の自動解決・dispatch 経路 (コマンド内部で `ClaudeReviewer` が起動される) が公式な Claude reviewer 経路であると明記する。Explore subagent による self-review はどの profile でも `zero_findings` 代替として認められないことを維持する。

### D5: PR ベースレビュー provider をローカルレビュー provider から分離 — `pr-reviewer` capability 新設

**問題**: 現状 `pr.rs` の `trigger_review` / `review_cycle` は `resolve_execution("reviewer", RoundType::Final)` で provider を取得した後 `validate_reviewer_provider(&resolved.provider)` を呼ぶ。`STRUCTURED_PROVIDERS = &["codex"]` のため `validate_reviewer_provider("claude")` は `PrReviewError::UnsupportedProvider` で即時 fail-closed する。つまり D2/D3 で `reviewer.provider = claude` を設定すると `/track:pr-review` が起動直後にエラー終了し、ローカルレビューと PR ベースレビューを両立できない。

**決定**: `agent-profiles.json` に `pr-reviewer` capability を新設し、`/track:pr-review` の provider 解決をそこに向ける。

選択理由 (`pr-reviewer` capability vs `reviewer.pr_provider` フィールド): `pr-reviewer` を独立 capability とする方が既存の `capabilities.<name>` モデルと一貫しており、将来 `pr-reviewer` に固有の設定 (タイムアウト、ポーリング間隔等) を追加する余地もある。`reviewer.pr_provider` は `reviewer` エントリが肥大化する上、`CapabilityConfigDto` の `#[serde(deny_unknown_fields)]` による追加フィールド禁止に抵触するためスキーマ変更が必要になる。

デフォルト設定 (既存動作を維持):

<!-- illustrative, non-canonical -->
```json
"pr-reviewer": {
  "provider": "codex"
}
```

`pr.rs` の変更:

- `trigger_review` / `review_cycle` は `resolve_execution("reviewer", RoundType::Final)` → `resolve_execution("pr-reviewer", RoundType::Final)` に差し替える。
- `validate_reviewer_provider` のセマンティクスを明確化する: この関数は「PR ベースレビューが Codex Cloud 互換 provider を使っているか」を検証するものであり、ローカルレビュー provider (`reviewer.provider`) は検証しない。
- `pr-reviewer` capability が未定義の場合は fail-closed エラー (`reviewer capability not defined` と同様のパターン)。

**結果**: `reviewer.provider = claude` はローカルレビュー (`/track:review`) の provider のみを制御する。`/track:pr-review` は `pr-reviewer.provider` (デフォルト `codex`) を参照し、`reviewer.provider` に何を設定しても `/track:pr-review` への影響はゼロ。D1 の「`/track:pr-review` は Codex Cloud 専用」という決定が構造的に担保される。

## Rejected Alternatives

### A. Jury parallel (両 provider を同一 round で並列実行)

review-fix-lead 内で Codex と Claude の両 reviewer を並列に呼び出し、findings の union で fix し、両者が `zero_findings` を宣言したら exit する設計。

却下理由: review-fix-lead / review.json schema / briefing dispatch すべてに改造が及ぶ。選択可能にするという目的に対してアーキテクチャの侵襲が大きく、保守コストが見合わない。

### B. Shadow mode (片方が読み専)

primary reviewer が ladder を駆動し、shadow reviewer は同 briefing を読んで findings を別ファイルに記録するだけの設計。

却下理由: shadow が見る artifact は primary の fix 反映後 (= primary が発見した問題は既に修正済み) なので、shadow の findings count が構造的に primary 側に有利な bias を持つ。provider の能力を公平に評価できない。

### C. Serial single-reviewer swap (track ごとに reviewer を切り替え)

ある track では全 ladder を Codex、別の track では全 ladder を Claude で回す運用。

却下理由: 選択可能にするという目的は満たすが、`agent-profiles.json` の設定変更が track ごとに必要になり、実質的にグローバル設定を track ごとに書き換える運用になる。D3 の mixed-provider ladder と比べて柔軟性に欠ける。

## Consequences

### Positive

- テンプレートユーザーが自分の契約・好み・モデル性能の変化に応じて reviewer provider を選べるようになる
- Codex デフォルトのため、既存の動作は変わらない
- `review-fix-lead` の provider 選択 (sibling ADR: `2026-05-23-1848-review-fix-lead-codex-migration.md`) と組み合わせることで、reviewer × fixer の 2×2 の組み合わせが設定可能になる
- 既存の verdict codec (`REVIEW_OUTPUT_SCHEMA_JSON` / `parse_review_final_message`) を変更せずに流用できる
- `Reviewer` usecase port はすでに provider 中立に設計されており、`ClaudeReviewer` 追加の実装コストが低い
- reviewer の provider/model 解決がプロンプトから Rust 機構に移動する (enforce-by-mechanism 原則に沿う; agent briefing アンチパターンの surface が縮小する)。この変更は **Codex がデフォルトの場合も含む** — 現行の Codex 経路も `sotp review local` 統合エントリポイント経由に変わり、skill が `--model` を手動解決して渡すパスはなくなる
- ローカルレビュー skill 文書 (`.claude/commands/track/review.md`) からモデル名・provider 名のリテラルが消える。`gpt-\d+` パターンに合致する Codex モデルリテラルが review 経路の skill 文書に混入する構造的リスクが解消され、`verify-orchestra` のハードコードモデルリテラル検査違反を構造的に防ぐ。この恩恵は設定が Codex のままであっても得られる (なお `pr-review.md` は D5 により `pr-reviewer` capability を参照するよう更新されるが、PR ベースレビューの Codex 専用制約は構造的制約として残る)
- provider dispatch のロジックがコマンド 1 箇所に集約され、skill 文書が provider 名を知らなくてよくなる。`review-fix-lead` の provider 解決と対称な仕組みになる
- `reviewer.provider = claude` と `/track:pr-review` が共存できる (D5 による `pr-reviewer` 分離の結果)。D1 の「`/track:pr-review` は Codex Cloud 専用」という制約が config 設定の影響を受けず構造的に担保される

### Negative

- `sotp review claude-local` サブコマンドと `ClaudeReviewer` 実装を追加・保守する必要がある
- `pr-reviewer` capability を `agent-profiles.json` に新設・保守する必要がある (既存 `reviewer` エントリとは独立したトップレベルエントリを追加する移行が必要)
- 並列 review-fix-lead 実行時の cargo build lock 競合は未解決のまま (本 ADR の scope 外)

### Neutral

- `claude -p` の read-only 起動は、ファイル書き込みを構造的に行わないため hook coverage 問題 (workspace-write サンドボックス対比) は発生しない
- mixed-provider ladder (D3) はオプション; 未指定ならフォールバックで従来動作が維持される

## Reassess When

- `/track:pr-review` で Claude GitHub App 相当の仕組みが利用可能になり、`pr-reviewer` capability に Claude オプションを追加することを検討する余地が生まれた時点
- 新モデル世代で reviewer 向けの provider 適性が大きく変わり、デフォルトを見直す必要が生じた時点
- `claude -p` の `--json-schema` / `--output-format json` の動作仕様が変わり、D2 で前提とした API レベルのスキーマ強制が崩れた時点
- `STRUCTURED_PROVIDERS` に新たな Codex Cloud 互換 provider が加わり、`pr-reviewer` のデフォルトや選択肢を拡げる余地が生まれた時点

## Related

- `knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md` — review-fix-lead の provider を選択可能にした sibling ADR (reviewer × fixer 2×2 の fixer 側)
- `knowledge/adr/2026-03-17-0000-reviewer-model-profiles.md` — reviewer model profile を `agent-profiles.json` 配下に集約する原典
- `knowledge/conventions/pre-track-adr-authoring.md` — pre-track ADR の運用ルール
- `.harness/config/agent-profiles.json` — capability/provider 解決の SSoT
- Claude Code headless (print-mode) docs — `--output-format json` / `--json-schema` / `--bare` フラグの仕様 (D2 の根拠: `structured_output` フィールドと API レベル制約付きデコーディングの確認元)
- Claude Code CLI reference — `claude -p` オプション一覧 (D2 呼び出し形式の確認元)
