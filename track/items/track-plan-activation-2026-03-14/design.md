# Design: track:plan plan-only / activate 導線

## Purpose

この文書は、このトラックにおける canonical design artifact である。
`tmp/track-plan-activation-design-2026-03-13/` にあった検討メモは、この文書へ統合した
source notes とみなし、実装判断の正本はこの `design.md` と `spec.md` に置く。

## UX Decision

UX を優先する。
そのため、内部実装では既存 wrapper や `--plan-only` 引数を流用してよくても、
利用者が覚える primary path は明示的な first-class command として提供する。
ただし、その public path を live docs / registry copy に出すのは command surface と
onboarding surface が実装されたタイミングに限る。planning track の段階で先に告知しない。

採用する user-facing command は次の 2 つ。

- `/track:plan-only <feature>`
- `/track:activate <track-id>`

非採用:

- `--plan-only` を user-facing の primary path にすること
- `cargo make track-branch-create '<id>'` を user-facing の primary path にすること

これらの low-level entrypoint は内部実装・terminal 用として残してよいが、
通常の docs / AI 案内では前面に出さない。

## User-Facing Workflow

### Standard lane

通常の開始導線は維持する。

1. `/track:plan <feature>`
2. `/track:implement`
3. `/track:review` または `/track:pr-review`
4. `/track:ci`
5. `/track:commit <message>`

### Plan-only lane

計画だけ先に固めて main 側へ commit したい場合の導線は次とする。

1. `/track:plan-only <feature>`
2. `/track:ci` で planning-only diff が allowlist と CI 要件を満たすことを確認する
3. explicit `track-id` selector 付き `/track:review` で planning artifact を確認する
4. explicit `track-id` selector 付き `/track:commit <message>` で planning artifact を main 側へ landing する
5. 必要なら explicit `track-id` selector 付き `/track:pr-review` で planning-only PR を扱い、hidden な `track/<id>` 前提に戻らないよう wrapper / script も含めて PR lane を成立させる。merge は `/track:merge <pr>` を含む PR lane として扱い、selector は track-id ではなく PR 番号に固定する
6. `/track:activate <track-id>`
7. `/track:implement`
8. `/track:review` または `/track:pr-review`
9. `/track:ci`
10. `/track:commit <message>`

activation 前の `/track:review` / `/track:commit` / PR は全面禁止ではない。
禁止するのは implementation-phase の code-bearing flow であり、
planning artifact だけを扱う review / commit / PR はこの lane の正式手順として許可する。
ただし non-track branch 上では auto-detect が materialized active track を優先するため、
pre-activation の planning-artifact flow では explicit `track-id` selector を必須にする。
さらに planning-only とみなす diff は固定 allowlist に限定する。
MVP では `track/items/<id>/` 配下、`track/registry.md`、`track/tech-stack.md`、
`.claude/docs/DESIGN.md` だけを許可し、それ以外の差分を含む場合は implementation-phase work とみなして reject する。

### Transitional compatibility lane

`/track:full-cycle <task>` は現在 repo に存在する live entrypoint なので、
takt 廃止または command 再定義が完了するまでは互換 surface として扱う。

ただし、このトラックが規定する primary onboarding path には昇格させない。
残っている間は `/track:implement` と同じ activation guard を受け、
branch-null の planning-only track を bypass できないことだけを要求する。

### Resume lane

再開の入口は `/track:status` に集約する。

`/track:status` は候補列挙ではなく、phase に応じた
「次に叩くべき command を 1 つだけ」返す router として扱う。

同じ方針を `track/registry.md` の `Current Focus` と `Active Tracks` にも適用する。
planning-only track が最新 active track のときは、recommended next command を
`/track:implement` ではなく `/track:activate <track-id>` にする。
一方で、branch が materialize 済みの `planned` track は既存どおり implementation 導線に進めてよい。
このとき registry の header/footer copy も同じ UX 方針に従う。
「`/track:plan <feature>` だけが唯一の入口」と読める固定文言は残さない。

### Non-track branch resolution

branch-null planning-only track を main 上へ commit できるようにしても、
non-track branch 上の current focus がそれだけで branchless track へ乗り換わってはいけない。

期待する優先順位:

1. 現在 branch が `track/<id>` ならその track
2. そうでなければ、materialized な active track のうち最新のもの
3. materialized active track が無い場合に限り、branch-null planning-only track のうち最新のもの

