---
adr_id: 2026-05-23-1848-review-fix-lead-codex-migration
decisions:
  - id: D1
    user_decision_ref: "chat_segment:review-fix-lead-codex-migration-design:2026-05-23"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:review-fix-lead-codex-migration-design:2026-05-23"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:review-fix-lead-codex-migration-design:2026-05-23"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:review-fix-lead-codex-migration-design:2026-05-23"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:review-fix-lead-codex-migration-design:2026-05-23"
    status: proposed
---
# review-fix-lead の provider を選択可能にする (Claude デフォルト、Codex オプション)

## Context

SoTOHE の review-fix-lead capability は現在 Claude Code (claude-opus-4-7) で実装されている (`.claude/agents/review-fix-lead.md`)。1 review scope (例: domain, infrastructure, cli) を担当し、`review → fix → ci-rust → 再 review` の loop を `zero_findings` に到達するまで自走する subagent。

review-fix-lead が担う agentic fix loop は、「コマンドを実行する → ログを読む → 修正する → 再実行する」の繰り返しを中心とする作業であり、Codex/GPT-5.5 はこの種の作業が得意なモデルとして知られている。一方、orchestrator / spec-designer / type-designer / impl-planner / implementer などの役割は Claude Code の subagent spawning / hook 基盤 / multilingual 強みが重要であり、Codex に切り替えると失うものが大きい。

このため、review-fix-lead の provider はテンプレートユーザーが自分の環境・好みに応じて選べる構成にするのが適切である。Claude をデフォルトとして従来動作を維持しつつ、Codex を追加の選択肢として提供する。

**現在の capability 構成** (`.harness/config/agent-profiles.json` 参照):
- `reviewer`: codex / gpt-5.5 (fast: gpt-5.4-mini) — 既に Codex
- `review-fix-lead`: capability エントリ未設定。現状は `.claude/agents/review-fix-lead.md` に定義された Claude subagent が担う
- 本 ADR は `review-fix-lead` capability を `agent-profiles.json` に新設し、Claude と Codex を選択可能にする

**安全モデルの解決** (2026-05-23): Codex の `workspace-write` サンドボックスでは `.git` ディレクトリがファイルシステムレベルで読み取り専用として保護される。これは OpenAI ドキュメント ("Protected paths in writable roots") で明記されており、本プロジェクト (WSL2/Linux, Codex CLI v0.125.0) でも実証済みである: `git add` は exit 128 `fatal: Unable to create '.../.git/index.lock': Read-only file system` で失敗し、ファイルはステージされなかった (`knowledge/research/2026-05-23-1848-agent-agnostic-vcs-guardrail.md` の §5 を参照)。この保護は設定キーで無効化できない。したがって、fixer を workspace-write サンドボックス内で起動する限り、git への書き込みは構造的に不可能であり、ポスト実行バリデーションに依存する必要はない。

## Decision

### D1: Codex オプションのスコープ — review-fix-lead のみ

orchestrator / implementer / spec-designer / type-designer / impl-planner / adr-editor は Claude Code (claude-opus-4-7) のまま据え置く。Codex を provider の選択肢として追加するのは **review-fix-lead 単体**。

理由:

- review-fix-lead が担う agentic fix loop は Codex が得意とする作業領域に近い
- 他の役割は Claude Code の subagent spawning / hook coverage / SWE-bench Pro 優位が効くため、Codex に切り替えると失うものが大きい
- review-fix-lead の contract (Input/Output) は provider-agnostic な形に既に整理されており、provider 切替のコストが低い

### D2: Codex 起動経路 — `codex exec --sandbox workspace-write`

review-fix-lead の Codex 版を `codex exec --model gpt-5.5 --sandbox workspace-write` (または同等の `--full-auto`、どちらも workspace-write を意味する) で起動する wrapper を新設する。例: `bin/sotp` のサブコマンドか `cargo make track-local-review-fix-codex` 相当。

wrapper の責務:

1. orchestrator から渡された briefing file path / scope 名 / scope file list を Codex prompt に inject
2. Codex が `cargo make track-local-review` (= 既存 reviewer) と shell-level のファイル編集操作を使って loop を回す
3. Codex 終了後、return value (`completed` / `blocked_cross_scope` / `failed`) を parse して orchestrator に返す

**禁止**: fixer を `--sandbox danger-full-access` または `--dangerously-bypass-approvals-and-sandbox` で起動してはならない。これらのフラグは `.git` 保護を解除し、D3 の安全保証を無効化する。

既存 Claude review-fix-lead (`.claude/agents/review-fix-lead.md`) は引き続き残置し、`agent-profiles.json` の設定で provider を切り替えられるようにする (D5 参照)。

