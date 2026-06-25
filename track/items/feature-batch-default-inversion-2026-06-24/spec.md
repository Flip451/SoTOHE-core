<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 29, yellow: 0, red: 0 }
---

# feature バッチ消化への既定反転 — per-layer 並列レビューを始動させる

## Goal

- [GO-01] `/track:full-cycle` の消化単位を per-task 直列から feature バッチへ反転する。1 feature を構成するタスク群を依存順に同一 working tree へ一括実装し、次タスクの追加でいずれかのレイヤーの累積差分が D3 の天井を超過する見込みになるか feature が完了するまで commit を挟まない。これにより review / CI / commit の round 分断を解消し、per-layer 並列レビューが典型的な multi-scope feature で既定経路からフル稼働する状態にする。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D1]
- [GO-02] バッチ実装後の差分に既存の `/track:review` を 1 回走らせる。差分が複数 scope（層）に跨る場合、scope 独立の並列レビューがその並列度をフルに発揮する。新しいレビュー機構は作らず、「まとめた差分を既存機構に渡す」形で機構への投資を回収する。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D2]
- [GO-03] review コスト天井を per-commit 全体ではなく per-layer-scope 単位で課し、その天井値を `.harness/config/review-scope.json` から注入する。各層が天井以下なら壁時計レビュー時間は 1 層分の O(天井²) で頭打ちになり、バッチ commit の総差分は層数倍まで許容される。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D3]
- [GO-04] バッチをまとめた 1 commit のハッシュをバッチ内の全タスクに `bin/sotp track transition <id> done --commit-hash <hash>` で記録する。per-task コミット無しでも各タスクの `commit_hash` フィールドにハッシュが入り、トレーサビリティが保たれる。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D4]

## Scope

### In Scope
- [IN-01] `/track:full-cycle` のループ構造を feature バッチ消化へ反転する。従来のタスク単位 implement → review → commit を、バッチ実装（依存順の全タスク一括）→ review → commit に置き換える。バッチ中は commit を挟まず、次タスクの追加でいずれかのレイヤーの累積差分が per-scope 天井を超過する見込みになった時点か feature 完了時点で commit を切る。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D1] [tasks: T005]
- [IN-02] `adr2pr.md:41` の Constraint 3（密結合時のみバッチを許可する例外）を既定反転する。バッチが既定になり、per-task 分割が例外（per-scope 天井超過が不可避な時点での分割）となる。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D1] [tasks: T006]
- [IN-03] バッチ実装後の差分に対して `/track:review` を 1 回呼ぶよう `/track:full-cycle` を変更する。review は既存の per-scope 並列機構をそのまま使い、新たなレビュー機構は追加しない。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D2] [tasks: T005]
- [IN-04] `/track:full-cycle` が `.harness/config/review-scope.json` の per-scope 天井設定を読み取り、次に実装するタスクを追加したときにいずれかのレイヤーの累積差分が天井を超過するか否かを判定するバッチ sizing ロジックを組み込む。天井未達なら継ぎ足しを継続し、天井超過が不可避になった時点で現バッチを commit して新バッチを開始する。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D3] [tasks: T005]
- [IN-05] `.harness/config/review-scope.json` に per-scope の diff 天井値フィールドを追加できるよう拡張する。グローバル既定（目安 ~500 行）を持ちつつ、scope（層）ごとに上書き可能とする。設定フォーマットは既存の per-group 設定に追記する形とし、新しい設定ファイルは作らない。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D3] [tasks: T002, T003, T004]
- [IN-06] バッチをまとめた 1 commit の hash をバッチ内の全タスクに `bin/sotp track transition <id> done --commit-hash <hash>` で記録する。1 バッチ = 1 commit であり、バッチ内の複数タスクは同一 hash を共有する。`TaskStatus::Done` が commit_hash を所有し、hash の unique 制約は無い（確認済み）。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D4] [tasks: T005]
- [IN-07] 変更面は `full-cycle.md` のループ構造、`adr2pr.md:41` の既定反転、`review-scope.json` からの天井設定読み出し、および D4 の hash 記録の 4 点のみとする。新スケジューラ・新スキーマ・新タグ（`layer` / `feature`）は作らない。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D5] [tasks: T002, T003, T004, T005, T006]

### Out of Scope
- [OS-01] 1 トラックに複数の独立 feature が同居する場合のバッチ境界 grouping（feature タグの導入等）は対象外（将来課題）。本トラックは単一 feature バッチのみを扱う。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D5]
- [OS-02] 新しいレビュー機構（feature 粒度でレビューを再キーする仕組み等）の新規実装は行わない。既存の `/track:review` および per-scope 並列機構をそのまま利用する。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D2]
- [OS-03] layer-wave スケジューラ、full task-DAG スケジューラ、worktree-per-task + branch + merge 等、Rejected Alternatives で廃案になった方式は実装しない。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D5]
- [OS-04] 実装が 1 タスクだけ、または元々 1 タスクで構成されている feature に対するバッチ利得の最大化は対象外。1 タスク feature ではバッチ化の利得は小さく、既存動作が維持されれば十分。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D1]
- [OS-05] DRY ゲートの有効・無効、DRY ゲートの実行回数変化は本 spec の acceptance の対象外。DRY は commit ごとに whole-corpus で走るゲートであり、batching は DRY の実行回数を減らすのみで、DRY の内部挙動に変化を加えない。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D5]

