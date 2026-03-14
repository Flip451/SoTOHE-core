# Design: track:plan plan-only / activate 導線

## Purpose

この文書は、このトラックの設計の正本である。
`tmp/track-plan-activation-design-2026-03-13/` の検討メモはここへ吸収し、
実装判断は `design.md` と `spec.md` を基準に行う。

また、`tmp/TODO.md` の `STRAT-04` (`track/registry.md` を Git 管理から外す案) は
このトラックでは扱わない。ここでは registry を今までどおり Git 管理された生成物として扱い、
plan-only / activate 導線に必要な最小限の変更だけを入れる。

## UX Decision

使い方の分かりやすさを優先する。
内部では既存 wrapper や `--plan-only` を使ってよくても、
利用者が覚える入口は明示的な command にそろえる。
ただし、その案内を docs や registry に出すのは、実装がそろってからにする。

採用する利用者向け command は次の 2 つ。

- `/track:plan-only <feature>`
- `/track:activate <track-id>`

非採用:

- `--plan-only` を利用者向けの正規入口にすること
- `cargo make track-branch-create '<id>'` を利用者向けの正規入口にすること

これらの低レベル command は内部実装や terminal 用として残してよいが、
通常の docs や AI の案内では前面に出さない。

## Closure Policy

このトラックは収束を優先する。
正とする挙動は Rust 実装と、そこから作られる metadata / rendered view に置く。
Python helper 群は移行完了までの互換レイヤとして扱う。

したがって、このトラックの review / fix loop では次を徹底する。

- Python に新しい機能を足さない
- Python を触るのは、必須の検証が壊れるとき、利用者向けの契約と食い違うとき、クラッシュするときだけに限る
- `takt` / `/track:full-cycle` は互換性のために guard だけ維持し、使い勝手の改善対象にはしない
- 構造改善は follow-up の Rust runtime consolidation track に送る

## Remaining Slices

残りは次の順で進める。

1. `T003`: guard / executor surface（`track.rs` 一本化）
   `implement` / `transition` / guarded review-commit-PR の fail-closed、明示的な指定、allowlist。
   PR の導線、guarded git executor、エラーメッセージ/guidance、
   resolver 結果の `status` / `catchup` / `revert` CLI wiring、activation transaction 安全要件もここに含める
2. `T004`: resolver + docs + registry + compatibility の一括更新
   resolver code（Python scripts + `render.rs` + `track_registry.py`）、`track/registry.md` の regeneration、
   入口 docs + downstream command docs + 互換 surface をまとめて phase model に合わせる。
   resolver と docs を同時に更新することで sequencing mismatch を防ぐ
3. `T005`: regression tests
   上記 2 面をまたぐ fixture と受け入れ条件の固定

`takt` / `/track:full-cycle` と Python helper は、各 slice で「壊さないための最小修正」に留める。

### Slice Boundary Rules

1. **T002 (done) / T003 の activation 分割**: T002 は `/track:activate` の基本経路（command 定義、
   `metadata.json.branch` 永続化、branch create/switch）を提供済み。activation transaction の
   安全要件（clean-worktree check、stale/divergent preflight、source-branch-visible commit、
   partial failure resume）は T003 で追加する。T002 の done 判定は基本経路の完了を意味し、
   安全要件は T003 が完了するまで activation の契約は完成しない。
   この中間状態は track branch 上でのみ存在し、main にマージされる前に T003 が完了する。
   T002 が `track.rs` に入れた `/track:plan-only` / `/track:activate` の基本経路は完了済みであり、
   以後の `track.rs` 変更はすべて T003 の scope に属する。

2. **`apps/cli/src/commands/track.rs`**: T003 が going-forward の唯一の owner。
   guard / selector / activation transaction / resolver wiring をすべてここで扱う。

3. **Python の扱い**: T004 の Python 作業は「既存 resolver scripts の if 分岐追加」レベルの
   最小限の priority alignment に限定する。新規 module 追加やリファクタリングは scope 外。
   Closure Policy の「Python に新しい機能を足さない」に従う。

## User-Facing Workflow

### Standard lane

通常の開始導線は維持する。

1. `/track:plan <feature>`
2. `/track:implement`
3. `/track:review` または `/track:pr-review`
4. `/track:ci`
5. `/track:commit <message>`

### Plan-only lane