### D3: 安全保証 — 起動時 smoke-test + defense-in-depth

この decision が対象とする安全保証は **2 種類の異なる懸念** に分かれる:

1. **ローカル git 書き込み操作の防止** (stage / commit / branch 操作): Codex `workspace-write` サンドボックスの `.git` 読み取り専用保護で構造的に担保される (`knowledge/research/2026-05-23-1848-agent-agnostic-vcs-guardrail.md` §5 に根拠の実証記録あり)。ユーザーはこの実証結果を確認した上で本安全モデルを 2026-05-23 の会話で承認した。`git push` (リモート書き込み) は `.git` ローカル保護の対象外であり、push 防止は補完保護 1 (credential isolation) が担う。
2. **scope 範囲外ファイルの編集防止**: `.git` 保護はこれを担保しない (Codex は workspace 内の任意ファイルを編集できる)。scope 境界の遵守は **briefing contract** (review-fix-lead の briefing が「スコープ内のファイルのみ編集可能」を明示) と **orchestrator の pre-commit レビュー** (`git diff --stat` 等での確認) に依存する。これらは構造的なサンドボックス保護ではなく behavioral contract であり、scope 違反ファイルが混入するリスクはゼロではない (accepted risk)。

この 2 点の分離を踏まえ、ポスト実行の **禁止 git 操作スキャン** は `.git` 保護により冗長であり不要。ポスト実行の **scope 違反検出** はオプションだが、briefing contract + orchestrator の pre-commit レビューが代替手段として機能するため、専用バリデーション実装は必須としない。このトレードオフ (scope 違反の構造的防止より実装コスト削減を優先) はユーザーが 2026-05-23 の会話で承認した。

wrapper はループ開始前に **起動時 smoke-test** を実施する:

1. **サンドボックス確認**: fixer を起動する前に、`workspace-write` フラグが実際に渡されることをアサートする (設定ミスによる `danger-full-access` 起動を防ぐ)
2. **Codex CLI バージョン確認**: `.git` 読み取り専用保護が維持されているバージョン範囲内であることを確認する (バージョン固定 + 事前チェック)

防御の深さ (defense-in-depth):

- **主要保護**: Codex workspace-write サンドボックスの `.git` 読み取り専用 (構造的、設定で無効化不可)
- **補完保護 1**: fixer に `GITHUB_TOKEN` / SSH 鍵を渡さない (credential isolation)。ファイルシステム保護が退行しても push 手段がない
- **補完保護 2**: GitHub リポジトリ側の branch protection / ruleset。local actor が回避できないサーバーサイドの最終防衛線

trusted orchestrator (Claude Code、hook 対象) が既存の guarded wrapper (`cargo make track-commit-message` 等) 経由でコミットを実施する。fixer はファイル編集のみ行い、コミットは行わない。

### D4: Codex 用 briefing template の新設

現 `.claude/agents/review-fix-lead.md` は Claude tool 名 (`Read` / `Grep` / `Glob` / `Edit`) を前提とした記述を含む。Codex 用 briefing では:

- `Read` → `cat` 相当 (もしくは Codex が直接ファイルを開く文言)
- `Grep` → `grep` / `rg` 使用指示
- `Edit` → patch 適用相当の Codex 慣用句
- `cargo make track-local-review` → そのまま (shell command なので変換不要)

briefing template は `.claude/agents/review-fix-lead.md` をそのまま流用せず、Codex 用に並行版 (例: `.harness/briefings/review-fix-lead-codex.md`) を新設する。共有部 (mission, contract, scope ownership, severity policy 参照) は文面を揃え、tool 指示部のみ provider 別に書く。

### D5: agent-profiles.json への provider 切替機構追加

`agent-profiles.json` の `review-fix-lead` capability (新設) を追加し、provider を選べるようにする。デフォルトは `claude` で従来動作を維持する。Codex を選びたいユーザーは `provider` を `codex` に変更する。

デフォルト設定:

<!-- illustrative, non-canonical -->
```json
"review-fix-lead": {
  "provider": "claude",
  "model": "claude-opus-4-7"
}
```

Codex を選ぶ場合:

<!-- illustrative, non-canonical -->
```json
"review-fix-lead": {
  "provider": "codex",
  "model": "gpt-5.5",
  "wrapper": "track-local-review-fix-codex"
}
```

orchestrator (`/track:review`) は `agent-profiles.json` を読んで spawn 経路を選ぶ。`provider == claude` なら従来どおり `Agent(subagent_type="review-fix-lead")`、`provider == codex` なら wrapper (D2) を Bash 経由で起動。

## Rejected Alternatives

### A. orchestrator + implementer + review-fix-lead をまとめて Codex に置換