これにより、`/track:status` や `/track:catchup` は既存の standard lane を保持しつつ、
materialized active track が存在しない repo だけで `Ready to Activate` を前面に出せる。
同じ優先順位は `track/registry.md` の `Current Focus` や `verify-latest-track` 相当の
latest-track verifier だけでなく、`scripts/external_guides.py` のように最新トラック文脈を
補助コンテキストとして読む loader にも適用する。
secondary surface の `/track:revert` も同じ current-track 解決規則に従う。

この優先順位を成立させるには、activation が non-track branch から観測可能でなければならない。
したがって `metadata.json.branch` を新しい `track/<id>` branch の上でだけ更新して終わりにする案は採らない。

## Workflow Phases

利用者向けには branch の有無ではなく phase 名で説明する。

- `Planning`
- `Ready to Activate`
- `In Progress`
- `Ready to Ship`
- `Done`

この phase は user-facing 概念であり、`metadata.status` と 1:1 で一致する必要はない。

最低限の期待挙動:

- planning-only track (`status=planned`, `branch=null`) は `Ready to Activate`
- materialized され、task がすべて `todo` の track は `Planning`
- task に `in_progress` / `done` が混在する track は `In Progress`

## Command Taxonomy

### Primary commands

通常利用者に覚えてほしい command はこれに限定する。

- `/track:catchup`
- `/track:plan <feature>`
- `/track:plan-only <feature>`
- `/track:activate <track-id>`
- `/track:status`
- `/track:implement`
- `/track:review`
- `/track:ci`
- `/track:commit <message>`
- `/track:done`

### Secondary commands

- `/track:pr-review`
- `/track:merge <pr>`
- `/track:revert`
- `/track:archive <id>`
- `/track:setup`

### Transitional compatibility commands

- `/track:full-cycle <task>`

`/track:full-cycle` は現時点では live command だが、
takt 廃止トラックとの整合が取れるまでは compatibility surface として扱う。
このトラックの責務は、残っている間の guard と routing の整合を保つことであり、
恒久的な primary lane として再定義することではない。

### Low-level / internal commands

- `cargo make track-branch-create '<id>'`
- `cargo make track-branch-switch '<id>'`
- `cargo make track-transition ...`
- `cargo make track-sync-views`

## Technical Model

### Planning-only track

planning-only track は次の条件でのみ許可する。

- `schema_version == 3`
- `status == "planned"`
- `branch == null`
- task が空、または全 task が `todo`

これ以外の non-archived branchless state は reject する。

重要:

- Python validator だけでなく Rust validator / render path も同じ条件を受け入れる必要がある
- `track views validate` と `cargo make ci` が通る状態まで含めて完成とする

この区別は registry / status routing にも効く。
branch-null の planning-only track だけが `Ready to Activate` と `/track:activate` を返し、
branch が materialize 済みの `planned` track は既存どおり implementation 導線を返す。

### Activation

activation は branch create/switch だけでは不十分で、
`metadata.json.branch` の materialization までを含む。
さらに、その materialized state は source branch (`main` など) に戻っても観測できる必要がある。

期待挙動:

1. target track の `metadata.json` を読む
2. `track/<track-id>` を create/switch できるか、既存 branch を安全に再利用できるかを preflight で確認する
3. source branch 上で `metadata.json.branch = "track/<track-id>"` を書き込み、rendered views を sync する
4. その materialization を source branch からも観測できる永続化状態として確定する
5. その確定済み状態から `track/<track-id>` branch を create または switch する
6. 以後の implementation/review/commit は通常どおり branch guard の保護下に入る

MVP では step 3 を activation commit として扱うのが最も単純である。
つまり `/track:activate` は clean worktree を要求し、source branch 上で activation commit を作ってから
`track/<id>` へ移る。
これにより、track branch と source branch の両方から同じ materialized metadata が見える。

fail-closed 条件:

- target track が `branch=null`, `status=planned`, task 未着手の planning-only track でない場合は reject
- `track/<id>` が stale/divergent で安全に再利用できない場合は、metadata 永続化前に reject する
- invalid state を mutate せず、次に取るべき action を返す
- clean worktree でない source branch 上から activation しようとした場合は、永続化手順を安全に実行できないため reject する

recovery:

- metadata 永続化までは preflight failure で fail-fast し、source branch を汚さない
- metadata 永続化後に checkout だけ失敗した場合は、`/track:activate <track-id>` の再実行で branch switch を resume できるようにする

### Public vs Internal Activation Path

user-facing:

- `/track:activate <track-id>`

internal:

- `cargo make track-branch-create '<id>'`
- `cargo make track-branch-switch '<id>'`

