---
adr_id: 2026-04-22-1432-branch-create-commit-ordering
decisions:
  - id: 2026-04-22-1432-branch-create-commit-ordering_grandfathered
    status: accepted
    grandfathered: true
---
# sotp track branch create: main 上の activation commit regression 修正

## Context

`sotp track branch create <id>` (`cargo make track-branch-create` wrapper 経由でも同じ) が、本来 track ブランチ上で作成されるべき activation commit を main ブランチ上で作成し、その後 track ブランチを派生させるため、main が 1 commit ahead になる regression が発生している。

根本原因は `execute_branch` と `execute_activate` が同一 path を共有していること。branch create は新規 track の branch 立て、activate は既存 planning-only track の materialize という別責務なのに、同じ path を通すため create path でも activate の副作用 (main 上 activation commit) が発生する。

`apps/cli/src/commands/track/activate.rs::execute_activate` 内の実行順序:

1. metadata.json materialize
2. `persist_activation_commit` → main 上で git commit を生成
3. `activation_git_commands` (`switch -c track/<id> main`) → 既に commit 済の HEAD を track に fork

この順序で、main にも track にも同一 commit が残り、main は 1 commit ahead となる。PR merge 時は merge commit が吸収するため silent regression になり、気付きにくい。

## Decision

### D1: execute_branch (BranchAction::Create) を独立 path に分離する

`apps/cli/src/commands/track/activate.rs::execute_branch(BranchAction::Create)` を `execute_activate(BranchMode::Create)` への forward から切り離し、**初回実装 (9c08f2a) 同等の単純な `git switch -c track/<id> main` path に戻す**。

責務:

- `sotp track branch create <id>` = **新規 track の branch 立て** (新 branch 作成 + switch のみ、commit は作らない)
- `sotp track activate <id>` = **既存 planning-only track の materialize** (metadata update + activation commit + branch switch、現行 path 保持)

この 2 つは根本的に責務が異なるため code path も分離する。

### D2: BranchMode::Create を execute_activate から退役

D1 の経路分離に伴い、`execute_activate` は `BranchMode::Switch` / `BranchMode::Auto` のみ受け付けるようにする。`BranchMode::Create` を execute_activate 経由で起動する path は存在しない。

実装選択肢:

- enum variant 自体を削除 (BranchMode を Switch / Auto の 2 つに)
- variant は残しつつ execute_activate で deny (Err 返し — `unreachable!` はパニックを起こすため CN-01 により禁止)

v4-only 化 (e76c8d3) の意図は維持。legacy path を復活させるわけではなく、**create と activate の責務分離を復活させる**が目的。

### D3: branch create の責務範囲 = branch 作成 + switch のみ

`sotp track branch create <id>` の責務は「`track/<id>` branch を main から新規作成し、その branch に switch する」のみとする。metadata.json の persist / activation commit / rendered view の sync 等は本 command の責務範囲外で、/track:init など呼び出し側が必要な後続 step (metadata update + commit) を track ブランチ上で実行する。

これにより「branch 作成」と「metadata 永続化」が分離され、main 上に commit が生える可能性を構造的に排除する。

## Rejected Alternatives

### A. execute_activate 内で BranchMode::Create のみ順序を switch→commit に変更

`execute_branch` は `execute_activate(BranchMode::Create)` への forward を維持しつつ、activate 内部で BranchMode::Create のときだけ branch switch を先行させる minimal 修正案。

**却下理由**: regression の表層 (commit 順序) は直るが、本質的問題 (create と activate が同 path を共有し責務が混在している) が残る。e76c8d3 の legacy retire で露呈した設計ひずみを放置する形になり、将来同種の regression が再発するリスクが高い。D1-D3 の責務分離が妥当。

## Consequences

### Positive

- main 上に activation commit が生える regression が構造的に排除される
- `sotp track branch create` と `sotp track activate` の責務境界が明確になり、将来の実装議論で「branch create は activate の一部なのか」という曖昧さが消える
- create path が単純な `git switch -c` のみになり、理解・変更コストが下がる
- `/track:init` が metadata persist を track ブランチ上で行うという設計と整合する (CN-13 共通構造パターン)

### Negative

- `BranchMode` enum から `Create` variant を退役する際、内部の rstest など複数箇所で caller の update が必要 (CLI interface 自体は不変、external には見えない)
- `execute_branch` に独立実装を復活させるため、activate.rs の module 構成が少し嵩む (責務分離の trade-off として許容)

## Reassess When

- OS-08 (ADR 2026-04-22-0829 で deferred とされた execute_branch / execute_activate 上位 refactor track) が実行されたとき
- branch create に加えて新しい branch 操作 (e.g. plan-only branch 向け switch) が追加されたとき
- v4-only schema 以外の schema version が再導入されるとき

## Related

- `knowledge/adr/README.md` — ADR 索引
- `knowledge/adr/2026-04-22-0829-plan-command-structural-refinements.md` — D3 (/track:init の branch create 責務) / D5 (preflight 緩和) と OS-08 (上位 refactor deferred)。本 ADR はそれらを補完する。
- `knowledge/conventions/pre-track-adr-authoring.md` — ADR 先行 authoring 原則
- `knowledge/conventions/workflow-ceremony-minimization.md` — post-hoc review / 事前承認限定