subagent spawning / hook coverage / SWE-bench Pro / MMMLU の同時負により net 損失が大きい。本 ADR は review-fix-lead 単体に選択肢を追加することで、Codex の agentic fix loop の強みを活かしつつ他の役割の安定性を維持する。

### B. shadow-mode fixer (Claude が driver、Codex が候補提示)

Claude review-fix-lead が main loop を駆動し、Codex は parallel に「自分ならこう直す」案を出して別ファイルに保存。最終的に Claude の fix が採用される。

却下理由: shadow の提案を merge する基準が機械化困難 (人手レビューになる)。provider を選択可能にするという目的に対して複雑さが見合わない。

### C. review-fix-lead を Claude のまま、内部 ci-rust 部分だけ Codex に委譲

agentic loop の中で「ci-rust の failure を読んで fix する」サブステップだけ Codex に投げる hybrid。

却下理由: round 内で Claude/Codex 切替が多発し、context handoff コストが loop 全体の効率を下げる。loop 全体を単一 provider に任せる方が構造がシンプル。

### D. ポスト実行での多層バリデーション (安全機構の旧案)

Codex 実行後に `git diff --name-only` で scope 違反を検出し、実行 log を走査して禁止 git 操作の痕跡を探す方式。

却下理由 (懸念別に整理):

- **禁止 git 操作スキャン (実行 log 走査)**: `workspace-write` サンドボックスが `.git` をファイルシステムレベルで読み取り専用にするため (OpenAI ドキュメント + 本プロジェクトでの実証)、Codex は git 操作を物理的に実行できない。事後スキャンは冗長であり「事後検出 = 失敗を検知するだけ」という弱点もない。起動時 smoke-test (D3) に置き換えることで、より確実な事前保証に移行する。
- **scope 違反ファイル検出 (`git diff --name-only`)**: `.git` 保護は scope 境界の強制とは無関係であり、この検出は技術的には有効な手段である。しかし、briefing contract (review-fix-lead は scope 内ファイルのみ編集) と orchestrator の pre-commit レビューが代替手段として機能するため、専用のポスト実行バリデーション実装は必須としない (オプション扱い)。

## Consequences

### Positive

- テンプレートユーザーが自分の好みや環境に応じて review-fix-lead の provider を選べるようになる
- Codex を選んだ場合、implementer (Opus) / reviewer (Codex) / fixer (Codex) という配置になり、loop 内が Codex 統一で context 切替なし
- provider 切替は `agent-profiles.json` の 1 行変更で完結し、切替コストが極小
- D3 の構造的サンドボックス保護により、Codex fixer が git 操作を行うリスクは構造的に排除されている
- ポスト実行バリデーションを廃止し起動時 smoke-test に一本化することで、wrapper の実装がシンプルになる

### Negative

- briefing template を Claude/Codex で並行保守する必要がある
- 並列 review-fix-lead 実行時の cargo build lock 競合は未解決のまま (本 ADR の scope 外)
- Codex CLI のバージョンが更新され `.git` 保護の動作が変わった場合、smoke-test が検出できないと保証が崩れる。Codex CLI バージョンのピン留めと定期確認が必要

### Neutral

- `--no-verify` による hook スキップは `.git` ファイルシステム保護より上位のレイヤーで動作するため、workspace-write 内では関係しない (fixer は hook を bypass する以前に git 操作自体ができない)
- Claude をデフォルトにするため、既存の動作は変わらない

## Reassess When

- Codex CLI のサンドボックス動作が変わり、workspace-write での `.git` 読み取り専用保護が保証されなくなった時点 (D3 の CLI バージョン範囲チェックはこれを近似的に検出するが、smoke-test が保護退行を見逃した場合は保証が崩れるため、Codex CLI の更新時には手動確認が必要)
- Codex CLI が Claude Code hook 互換機構 (subprocess 内での pre/post-tool fire) を提供するに至った時点 (保護の仕組みを見直す余地が生まれる)
- 新モデル世代で review-fix-lead 向けの provider 適性が大きく変わった時

## Related

- `.claude/agents/review-fix-lead.md` — Claude 版 review-fix-lead の定義
- `.claude/rules/10-guardrails.md` — hook coverage gap の規定 (workspace-write subprocess は hook 対象外)
- `knowledge/research/2026-05-23-1848-agent-agnostic-vcs-guardrail.md` — Codex workspace-write サンドボックスの `.git` 保護実証 (本 ADR D2/D3 の根拠)
- `knowledge/conventions/pre-track-adr-authoring.md` — pre-track ADR の運用ルール
- `.harness/config/agent-profiles.json` — capability/provider 解決の SSoT