内部的には `/track:activate` が上記 wrapper を使ってもよいし、
CLI subcommand を直接叩いてもよい。
ただし docs と AI 案内では `/track:activate` を canonical public path とする。

## Executor Coverage

planning-only lane は command docs だけ整えても成立しない。
実際に branch 前提や current-track auto-detect を持つ executor まで scope に含める。

local planning-only commit lane:

- `.claude/commands/track/review.md`
- `.claude/commands/track/commit.md`
- `apps/cli/src/commands/git.rs`
- `scripts/git_ops.py`
- `scripts/track_branch_guard.py`

この lane では explicit `track-id` selector が docs 上の慣習に留まらず、
guarded commit path まで渡ることが必要である。
MVP では既存の `track-dir` plumbing を活かし、non-track branch 上でも
planning-only artifact だけを対象にした review / commit が成立するようにする。

planning-only PR lane:

- `.claude/commands/track/pr-review.md`
- `.claude/commands/track/merge.md`
- `Makefile.toml`
- `apps/cli/src/commands/pr.rs`
- `scripts/pr_review.py`
- `scripts/pr_merge.py`

ここは現状 `track/<id>` branch 前提が強い。
このトラックでは secondary command として残す以上、public docs だけ先に書き換えるのではなく、
executor も explicit selector と planning-only allowlist に追従させる。
少なくとも hidden `track/<id>` 前提のまま public path が成功するようには見せない。
selector 規則も分ける。`/track:pr-review` は explicit `track-id` selector 必須、
`/track:merge` は PR 番号を canonical selector とし、non-track branch 上で
empty-args current-branch auto-detect に戻らないようにする。

activation / status routing lane:

- `.claude/commands/track/status.md`
- `apps/cli/src/commands/track.rs`

activation と status は user-facing phase router なので、
branch materialization や next-command recommendation が docs と renderer だけで一致していても不十分である。
actual CLI entrypoint まで同じ phase model を採用する。

## Guardrails

branch-null の planning-only track では planning は許可するが、
implementation-phase の入口は fail-closed にする。

許可:

- planning artifact の更新
- task 追加
- rendered view の再生成
- explicit `track-id` selector 付きで planning artifact だけを対象とした `/track:review`
- explicit `track-id` selector 付きで planning artifact だけを対象とした `/track:commit`
- explicit `track-id` selector 付きで planning artifact だけを対象とした PR 作成・レビュー
- explicit PR 番号付き `/track:merge <pr>` による planning-only PR のマージ準備
- 上記 allowlist に収まる planning-only diff

拒否:

- `todo -> in_progress`
- `done` / `skipped` への遷移
- `/track:implement`
- repo に残っている間の `/track:full-cycle`
- code-bearing review / commit / PR 系の implementation-phase flow

期待エラーは、単に失敗するだけでなく activation 導線を案内すること。

例:

```text
track is not activated yet; run /track:activate <track-id>
```

## Status Command Responsibilities

`/track:status` は summary だけでなく workflow router の役割を持つ。

最低限返すべき項目:

1. current phase
2. phase 判定理由
3. recommended next command
4. blocker

例:

```text
Current phase: Ready to Activate
Reason: track exists, status is planned, branch is not materialized yet
Recommended next command: /track:activate track-plan-activation-2026-03-14
Blocker: implementation commands are disabled until activation completes
```

## Documentation Policy

docs では branch/null/materialization の内部用語を前面に出しすぎない。

利用者向けには次の用語を優先する。

- planning-only track
- activated track
- ready to activate

内部仕様・validator・guardrail の節だけ branch/null を使う。

入口 docs も例外ではない。少なくとも次は UX-first 導線に同期する。

- `.claude/commands/track/plan.md`
- `.claude/commands/track/plan-only.md`
- `.claude/commands/track/activate.md`
- `.claude/commands/track/implement.md`
- `.claude/commands/track/status.md`
- `.claude/commands/track/commit.md`
- `.claude/commands/track/full-cycle.md`
- `.claude/commands/track/ci.md`
- `.claude/commands/track/review.md`
- `.claude/commands/track/pr-review.md`
- `.claude/commands/track/merge.md`
- `.claude/commands/track/revert.md`
- `.claude/commands/track/catchup.md`
- `.claude/commands/track/done.md`
- `START_HERE_HUMAN.md`
- `.claude/commands/track/setup.md`
- `.claude/docs/WORKFLOW.md`
- `.claude/hooks/agent-router.py`
- `.claude/hooks/block-direct-git-ops.py`
- `.claude/skills/track-plan/SKILL.md`
- `.claude/rules/07-dev-environment.md`
- `libs/domain/src/guard/policy.rs`
- `scripts/external_guides.py`
- `DEVELOPER_AI_WORKFLOW.md`
- `track/workflow.md`