## Constraints
- [CN-01] per-scope 天井の判定対象は「次に実装するタスクを追加した場合に累積差分が天井を超過するか」とする。あるレイヤーが天井に達しても、控えているタスクが別レイヤーのみであれば、そのレイヤーの累積は増えないため継ぎ足しを続けてよい。天井超過が見込まれる場合のみ現バッチを commit してから次バッチを開始する。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D1, knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D3] [tasks: T005]
- [CN-02] review コスト天井は `.harness/config/review-scope.json` から注入する。コードに直接焼き込まない。グローバル既定を持ちつつ scope（層）ごとに上書き可能とし、プロジェクト・層ごとのチューニングはコード変更なしで設定ファイルの更新のみで行える。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D3] [tasks: T002, T003, T004]
- [CN-03] 1 バッチは 1 commit にまとめ、バッチ内の全タスクに同一 commit_hash を記録する（`bin/sotp track transition <id> done --commit-hash <hash>` を各タスクに適用）。`TaskStatus::Done` の hash unique 制約は無いことを前提とする。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D4] [conv: knowledge/conventions/track-lifecycle.md#状態遷移 API] [tasks: T005]
- [CN-04] 実装変更は既存コマンドの修正に閉じ、新スケジューラ・新スキーマ・`layer` / `feature` タグの追加は行わない。典型的な単一 feature では `/track:implement`、`/track:review`、`/track:commit` の既存コマンドをそのまま利用し、これらを呼ぶ `/track:full-cycle` のループ制御のみを変更する。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D5] [tasks: T005, T006]
- [CN-05] optimistic batching（未レビューの下層の上に上層を積む）による手戻りリスクを許容する。多くの feature では domain が先に固まるため許容範囲とする（ADR §Consequences §Negative 参照）。この constraint はバッチ実装の順序を「依存順（下層 → 上層）」に固定することを意味する。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D1] [tasks: T005]

## Acceptance Criteria
- [ ] [AC-01] `/track:full-cycle` は、1 feature のタスク群を依存順に実装したのち、commit を 1 回（またはレイヤー天井超過時の分割で最小回数）で切る。従来のタスク境界ごとの commit は発生しない。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D1] [tasks: T005]
- [ ] [AC-02] バッチ実装後の差分に対して `/track:review` が 1 回呼ばれ、複数 scope に跨る差分があるとき scope 独立の並列レビューが同時に走る。バッチ中に commit を挟んだことによるレビュー round 分断が発生しない。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D2] [tasks: T005]
- [ ] [AC-03] `/track:full-cycle` が `.harness/config/review-scope.json` の per-scope 天井値を読み取り、次タスクを追加した際にいずれかのレイヤーが天井を超過すると判定した時点で現バッチを commit してから次バッチを開始する。天井が未達の間は継ぎ足しを継続する。別レイヤーのタスクを追加しても天井超過済みレイヤーの累積差分は増えないため、別レイヤーへの継ぎ足しはブロックされない。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D3] [tasks: T005]
- [ ] [AC-04] `.harness/config/review-scope.json` に per-scope 天井値を設定するフィールドが追加されており、グローバル既定と scope ごとの上書きが設定ファイルの変更のみで行える。コード変更なしに天井値を調整できる。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D3] [tasks: T002, T003, T004]
- [ ] [AC-05] バッチ commit 後、バッチ内の全タスクに対して `bin/sotp track transition <id> done --commit-hash <hash>` が呼ばれ、同一 commit_hash が各タスクに記録される。`plan.md` / `metadata.json` で各タスクの `commit_hash` フィールドが埋まっていることで確認できる。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D4] [conv: knowledge/conventions/track-lifecycle.md#状態遷移 API] [tasks: T005]
- [ ] [AC-06] `adr2pr.md` の Constraint 3 がバッチ既定に書き換えられており、「per-task が既定 / 密結合時のみバッチ許可」という旧記述は存在しない。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D1] [tasks: T006]
- [ ] [AC-07] 変更は `full-cycle.md` のループ構造、`adr2pr.md:41` の既定反転、`review-scope.json` の天井フィールド追加、および D4 の同一 commit hash 記録経路の 4 点に閉じており、新スケジューラ・新スキーマ・`layer` / `feature` タグの追加が存在しない。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D5] [tasks: T002, T003, T004, T005, T006]
- [ ] [AC-08] 典型的な multi-scope feature（domain → usecase → infrastructure と層ごとに分割した 3 タスク等）を `/track:full-cycle` で実行した場合、review / CI / commit の total round 数が per-task 直列（N タスク = N review round）より少なくなる。 [adr: knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D1, knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md#D2] [tasks: T005, T006]

## Related Conventions (Required Reading)
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/track-lifecycle.md#状態遷移 API
- knowledge/conventions/pre-track-adr-authoring.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 29  🟡 0  🔴 0

