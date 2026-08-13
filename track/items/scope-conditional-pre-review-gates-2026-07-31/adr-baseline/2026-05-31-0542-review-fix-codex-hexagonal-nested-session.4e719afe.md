---
adr_id: 2026-05-31-0542-review-fix-codex-hexagonal-nested-session
decisions:
  - id: D1
    user_decision_ref: "chat_segment:review-fix-codex-hexagonal-nested-session-design:2026-05-31"
    candidate_selection: "from:[cli-composition-only, hexagonal-port-adapter] chose:hexagonal-port-adapter"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:review-fix-codex-hexagonal-nested-session-design:2026-05-31"
    candidate_selection: "from:[de-nest, broad-parallel-isolation, keep-nesting-sandbox-config] chose:keep-nesting-sandbox-config"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:review-fix-codex-hexagonal-nested-session-design:2026-05-31"
    candidate_selection: "from:[skip-validation, dogfooding-validate-keep-if-stable] chose:dogfooding-validate-keep-if-stable"
    status: proposed
---
# Codex review-fix-lead の hexagonal Rust 化 + 入れ子 reviewer の session 作成失敗の解消 + 自己 dogfooding 検証

## Context

Codex review-fix-lead capability（ADR `2026-05-23-1848-review-fix-lead-codex-migration.md` で導入）は、現在 `Makefile.toml` の `track-local-review-fix-codex` タスク（約 280 行のインライン bash、803-1082 行）として実装されている。引数パース・起動時 smoke-test・credential isolation（GITHUB_TOKEN / SSH 系除外、一時 HOME 作成）・プロンプト構築・`codex exec --sandbox workspace-write` 起動・return value sentinel パース・exit-code マッピングまで、全オーケストレーションを shell で行っている。

一方、reviewer capability は層分離されている：

- usecase port `Reviewer`（`libs/usecase/src/review_v2/ports.rs:7`、`review()` / `fast_review()`）
- infrastructure adapter `CodexReviewer` / `ClaudeReviewer`（`libs/infrastructure/src/review_v2/`）
- cli-composition の wiring（`apps/cli-composition/src/review_v2/mod.rs:150`、`profiles.resolve_execution("reviewer", round_type)` で provider を解決して adapter を選ぶ）

review-fix-lead（fixer）には対応する port も adapter も無く、ヘキサゴナル構造を欠いている。確立済みの移行方針（shell/Python → Rust の sotp CLI 統一、ADR `2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md`／cargo make タスクは `bin/sotp make <task>` へ委譲、`.claude/rules/07-dev-environment.md` の sotp make Dispatch）に照らすと、この fixer は review 経路に残る最後の大きな shell オーケストレーションであり、Rust へ移すべき対象である。

### 入れ子 reviewer の session 作成失敗（再現で root cause を確定）

Codex review-fix-lead は agentic loop（review → fix → 再 review）を1つの codex agent の中で回す。fixer は `codex exec --sandbox workspace-write` で起動され、その中で reviewer を**入れ子起動**する（`Makefile.toml:1000`: fixer の prompt が `cargo make track-local-review` を指示 → `sotp review local` → `CodexReviewer` → `codex exec`）。

運用（2026-05-26）で「session 作成失敗（`Failed to create session: Read-only file system (os error 30)`）」が観測され、当時は「並列衝突」と推測されていたが、再現調査で root cause が確定した：

- **`codex sandbox linux` で workspace-write 相当の sandbox を任意コマンドにかけて検証**したところ、sandbox 下の writable root は `cwd` / `/tmp` / `$TMPDIR` / `~/.codex/memories` に限られ、**`~/.codex/sessions` は read-only**（`touch ~/.codex/sessions/...` が `Read-only file system` で失敗）。
- 入れ子の `codex exec` は起動時に session を `~/.codex/sessions` に書こうとするが、外側 sandbox の landlock 下でここが read-only のため `Failed to create session: Read-only file system` で落ちる。`codex sandbox linux --full-auto -- codex exec --sandbox read-only ...` で**そのまま再現**した。
- **これは並列固有ではない**。入れ子は単独でも失敗する。当時の「並列衝突」という framing は誤診で、真因は「入れ子 reviewer の session ディレクトリが外側 sandbox の writable root 外」。`docker run --rm` の cargo build lock や `track-active-gate` 経由の rendered view/signal 再生成は、Claude rfl 経路も並列で同様に実行するが失敗しないため、本失敗の原因ではない。
- web 調査（OpenAI codex の docs / GitHub issues #16790, #23601）も、workspace-write の writable root に `~/.codex` が含まれず session/tmp 書き込みが EROFS になることを裏付ける。

### 修正の実証

入れ子を維持したまま、外側 fixer の sandbox に2つの設定を加えると入れ子 reviewer が完全動作することを実演で確認した：

- `sandbox_workspace_write.writable_roots=["~/.codex"]` → 入れ子 reviewer が session を作成できる（EROFS 解消、session id が振られる）。
- `sandbox_workspace_write.network_access=true` → 入れ子 reviewer が自分の LLM API に到達できる（writable_roots だけでは session は作れるが network が無いと応答できない）。