計画だけ先に固めて main へ commit したい場合の手順は次のとおり。

1. `/track:plan-only <feature>`
2. `/track:ci` で planning-only diff が allowlist と CI 要件を満たすことを確かめる
3. 明示的な `track-id` 付き `/track:review` で planning artifact を確認する
4. 明示的な `track-id` 付き `/track:commit <message>` で planning artifact を main へ載せる
5. 必要なら明示的な `track-id` 付き `/track:pr-review` で planning-only PR を扱う。hidden な `track/<id>` 前提に戻らないよう、wrapper / script も含めて PR の導線を成立させる。merge は `/track:merge <pr>` を使い、指定子は track-id ではなく PR 番号に固定する
6. `/track:activate <track-id>`
7. `/track:implement`
8. `/track:review` または `/track:pr-review`
9. `/track:ci`
10. `/track:commit <message>`

activation 前の `/track:review` / `/track:commit` / PR を全面禁止にはしない。
止めるのは implementation-phase の code-bearing flow だけであり、
planning artifact だけを扱う review / commit / PR は正式な手順として許可する。
ただし non-track branch では自動判定が branch 確定済みの active track を優先するため、
activation 前の planning-artifact flow では明示的な `track-id` を必須にする。
また、planning-only とみなす diff は固定 allowlist に限る。
MVP では `track/items/<id>/` 配下、`track/registry.md`、`track/tech-stack.md`、
`.claude/docs/DESIGN.md` だけを許可し、それ以外の差分を含む場合は implementation-phase の作業として reject する。

### Transitional compatibility lane

`/track:full-cycle <task>` はいまも repo に残っている入口なので、
takt 廃止または command 再定義が終わるまでは互換 surface として扱う。

ただし、これを primary onboarding path にはしない。
残っている間は `/track:implement` と同じ activation guard を受け、
branch-null の planning-only track を bypass できないことだけを求める。

### Resume lane

再開の入口は `/track:status` に集約する。

`/track:status` は候補を並べるのではなく、phase に応じて
「次に叩く command を 1 つだけ」返す振り分け役として扱う。

同じ方針を `track/registry.md` の `Current Focus` と `Active Tracks` にも適用する。
planning-only track が最新 active track のときは、推奨する次の command を
`/track:implement` ではなく `/track:activate <track-id>` にする。
一方で、branch が materialize 済みの `planned` track は、これまでどおり implementation 導線に進めてよい。
registry の header/footer copy も同じ方針にそろえる。
「`/track:plan <feature>` だけが唯一の入口」と読める固定文言は残さない。

ただし registry 自体は、このトラックでは過渡期の read model のまま扱う。
ここでやるのは next-command recommendation と phase copy の整合までであり、
`STRAT-04` が扱う「Git 管理対象から外す」「artifact / Pages / PR comment へ逃がす」といった
repo 全体の公開戦略の変更までは含めない。

### Non-track branch resolution

branch-null の planning-only track を main 上へ commit できるようにしても、
non-track branch 上の current focus が、それだけで branchless track に切り替わってはいけない。

期待する優先順位:

1. 現在 branch が `track/<id>` ならその track
2. そうでなければ、materialized な active track のうち最新のもの
3. materialized active track が無い場合に限り、branch-null planning-only track のうち最新のもの

これにより、`/track:status` や `/track:catchup` は既存の standard lane を保ったまま、
materialized active track がない repo だけで `Ready to Activate` を前面に出せる。
同じ優先順位は `track/registry.md` の `Current Focus` や `verify-latest-track` 相当の
latest-track verifier だけでなく、`scripts/external_guides.py` のように最新トラック文脈を
補助コンテキストとして読む loader にも適用する。
secondary surface の `/track:revert` も同じ current-track 解決規則に従う。

この優先順位を成り立たせるには、activation が non-track branch からも見えなければならない。
そのため、`metadata.json.branch` を新しい `track/<id>` branch の上でだけ更新して終わりにする案は採らない。

## Workflow Phases

利用者向けには branch の有無ではなく phase 名で説明する。

- `Planning`
- `Ready to Activate`
- `In Progress`
- `Ready to Ship`
- `Done`

この phase は利用者向けの概念であり、`metadata.status` と 1:1 で一致する必要はない。

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
takt 廃止トラックとの整合が取れるまでは互換 surface として扱う。
このトラックの責務は、残っている間の guard と routing の整合を保つことであり、
恒久的な主導線として再定義することではない。

