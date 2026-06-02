---
adr_id: 2026-06-01-2300-review-fixer-self-resolve-scope-files
decisions:
  - id: D1
    user_decision_ref: "chat:2026-06-01:review-fixer-self-resolve-scope-files"
    status: proposed
  - id: D2
    user_decision_ref: "chat:2026-06-01:review-fixer-self-resolve-scope-files"
    status: proposed
  - id: D3
    user_decision_ref: "chat:2026-06-02:review-fixer-drop-reviewer-model"
    status: proposed
---
# review fixer に orchestrator から渡す引数を最小化する — `--scope-files` と `--reviewer-model` を廃止する

## Context

review fixer（`sotp review fix-local` / `track-local-review-fix-codex`）は、編集してよいファイル集合（modification boundary）を `--scope-files` で orchestrator から外部受領する。orchestrator はこの一覧を作るために CLI のスコープ分類ロジックを複製する必要があり（`/track:review` skill が orchestrator に「apply the CLI classifier logic to the changed file list」と指示している）、次の問題を生む。

- orchestrator 側でのスコープ分類の重複。orchestrator が判断を持ち込む密輸の温床になる。
- orchestrator がファイルパスを Bash コマンド文字列に乗せるため、`.github`（"git" の部分文字列）が `block-direct-git-ops` フックに当たる footgun。
- 冗長。fixer は `--scope` と track 文脈を既に持ち、reviewer がスコープのファイル一覧を解決する（CLI が reviewer に scope file list を自動注入する）のと同じ正規分類で境界を解決できる。

加えて fixer は runtime で空の scope_files を fail-closed 拒否する（`EmptyScopeFiles` guard）ため、`--scope-files` は実質必須化している。fixer は Codex agent を skill で駆動するため、境界の解決は skill 内の指示で完結できる（Rust 実装の追加を要しない）。

同種の冗長が reviewer model にもある。fixer は入れ子で起動する reviewer の model を `--reviewer-model` フラグで orchestrator から受け取り、それを reviewer 起動コマンド（`cargo make track-local-review`）の `--model` に渡している。しかし `sotp review local`（= `track-local-review`）の `--model` は ad-hoc な上書き用の省略可能フラグであり、未指定時は `agent-profiles.json` の `reviewer` capability から round-type ごとに自動解決される（fast round は `fast_model`、final round は `model`）。つまり reviewer 起動コマンドが既に model を自己解決するため、fixer に reviewer model を外部から渡す必要はない。現在 fixer の prompt には "Reviewer model: {reviewer_model}" が埋め込まれ、コマンドに `--model {reviewer_model}` が付与されているが、これらは reviewer が自己解決する値の重複指定になっている。

本 ADR は `knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md` の D1 が定義した fixer 契約・CLI surface のうち、`scope-files` 入力と `reviewer-model` 入力を含む部分を後続決定として更新する。その他の port / adapter / DTO / clap / wiring / sandbox / dogfooding の判断は維持する。

## Decision

### D1: review-fix-lead skill が正規コマンドで境界を自己解決し、`--scope-files` を廃止する

review-fix-lead の Codex skill に、reviewer と同じ CLI の正規スコープ分類コマンド（例: `bin/sotp review files --scope <scope>`）を agent 自身が実行して modification boundary を得る、という指示を書く。これにより orchestrator が `--scope-files` を渡す必要がなくなる。`sotp review fix-local` の `--scope-files` フラグ・`RunReviewFixCommand` の該当フィールド・空拒否の `EmptyScopeFiles` guard を撤去する。

境界解決の責務は skill 内の正規コマンド呼び出しに移る。Rust 側に新たな分類 port を追加するのではなく、フラグと guard を撤去するだけ（純減）。空・失敗時の扱いは skill が正規コマンドの結果に基づいて記述する。

### D2: `/track:review` skill から scope-files 導出指示を削除する

orchestrator に対する「CLI classifier logic を適用して scope-files を導出・付与せよ」という指示を削除し、fixer 起動から `--scope-files` を外す。orchestrator はスコープ分類を複製しない。

### D3: reviewer model は入れ子の review コマンドが自己解決する — `--reviewer-model` を廃止する

reviewer 起動コマンド（`cargo make track-local-review` = `sotp review local`）は `agent-profiles.json` の `reviewer` capability から model を round-type ごとに自動解決する。fast round では `fast_model`、final round では `model` が使われる。そのため fixer に reviewer model を外部から渡す必要はなく、`--reviewer-model` フラグを廃止する。