`codex sandbox linux --full-auto -c 'sandbox_workspace_write.writable_roots=["~/.codex"]' -c 'sandbox_workspace_write.network_access=true' -- codex exec --sandbox read-only '...'` で、入れ子 codex が session 作成 → 応答まで完走した。

## Decision

### D1: Codex fixer を usecase port + infrastructure adapter として実装する（reviewer と対称）

review-fix-lead の Rust 実装を、reviewer と同じヘキサゴナル構造で配置する：

- usecase 層に fixer の port（`Reviewer` の sibling、例: `ReviewFixRunner`）を定義する。port は briefing / scope / scope-files / round-type / reviewer-model を入力に取り、`completed` / `blocked_cross_scope` / `failed` の結果を返す契約とする。
- infrastructure 層に Codex 実装の adapter（`CodexReviewer` の sibling、例: `CodexReviewFixRunner`）を置く。smoke-test・credential isolation・codex 実行環境の構築（D2 の sandbox config を含む）・`codex exec` 起動・sentinel パースはこの adapter に収める。
- usecase 層に boundary 用 DTO（command / output）を置く。
- cli-composition が `profiles.resolve_execution("review-fix-lead", round_type)` で provider を解決し adapter を wiring する（reviewer の `mod.rs:150` と同じ機構）。
- apps/cli に clap サブコマンド（例: `sotp review fix-local`）を追加し、現行の 7 フラグ（`--scope` / `--briefing-file` / `--track-id` / `--round-type` / `--reviewer-model` / `--model` / `--scope-files`）を受ける。
- `Makefile.toml` の `track-local-review-fix-codex` は `bin/sotp make track-local-review-fix-codex "$@"` の thin passthrough に置き換え、インライン bash を削除する。

port / adapter / DTO の正確な型形状・メソッド・kind は type-design（Phase 2）で確定する。層の選択は `architecture-rules.json` とヘキサゴナル原則に従う：プロセス起動・外部 CLI 実行・実行環境構築は infrastructure adapter、契約と orchestration は usecase、wiring は composition root、引数表面は cli。

### D2: 入れ子を維持し、外側 fixer の sandbox 設定で入れ子 reviewer の session 作成失敗を解消する

Codex review-fix-lead は agentic loop を1つの codex agent コンテキストで回す（reviewer を入れ子起動する）構造を**維持する**。loop を Rust 側に分解して reviewer を host レベルで別個に呼ぶ案（de-nest）は採らない（Rejected Alternatives B）。

入れ子 reviewer の session 作成失敗（Context で確定した root cause）は、D1 の Rust adapter が外側 fixer の `codex exec --sandbox workspace-write` を起動する際に次の sandbox 設定を与えることで解消する（実演で完走を確認済み）：

- **`sandbox_workspace_write.writable_roots` に `CODEX_HOME`（`~/.codex`）を含める** — 入れ子 reviewer が session（`~/.codex/sessions`）を作成できるようにする。
- **`sandbox_workspace_write.network_access=true`** — 入れ子 reviewer が自分の LLM API に到達できるようにする。

（より厳格な代替として、入れ子 reviewer の `CODEX_HOME` を呼び出しごとに writable な一時パス（`/tmp` 配下や workspace 内）へ向け、auth を env で供給して `~/.codex` を read-only のまま保つ方式もある。session 書き込みの解消にはどちらも有効で、writable_roots 方式が実演済みの基準線。具体機構は type-design / 実装で確定する。）

**安全性（network_access を開いても push 防止は維持される）：** wrapper ADR `2026-05-23-1848` D3 の安全モデルは `network_access` と独立に成立する。

- `.git` read-only（workspace-write のファイルシステム保護）は network 設定と無関係 → ローカル git 操作は依然不可。
- credential isolation（GITHUB_TOKEN 除外・`GIT_SSH_COMMAND=/bin/false`・SSH 鍵なし）も network 設定と無関係 → network があっても push は認証できない。

`network_access=true` は外側 fixer sandbox 全体に対してアウトバウンドネットワークを開く（fixer が起動するすべてのプロセスが対象）。これは入れ子 reviewer が自分の LLM API に到達するために必要であり、fixer の本来機能に内在する（外側 fixer codex も元々 LLM network を使う）。なお、network を開いても push 防止は維持される。push 防止（.git 保護 + credential isolation）は network 設定と独立に構造的に成立する。

### D3: 自己参照 dogfooding で検証する（安定すれば Codex 固定を維持してよい）

新しい Codex review-fix-lead 実装は、**自己参照 dogfooding**（その実装を導入する変更集合そのものに対して fixer を走らせる）で検証する。現状 Codex fixer は入れ子 reviewer の session 作成失敗で事実上動かないため、dogfooding は D2 の修正が効いていることの実負荷検証になる。dogfooding のため agent-profiles.json の `review-fix-lead.provider` を `codex` に切り替える。