### Low-level / internal commands

- `cargo make track-branch-create '<id>'`
- `cargo make track-branch-switch '<id>'`
- `cargo make track-transition ...`
- `cargo make track-sync-views`

## Technical Model

### Planning-only track

planning-only track は次の条件を満たすときだけ許可する。

- `schema_version == 3`
- `status == "planned"`
- `branch == null`
- task が空、または全 task が `todo`

これ以外の active な branch なし状態は reject する。
archived track の `branch=null` はこのトラックの scope 外であり、既存の挙動を変更しない。

重要:

- Python validator だけでなく Rust validator / render path も同じ条件を受け入れる必要がある
- `track views validate` と `cargo make ci` が通るところまで含めて完成とする

この区別は registry / status routing にも効く。
branch-null の planning-only track だけが `Ready to Activate` と `/track:activate` を返し、
branch が materialize 済みの `planned` track は既存どおり implementation 導線を返す。

### Activation

activation は branch create/switch だけでは足りず、
`metadata.json.branch` の確定まで含む。
さらに、その確定済み状態は source branch (`main` など) に戻っても見えなければならない。

期待挙動:

1. target track の `metadata.json` を読む
2. `track/<track-id>` を create/switch できるか、既存 branch を安全に再利用できるかを preflight で確認する
3. source branch 上で `metadata.json.branch = "track/<track-id>"` を書き込み、rendered views を sync する
4. その状態を、source branch からも見える永続化済みの状態として確定する
5. その確定済み状態から `track/<track-id>` branch を create または switch する
6. 以後の implementation / review / commit は、通常どおり branch guard の保護下に入る

MVP では step 3 を activation commit として扱うのがいちばん単純である。
つまり `/track:activate` は clean worktree を要求し、source branch 上で activation commit を作ってから
`track/<id>` へ移る。
これにより、track branch と source branch の両方から同じ metadata が見える。

fail-closed 条件:

- target track が `branch=null`, `status=planned`, task 未着手の planning-only track でない場合は reject
- `track/<id>` が stale/divergent で安全に再利用できない場合は、metadata 永続化前に reject する
- invalid state を mutate せず、次に取るべき action を返す
- clean worktree でない source branch 上から activation しようとした場合は、永続化手順を安全に実行できないため reject する

recovery:

- metadata 永続化までは preflight failure で fail-fast し、source branch を汚さない
- metadata 永続化後に checkout だけ失敗した場合は、`/track:activate <track-id>` を再実行して branch switch を resume できるようにする

### Public vs Internal Activation Path

利用者向け:

- `/track:activate <track-id>`

internal:

- `cargo make track-branch-create '<id>'`
- `cargo make track-branch-switch '<id>'`

内部的には `/track:activate` が上記 wrapper を使ってもよいし、
CLI subcommand を直接叩いてもよい。
ただし docs と AI の案内では `/track:activate` を正規の入口とする。

## Executor Coverage

planning-only の導線は command docs だけ整えても成立しない。
実際に branch 前提や current-track auto-detect を持つ executor まで scope に含める。

local planning-only commit 導線:

- `.claude/commands/track/review.md`
- `.claude/commands/track/commit.md`
- `apps/cli/src/commands/git.rs`
- `scripts/git_ops.py`
- `scripts/track_branch_guard.py`

この lane では明示的な `track-id` が docs 上の慣習に留まらず、
guarded commit path まで渡ることが必要である。
MVP では既存の `track-dir` plumbing を活かし、non-track branch 上でも
planning-only artifact だけを対象にした review / commit が成立するようにする。

planning-only PR 導線:

- `.claude/commands/track/pr-review.md`
- `.claude/commands/track/merge.md`
- `Makefile.toml`
- `apps/cli/src/commands/pr.rs`
- `scripts/pr_review.py`
- `scripts/pr_merge.py`

ここは現状 `track/<id>` branch 前提が強い。
このトラックでは secondary command として残す以上、public docs だけ先に書き換えるのではなく、
executor も明示的な指定と planning-only allowlist に追従させる。
少なくとも hidden な `track/<id>` 前提のまま、正規の入口が成功するようには見せない。
指定の規則も分ける。`/track:pr-review` は明示的な `track-id` を必須とし、
`/track:merge` は PR 番号を正規の指定子とする。non-track branch 上で
empty-args current-branch auto-detect に戻らないようにする。

