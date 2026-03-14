# Spec: track:plan plan-only / activate 導線

## Goal

`/track:plan` と branch 自動作成・切替を切り分け、
planning-only track を先に main へ commit できるようにする。
そのうえで実装開始は activation という別フェーズに分け、
利用者には `plan-only` と `activate` を含む分かりやすい導線として見せる。
内部の wrapper や flags ではなく、明示的な command を正規入口にする。

## Scope

- `schema_version: 3` で `branch: null` を許可できる条件を整理する
- `track-branch-create` / `track-branch-switch` を、branch を作るだけでなく `metadata.json.branch` の確定まで含む動作に見直す
- activation 前に implementation 系の入口へ進めないようにする
- 利用者向け command として `/track:plan-only <feature>` / `/track:activate <track-id>` を追加する
- それに合わせて command docs と workflow docs を更新する
- `.claude/commands/track/full-cycle.md`、`.claude/commands/track/ci.md`、`.claude/commands/track/review.md`、`.claude/commands/track/pr-review.md`、`.claude/commands/track/merge.md`、`.claude/commands/track/revert.md`、`.claude/commands/track/catchup.md`、`.claude/commands/track/done.md`、`.claude/hooks/agent-router.py`、`.claude/hooks/block-direct-git-ops.py`、`.claude/skills/track-plan/SKILL.md`、`.claude/rules/07-dev-environment.md`、`.claude/docs/WORKFLOW.md`、`libs/domain/src/guard/policy.rs` を含む live routing / guidance surface の整合
- `Makefile.toml`、`scripts/pr_review.py`、`scripts/pr_merge.py`、`apps/cli/src/commands/pr.rs`、`apps/cli/src/commands/track.rs`、`apps/cli/src/commands/git.rs`、`scripts/git_ops.py`、`scripts/track_branch_guard.py` を含む actual executor / guard surface が、planning-only の explicit-selector lane と activation guard に追従すること
- `/track:status` を current phase と次の一手を返す router 寄りの導線へ寄せる
- `track/registry.md` の Current Focus / Active Tracks と header/footer copy が branch-null の planning-only track に対して `/track:activate <track-id>` を返し、materialized な `planned` track とは区別できるようにする
- non-track branch 上の current-track / current-focus / latest-track resolution が、branch-null planning-only track と materialized active track の混在時にも standard lane を壊さないようにする
- `scripts/external_guides.py` の latest-track context loading を含む補助コンテキスト解決が、branch-null planning-only track に引きずられて誤った current work を注入しないようにする
- planning-only artifact を activation 前に explicit `track-id` selector 付き review / commit / 必要に応じて PR で main 側へ landing する正式導線を定義する
- onboarding / setup の入口 docs (`START_HERE_HUMAN.md`, `.claude/commands/track/setup.md`) を新導線へ同期する
- `track/items/track-plan-activation-2026-03-14/design.md` への canonical design 統合
- schema / wrapper / guardrail / docs の regression test 追加

## Non-Goals

- 同一 track を複数 branch に分ける worker-branch モデルの導入
- same-track parallel worker の worktree 自動管理
- PR merge や branch delete automation の新規導入・全面再設計
- 既存 branch-per-track モデル全体の再設計
- `tmp/TODO.md` の `STRAT-04` として整理されている `track/registry.md` の非 Git 化 / 完全生成ビュー化
- `STRAT-04` に連動する repo-wide generated view policy (`STRAT-06`) の再設計
- Python helper 群への新規投資や、parity のための深追い。Python は移行完了までの互換層とし、このトラックでは Rust 側の正しい動作を支える以上の保守は目的にしない
- `takt` / `/track:full-cycle` の延命や UX 改善。残っている間に activation guard をすり抜けないことだけを保証し、それ以上の改善は扱わない

## Constraints