検証で Codex rfl が安定すれば、この固定を revert する必要はなく `codex` のまま運用してよい。安定しなければ `claude` に戻す。いずれも wrapper ADR `2026-05-23-1848` D5 の「provider 選択可能」オプションの行使であり、維持か revert かは dogfooding の結果に基づく。

## Rejected Alternatives

### A. cli-composition のみに閉じた配置（port / adapter を作らない）

プロセス管理を `apps/cli-composition` に `std::process::Command` で閉じ、usecase port / infrastructure adapter を設けない案。

却下理由：reviewer がヘキサゴナル構造（port + adapter）を持つのに fixer だけ持たないのは非対称で、「正しい層配置」という要件を満たさない。fixer の契約を port として定義しなければ mock によるユニットテストもできず、テスト容易性も損なわれる。

### B. De-nest（Rust が loop を駆動し、reviewer を host レベルで別個に呼ぶ）

入れ子を解消し、Rust adapter が「reviewer 呼び出し → findings → fixer-agent 呼び出し」を駆動する。reviewer は外側 sandbox の外（host レベル）で動くため session 書き込みも network も問題なく、root cause を構造的に回避できる。

却下理由：review-fix-lead の価値は「1つの codex agent が review → fix → 再 review を一貫したコンテキストで回す」agentic loop の連続性にある。de-nest すると fixer agent が review と fix の連続コンテキストを失い、各ステップが分断される。session 失敗は D2 の sandbox 設定（writable_roots + network_access）で入れ子のまま解消できることを実演で確認したため、コンテキストを犠牲にする de-nest は不要。

### C. 失敗を「並列衝突」と捉え、per-scope の包括的分離を作り込む

build target 分離・sync-views/signal 再生成の直列化など、並列レビュー経路全体の per-scope 資源分離を構築する。

却下理由：再現調査により、失敗は並列固有ではなく（入れ子は単独でも失敗）、真因は入れ子 reviewer の session ディレクトリが外側 sandbox の writable root 外にあることと確定した。build lock / sync-views race は Claude rfl 経路も並列で実行するが失敗しないため本失敗の原因ではない。包括的分離は誤診に基づく過剰実装になる。

### D. dogfooding を経ずに provider を Codex へ切り替える

安定性の実負荷検証をせずに review-fix-lead を Codex に固定する。

却下理由：Codex rfl の安定性は自己参照 dogfooding（D3）で検証してから維持を判断する。検証を経ずに切り替えると、入れ子 reviewer の session / network 等の不具合が運用で再露見しうる。維持か revert かは dogfooding の結果に基づく（D3）。

## Consequences

### Good

- fixer が reviewer と同じテスト容易性・層責務分離を獲得する（port の mock でユニットテスト可能になる）。
- 入れ子 reviewer の session 作成失敗が構造的に解消され、現状事実上動かない Codex review-fix-lead が動作するようになる。agentic loop の一貫したコンテキストは維持される。
- review 経路に残る最後の大きな shell オーケストレーションが Rust 化され、cargo make タスクが thin passthrough に揃う。
- dogfooding により、実装そのものが実負荷で検証される。

### Bad

- 作業範囲が大きい：port + adapter + DTO + clap + wiring に加え、sandbox 設定の実装と dogfooding が重なる。task 分割が前提。
- `sandbox_workspace_write.writable_roots` に `~/.codex` を含めると、fixer が（session 書き込みのために）`~/.codex` 全体に書き込めるようになる。auth/config への意図しない変更余地が生じる（厳格代替の CODEX_HOME 一時パス方式で緩和可能）。

### Neutral

- `network_access=true` は push 防止を弱めない（.git read-only + credential isolation が network と独立に成立）。
- provider 固定は dogfooding で安定性を検証し、安定すれば維持してよい（revert は必須ではない）。維持か revert かは検証結果に基づく。
- codex の `failed to record rollout items` telemetry エラーは入れ子でも残るが応答自体は成立する非致命的事象であり、本 ADR の対象外。

## Reassess When

- Codex CLI の sandbox / writable_roots / session ディレクトリの挙動が変わり、入れ子 reviewer の session 作成に追加設定が不要になったとき。
- Codex の `network_access` の意味づけが変わり、push 防止モデルの再評価が必要になったとき。
- cli 以外の delivery adapter が増え、fixer port を multi-adapter で再評価する必要が出たとき。
- reviewer と fixer の port が共通抽象に収れんできる構造が見えたとき。

## Related

- `knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md` — 本 ADR が拡張する wrapper ADR。並列衝突を Negative（scope 外）として残していたが、再現調査により真因は入れ子 reviewer の session 作成失敗（並列固有ではない）と確定し、本 ADR で解消する。
- `knowledge/adr/2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md` — shell/Python → Rust（sotp CLI 統一）の移行方針。
- `knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md` — cli-composition を composition root とする決定（fixer の wiring 配置先）。
- `knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md` — cli は usecase 経由でアクセスする決定。
- `.claude/rules/07-dev-environment.md` — sotp make Dispatch（cargo make → bin/sotp make 委譲）。