activation / status の振り分け導線:

- `.claude/commands/track/status.md`
- `apps/cli/src/commands/track.rs`

activation と status は利用者向けの phase 振り分け役なので、
branch の確定や next-command recommendation が docs と renderer だけで一致していても足りない。
実際の CLI entrypoint まで同じ phase model を採用する。

## Guardrails

branch-null の planning-only track では planning は許可するが、
implementation-phase の入口は fail-closed にする。

許可:

- planning artifact の更新
- task 追加
- rendered view の再生成
- planning artifact だけを対象にした `/track:ci`
- explicit `track-id` selector 付きで planning artifact だけを対象とした `/track:review`
- explicit `track-id` selector 付きで planning artifact だけを対象とした `/track:commit`
- explicit `track-id` selector 付きで planning artifact だけを対象とした PR 作成・レビュー
- explicit PR 番号付き `/track:merge <pr>` による planning-only PR のマージ
- 上記 allowlist に収まる planning-only diff

拒否:

- `todo -> in_progress`
- `done` / `skipped` への遷移
- `/track:implement`
- repo に残っている間の `/track:full-cycle`
- code-bearing review / commit / PR 系の implementation-phase flow

期待するエラーは、単に失敗するだけでなく activation 導線を案内すること。

例:

```text
track is not activated yet; run /track:activate <track-id>
```

## Status Command Responsibilities

`/track:status` は summary だけでなく workflow の振り分け役でもある。

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

docs では branch / null / materialization といった内部用語を前面に出しすぎない。

利用者向けには次の用語を優先する。

- planning-only track
- activated track
- ready to activate

内部仕様・validator・guardrail の節だけ branch/null を使う。

入口 docs も例外ではない。以下はすべて UX-first の導線に同期する対象であり、
T004 で一括して扱う。

T004（resolver + docs + registry + compatibility）:

- `scripts/external_guides.py`
- `scripts/verify_latest_track_files.py`
- `scripts/track_resolution.py`
- `scripts/track_registry.py`
- `libs/infrastructure/src/track/render.rs`
- `track/registry.md`（regeneration diff）
- `.claude/commands/track/plan.md`
- `.claude/commands/track/plan-only.md`
- `.claude/commands/track/activate.md`
- `.claude/commands/track/status.md`
- `.claude/commands/track/full-cycle.md`
- `.claude/commands/track/setup.md`
- `.claude/commands/track/implement.md`
- `.claude/commands/track/review.md`
- `.claude/commands/track/commit.md`
- `.claude/commands/track/ci.md`
- `.claude/commands/track/pr-review.md`
- `.claude/commands/track/merge.md`
- `.claude/commands/track/revert.md`
- `.claude/commands/track/catchup.md`
- `.claude/commands/track/done.md`
- `START_HERE_HUMAN.md`
- `DEVELOPER_AI_WORKFLOW.md`
- `track/workflow.md`
- `.claude/docs/WORKFLOW.md`
- `.claude/hooks/agent-router.py`
- `.claude/hooks/block-direct-git-ops.py`
- `.claude/skills/track-plan/SKILL.md`
- `.claude/rules/07-dev-environment.md`
- `libs/domain/src/guard/policy.rs`

新規利用者が最初に読む文書に、旧導線だけが残る状態は許容しない。

## Implementation Slices

### T001

planning-only schema を Python / Rust の両方で受け入れられるようにする。

### T002

planning-only entrypoint と activation の基本経路を実装する。
`/track:plan-only` は branch を自動作成しない planning artifact 作成 path を持ち、
`/track:activate` は `metadata.json.branch` の確定と branch create/switch の基本 path を提供する。
activation transaction の安全要件（clean-worktree check、stale/divergent preflight、
source-branch-visible 永続化、partial failure からの resume）は T003 で完成させる。

### T003