- 既存の standard lane（`/track:plan <feature>` → branch 作成 → implementation）は維持する
- planning-only track 以外では branch guard の fail-closed 契約を弱めない
- `metadata.json` を SSoT とする track workflow は維持する
- implementation/review/commit の current track 解決は materialized branch を前提に維持する
- branch-null planning-only track が新しく作られても、既存の materialized active track を non-track branch 上の current focus から追い出して standard lane を壊してはいけない
- `verify-latest-track` など latest-track 系の検証も、branch-null planning-only track によって誤った current work を選んではいけない
- `scripts/external_guides.py` など latest-track fallback で補助コンテキストを読む経路も、branch-null planning-only track によって materialized active track より優先されてはいけない
- activation は `metadata.json.branch` を track branch 上だけに閉じ込めず、non-track branch の current-focus / status / registry からも観測できる永続化状態として扱う
- pre-activation planning-only flow で許可する diff は `track/items/<id>/` 配下・`track/registry.md`・`track/tech-stack.md`・`.claude/docs/DESIGN.md` の allowlist に限定し、それ以外は implementation-phase とみなして reject する
- pre-activation lane では `/track:review` / `/track:commit` / `/track:pr-review` は explicit `track-id` selector を必須にし、`/track:merge` は PR 番号を canonical selector として empty-args auto-detect に依存しない
- `track/registry.md` はこのトラックでは git-tracked な generated view のまま扱い、必要最小限の copy / routing / rendering 変更だけに留める
- registry の repo-wide 運用方針変更や non-git read model 化は follow-up (`STRAT-04`) 側で扱い、このトラックではその前提を先取りしない
- `/track:plan-only` / `/track:activate` を live user-facing docs や registry copy に前倒しで出すのは、それらの command surface と onboarding docs が実装された段階に限る
- user-facing docs では low-level wrapper 名よりも phase 名と primary command を優先する
- 実装上 wrapper や alias を使ってもよいが、利用者向け導線では `/track:plan-only` と `/track:activate` を正規入口にする
- branch-null の planning-only track と materialized な `planned` track は routing 上で明確に区別する
- `takt` は削除前提であり、このトラックは `/track:full-cycle` の存廃を決めない。ただし command が repo に残っている間は activation guard を bypass させない
- `track/tech-stack.md` に未解決の作業メモ marker がない状態を維持する

## Canonical Design

- `track/items/track-plan-activation-2026-03-14/design.md`
- `track/archive/branch-strategy-2026-03-12/spec.md`
- `track/archive/track-branch-guard-2026-03-12/spec.md`

## Acceptance Criteria