新規利用者が最初に読む文書に旧導線だけが残る状態は許容しない。

## Implementation Slices

### T001

planning-only schema を Python/Rust 両方で受け入れられるようにする。

### T002

planning-only entrypoint と activation の technical path を実装する。
`/track:plan-only` は branch を自動作成しない planning artifact 作成 path を持ち、
`/track:activate` はその後段の materialization path を担う。
加えて、planning-only artifact を activation 前に main 側へ landing するための
explicit `track-id` selector 付き review / commit / PR 導線も同じ task で整理する。
これには `Makefile.toml`、`apps/cli/src/commands/pr.rs`、`scripts/pr_review.py`、`scripts/pr_merge.py` の PR executor と、
`apps/cli/src/commands/git.rs` / `scripts/git_ops.py` の guarded commit path、`apps/cli/src/commands/track.rs` の activation/status entrypoint を含める。
activation は source branch からも観測できる永続化 transaction とし、
track branch 上だけに `branch` 情報を閉じ込めない。

### T003

activation 前の implementation-phase 入口を fail-closed にする。
`/track:implement` と、repo に残っている間の `/track:full-cycle` を含めて guard 対象に入れる。
加えて、non-track branch 上の current-track resolution も materialized active track を優先し、
branch-null planning-only track が standard lane を乗っ取れないようにする。
この解決規則は `track_resolution.py` だけでなく registry renderer と latest-track verifier にも波及する。
PR lane でも hidden な branch 前提や bypass が残らないよう、
`Makefile.toml`、`apps/cli/src/commands/pr.rs` と `scripts/pr_review.py` / `scripts/pr_merge.py` の fail-closed 条件も guard 対象に含める。

### T004

`design.md` を canonical artifact にしたうえで、
`plan-only` / `activate` / `status` / registry / setup / onboarding docs に加え、
`full-cycle` / `ci` / `merge` / `revert` / router / backing skill / workflow docs / external guide context loader も UX-first に揃える。
`done.md` のような closeout surface も、remaining active track が branch-null のときに
誤って implementation command を案内しないよう対象に含める。
hook / guard policy message のように low-level wrapper を案内する surface も、
public path としては `/track:activate` を優先する前提で同期対象に含める。
`catchup.md` のような newcomer entrypoint や `revert.md` / external guide context loader のような
latest-track fallback を持つ補助 surface も、旧 latest-active fallback を残したままにしない。
primary surface である `plan.md` / `plan-only.md` / `activate.md` / `implement.md` /
`status.md` / `commit.md` / `track/workflow.md` / `DEVELOPER_AI_WORKFLOW.md` も
manual verification の対象に含める。

### T005

schema / activation / router / registry / docs の regression test を揃える。
branch-null planning-only track と materialized `planned` track の両方を fixture として扱う。
同一 fixture を Python validator と Rust render / validate path の両方へ通し、accept/reject parity だけでなく rendered guidance parity も固定する。
planning-only artifact を review / commit してから activation する手順と、
mixed-state repo で pre-activation review / commit / PR が explicit `track-id` selector で
正しい planning-only track を選べること、
planning-only allowlist 外の diff が pre-activation lane で reject されること、
PR wrapper / script が planning-only lane で hidden `track/<id>` 前提を要求しないこと、
activation preflight が stale/divergent branch を metadata 永続化前に止めることと、
永続化後の partial failure から `/track:activate <track-id>` 再実行で resume できること、
activation 後に source branch へ戻っても materialized state が見えることを確認する。
materialized active track と branch-null planning-only track が混在する fixture も持ち、
`status` / `catchup` / `revert` / external guide context loading / current focus / latest-track verifier が materialized active track を優先することを固定する。

## Out of Scope

今回の MVP では次を扱わない。

- same-track multi-worktree automation
- worker-branch model
- PR merge / branch delete automation の新規導入・全面再設計
- branch-per-track strategy 全体の再設計

## Source Migration Note

元の検討メモ:

- `tmp/track-plan-activation-design-2026-03-13/feature-branch-strategy-design.md`
- `tmp/track-plan-activation-design-2026-03-13/track-plan-branch-decoupling-2026-03-13.md`
- `tmp/track-plan-activation-design-2026-03-13/track-guidance-restructure-2026-03-13.md`

これらは historical source notes として残してよいが、
このトラックの実装判断に必要な内容は本 `design.md` に移したものとして扱う。