activation 前の implementation-phase の入口を fail-closed にする。
`/track:implement` と、repo に残っている間の `/track:full-cycle` を含めて guard 対象に入れる。
planning artifact だけを扱う review / commit / PR は明示的な指定つきで main に載せられるようにし、
逆にコードを含む review / commit / PR は activation 前に進めないようにする。
`apps/cli/src/commands/track.rs`、`Makefile.toml`、`apps/cli/src/commands/pr.rs`、
`scripts/pr_review.py`、`scripts/pr_merge.py`、`apps/cli/src/commands/git.rs`、
`scripts/git_ops.py`、`scripts/track_branch_guard.py` を含む executor / guard 実装はここで扱う。
これらの runtime surface が返すエラーメッセージと guidance（`/track:activate` への案内を含む）も
ここで整え、docs 側の T004 と矛盾しないようにする。
activation transaction の安全要件（dirty-worktree rejection、source-branch 上での activation commit semantics、
stale/divergent branch の preflight fail-fast、永続化後の partial failure からの resume）も
guard surface の一部としてここで実装する。
`apps/cli/src/commands/track.rs` のうち、
activation guard と review / commit / PR selector はここで持つ。
resolver 結果を `status` / `catchup` / `revert` の CLI path へ流し込む wiring もここで持つ。

### T004

resolver・docs・registry・compatibility surface を一括で整える。

shared resolver の優先順位（`scripts/external_guides.py`、`scripts/verify_latest_track_files.py`、
`scripts/track_resolution.py`）を既存の if 分岐レベルの最小修正でそろえ、
non-track branch では branch 確定済みの active track を planning-only track より優先する。
Current Focus ordering を持つ `scripts/track_registry.py` と `libs/infrastructure/src/track/render.rs` も
同じ priority rule に合わせ、`track/registry.md` の regeneration diff（next-command recommendation と
header/footer copy を含む）もここで扱う。

同時に入口 docs（`.claude/commands/track/plan.md`、新規 `plan-only.md` / `activate.md`、`status.md`、
`.claude/commands/track/setup.md`、`START_HERE_HUMAN.md`、`track/workflow.md`、
`DEVELOPER_AI_WORKFLOW.md`）、互換 surface（`.claude/commands/track/full-cycle.md`、
`.claude/hooks/agent-router.py`、`.claude/hooks/block-direct-git-ops.py`、
`.claude/skills/track-plan/SKILL.md`、`.claude/rules/07-dev-environment.md`、
`.claude/docs/WORKFLOW.md`、`libs/domain/src/guard/policy.rs`）、
downstream command docs（`implement.md` / `review.md` / `commit.md` / `ci.md` /
`pr-review.md` / `merge.md` / `revert.md` / `catchup.md` / `done.md`）を
新しい phase model に合わせて一括更新する。

resolver code と docs を同時に更新することで、user-facing guidance と
resolver priority の sequencing mismatch を防ぐ。
Python の新規 module 追加やリファクタリングは scope 外とし、
Closure Policy の「Python に新しい機能を足さない」に従う。
`takt` や `/track:full-cycle` を改善すること自体は目的にせず、
activation guard をすり抜けないことと、誤った案内を出さないことだけを保証する。

### T005

schema / activation / router / registry / docs の回帰テストをそろえる。
branch-null planning-only track と materialized `planned` track の両方を fixture として扱う。
同じ fixture を Python validator と Rust render / validate path の両方へ通し、accept / reject の一致だけでなく rendered guidance の一致も固定する。
planning-only artifact を review / commit してから activation する手順、
mixed-state repo で activation 前の review / commit / PR が明示的な `track-id` で
正しい planning-only track を選べること、
planning-only allowlist 外の diff が activation 前の導線で reject されること、
PR wrapper / script が planning-only 導線で hidden な `track/<id>` 前提を要求しないこと、
`/track:merge` が current-branch auto-detect に戻らず PR 番号だけを受け付けること、
activation preflight が stale / divergent branch を metadata 永続化前に止めること、
永続化後の partial failure から `/track:activate <track-id>` の再実行で resume できること、
activation 後に source branch へ戻っても materialized state が見えることを確認する。
materialized active track と branch-null planning-only track が混在する fixture も用意し、
`status` / `catchup` / `revert` / external guide context loading / current focus / latest-track verifier が materialized active track を優先することを固定する。
互換導線として残す `/track:full-cycle` が activation guard を bypass しないこともここで固定する。
最後に `cargo make ci` を通し、既存の branch-per-track workflow を壊していないことを確認する。

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

これらは履歴用の source notes として残してよいが、
このトラックの実装判断に必要な内容は本 `design.md` に移したものとして扱う。