- [ ] `schema_version: 3` の active track では、planning-only 条件 (`status=planned` かつ task が未着手) のときだけ `branch: null` を許可し、それ以外の active な branch なし状態は reject する。archived track の `branch: null` はこのトラックの scope 外であり、既存の挙動を変更しない
- [ ] Python validator (`scripts/track_schema.py` とその test) と Rust validator / render path が、同じ fixture に対して同じ planning-only 条件を受け入れ、`track views validate` と `cargo make ci` がそろって通る
- [ ] `cargo make track-branch-create '<id>'` と `cargo make track-branch-switch '<id>'` は、branch を作るだけでなく `metadata.json.branch` の確定まで行う internal path として整理され、activation の土台になる
- [ ] `/track:plan-only <feature>` と `/track:activate <track-id>` が利用者向けの command として定義され、docs の primary path に現れる
- [ ] `/track:plan-only <feature>` で作った planning-only artifact は、activation 前でも `/track:ci`、明示的な `track-id` 付き `/track:review`、`/track:commit <message>`、必要に応じて planning-only PR flow を通じて main に載せられる
- [ ] activation 前の selector ルールが command ごとに明確である。`/track:review`、`/track:commit`、`/track:pr-review` は明示的な `track-id` を必須とし、`/track:merge` は PR 番号を必須とする。`/track:ci` は diff-only / track-agnostic であり、track selector を必要としない（worktree の diff を検証するだけ）。current-branch の auto-detect には戻らない
- [ ] activation 前の planning-only review / commit / PR lane は docs 上の約束だけで終わらず、実際の executor にも反映される。`apps/cli/src/commands/git.rs`、`scripts/git_ops.py`、`scripts/track_branch_guard.py` を含む guarded commit path が、non-track branch 上でも対象の planning-only track を正しく扱える
- [ ] `Makefile.toml`、`scripts/pr_review.py`、`scripts/pr_merge.py`、`apps/cli/src/commands/pr.rs` は、planning-only artifact の PR lane を hidden な `track/<id>` 前提で塞がない。explicit selector と allowlist を伴う正式導線として扱うか、少なくとも public docs と同じ fail-closed 条件を返す
- [ ] `/track:activate` は planning-only (`branch=null`, `status=planned`, task 未着手) 以外の track を reject し、すでに activate 済みか無効な状態かを分かる形で案内する
- [ ] branch-null track は activation 前に、implementation-phase の遷移、`/track:implement`、リポジトリ内に残っている間の `/track:full-cycle`、コードを含む review / commit / PR に進めない。一方で planning artifact だけを扱う review / commit / PR は main に載せるために許可し、許可 diff は `track/items/<id>/` 配下、`track/registry.md`、`track/tech-stack.md`、`.claude/docs/DESIGN.md` に限定する
- [ ] `/track:status` は current phase、phase 判定理由、recommended next command、blocker の 4 項目を返す。branch-null の planning-only track に対しては `Ready to Activate` phase と `/track:activate <track-id>` を返し、materialized な `planned` track では実装導線を維持する
- [ ] `track/registry.md` の `Current Focus` と `Active Tracks` も、branch-null の planning-only track に対して `/track:activate <track-id>` を返し、materialized な `planned` track では従来どおり implementation command を返す
- [ ] `track/registry.md` の header/footer copy も `plan-only` / `activate` 導線と矛盾せず、旧 `/track:plan <feature>` だけを唯一の入口として残さない
- [ ] `track/registry.md` への変更は、git-tracked な generated view を前提にした最小限の routing / copy / rendering update に留め、non-git read model 化や公開経路の再設計はこのトラックへ持ち込まない
- [ ] activation は `metadata.json.branch` を track branch 上だけに書いて終わりにせず、source branch に戻ったあとも current-focus / status / registry / external-guide context loading から materialized state を観測できる
- [ ] `/track:activate` は target branch の preflight を先に行い、`track/<id>` が stale / divergent なら metadata 永続化前に fail する。metadata 永続化後に checkout だけ失敗した場合は、`/track:activate <track-id>` を再実行して再開できる
- [ ] `/track:activate` は clean worktree を要求し、未コミットの変更がある source branch 上では activation commit の作成前に reject する。エラーメッセージは worktree を clean にする方法を案内する
- [ ] branch-null planning-only track と materialized active track が同時にある場合、non-track branch 上の `status` / `catchup` / `revert` / external-guide context loading / current-focus / latest-track 解決は materialized active track を優先し、plan-only lane が既存 standard lane を乗っ取らない
- [ ] docs は standard lane と plan-only lane を区別し、wrapper 名や `--plan-only` を primary path として前面に出しすぎない
- [ ] 入口 docs（`plan.md`、`plan-only.md`、`activate.md`、`status.md`、`setup.md`、`START_HERE_HUMAN.md`、`track/workflow.md`、`DEVELOPER_AI_WORKFLOW.md`）、互換 surface（`full-cycle.md`、`agent-router.py`、`block-direct-git-ops.py`、`SKILL.md`、`07-dev-environment.md`、`WORKFLOW.md`、`policy.rs`）、downstream command docs（`implement.md`、`review.md`、`commit.md`、`ci.md`、`pr-review.md`、`merge.md`、`revert.md`、`catchup.md`、`done.md`）、を一括で新しい phase model と selector ルールに合わせ、activation guard をすり抜けず、旧導線と矛盾した案内を出さない
- [ ] Python/Rust parity は、branch-null state の accept / reject だけでなく、registry / current-focus / next-command など rendered guidance の一致まで固定する
- [ ] `design.md` が、branch-null 条件、phase モデル、public / internal command の境界を含む設計の正本として成立し、実装判断に `tmp/` を必須としない
- [ ] schema / wrapper / docs / guardrail の回帰テストが追加され、`cargo make ci` が通る