撤去対象は次の通り:

- `sotp review fix-local` の `--reviewer-model` フラグ（`apps/cli/src/commands/review/fix_local.rs` の `FixLocalArgs.reviewer_model` フィールド）
- `RunReviewFixLocalInput` の `reviewer_model` フィールド（`apps/cli-composition/src/review_v2/inputs.rs`）
- `RunReviewFixCommand` の `reviewer_model` フィールド（`libs/usecase/src/review_v2/run_review_fix.rs`）
- `review_fix_runner` が組み立てる reviewer 起動コマンドの `--model {reviewer_model}` 指定と prompt 内の "Reviewer model: {reviewer_model}" 行（`libs/infrastructure/src/review_v2/review_fix_runner/prompt.rs`）
- `make_review_fix` ラッパー経由で渡される `--reviewer-model` 引数（`apps/cli/src/commands/make_review_fix.rs`）
- `/track:review` skill の reviewer-model 解決・受け渡し指示

撤去後、reviewer 起動コマンドは `--round-type` / `--group` / `--track-id` / `--briefing-file` のみを渡し model を明示しない。これは `sotp review local` を直接実行したときと同じ自己解決の経路に一本化する。

D1（`--scope-files` 撤去）と合わせ、orchestrator が fixer に渡す引数は `--scope` / `--briefing-file` / `--track-id` / `--round-type` の4つに最小化される。fixer 自身の model（`--model`）は既に省略可能で `review-fix-lead.model` から解決されており、変更不要。

D1 が skill 内の正規コマンド呼び出しでスコープ境界の解決ロジックを置き換えるのに対し、D3 は置き換えロジックを要しない純粋な撤去である。reviewer 起動コマンドが既に model を自己解決するため、fixer は単に `--model` を渡さなくなるだけで済む。

## Rejected Alternatives

### A. `--scope-files` を現状維持する

orchestrator 側のスコープ分類の重複、`.github` フック footgun、冗長をそのまま残す。却下。

### B. `--scope-files` を optional 化し、未指定時のみ自己解決する（フラグを残す）

二重経路が残り footgun も完全には消えず、境界解決の出所が一本化しない。却下。

## Consequences

### Positive

- orchestrator 側のスコープ分類の重複が消える。
- orchestrator がファイルパスを Bash コマンドに乗せないため、`.github` の `block-direct-git-ops` フック footgun が消える。
- 境界解決が reviewer と同一の CLI 正規分類に一本化される。
- 実装は Codex skill への正規コマンド呼び出し指示の追記と、CLI フラグ・guard・該当フィールドの撤去のみ。Rust に新たな分類 port を追加しない（純減）。
- orchestrator が fixer に渡す引数が `--scope` / `--briefing-file` / `--track-id` / `--round-type` の4つに最小化される。reviewer model の手渡し（`agent-profiles.json` が既に定義する値の重複指定）が消え、reviewer 起動が `sotp review local` を直接実行したときと同じ自己解決の経路に一本化される。

### Negative

- review-fix-lead skill が正規スコープ分類コマンドの実行結果に依存する。コマンドが利用可能であること、空・失敗を返した場合の扱いを skill が明示的に記述する必要がある。
- `reviewer_model` は cli / cli-composition / usecase / infrastructure の複数層を貫く public 型のフィールドであり、撤去は型契約変更（複数層の public 型からフィールドを削除する変更）になる。実装時に TDDD フローを要する。

### Neutral

- reviewer は既にスコープを自己解決しており、fixer skill をそれに揃えるだけ。

## Reassess When

- スコープ分類の CLI 機構が変わった場合。
- 境界を明示的に上書きする正当な必要が生じた場合（その時はフラグ再導入を検討する）。
- reviewer 起動コマンド（`sotp review local`）の model 自己解決機構が変わった場合（例: `agent-profiles.json` の `reviewer` capability からの round-type 別解決をやめた場合）。
- fixer が reviewer と異なる model を使う正当な必要が生じた場合（その時は `--reviewer-model` 再導入を検討する）。

## Related

- `knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md` — review-fix-lead の hexagonal Rust 化。その D1 が定義した 7 フラグ interface のうち、本 ADR は `scope-files` 入力（D1/D2）と `reviewer-model` 入力（D3）を後続決定として撤去し、fixer が orchestrator から受け取る引数を最小化する。
- `knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md` — scope 分類ロジックの CLI 公開（classify / files）。fixer skill が内部で利用する正規コマンド。
- `knowledge/adr/` — ADR 索引
