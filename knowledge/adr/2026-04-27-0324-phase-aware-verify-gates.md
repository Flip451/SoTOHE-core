# verify チェーンを file 存在ベースの phase 責務分離に揃える

## Context

`/track:init` 直後 (metadata.json + ADR ファイルのみ存在し、`spec.json` / `<layer>-types.json` / `impl-plan.json` / `task-coverage.json` / `spec.md` / `observations.md` は未作成) の Phase 0 状態で `cargo make ci` を実行すると、3 つの verify がブロックして commit に進めない。

- `verify-latest-track-local` — `spec.md` / `spec.json` と `plan.md` の両方を必須として要求 (`libs/infrastructure/src/verify/latest_track.rs:42`)
- `verify-view-freshness-local` — `plan.md` absent を FAIL (`libs/infrastructure/src/verify/view_freshness.rs:84-90`)
- `verify-catalogue-spec-refs-local` — `spec.json` absent を `CliError` で hard fail (`apps/cli/src/commands/verify_catalogue_spec_refs.rs:216`)

`ci-local` の他の verify はすでに `knowledge/conventions/workflow-ceremony-minimization.md` の Rules「file 存在 = phase 状態」原則に沿って、該当ファイルがなければ SKIP する設計になっている (`verify-plan-progress` / `verify-track-metadata` / `verify-track-registry` / `verify-plan-artifact-refs` / `verify-spec-states-current` / `check-catalogue-spec-signals` / `check-approved`)。上記 3 箇所だけ整合性が取れていない状態が現状の問題。

結果として、ユーザーが小さな単位で commit (例: 「ADR + Phase 0 init」を 1 commit、「Phase 1 spec」を別 commit) で段階的に進めたい場面で gate に阻まれ、Phase 0+1+2+3 を 1 つの大きな commit にまとめざるを得ない (実例: `type-designer-tuning-2026-04-25` トラック commit `6e76ef3`)。これは `.claude/rules/10-guardrails.md` の small task commit guideline (<500 行) および `track/workflow.md` 第 12 項「レビューサーフェース最小化」と矛盾する。

関連:

- `knowledge/conventions/workflow-ceremony-minimization.md` — Rules「file 存在 = phase 状態」+ Examples「`verify-latest-track-local` が `impl-plan.json` の存在を検出したときのみ task 項目をチェックする」(後者は意図表明だが現実装と乖離)
- `knowledge/conventions/pre-track-adr-authoring.md` — ADR を track 前段階で作る運用なので、ADR + Phase 0 init を別 commit にしたい動機が強い
- memory `project_phase_granular_commit_gate.md` — 2026-04-25 `type-designer-tuning` track での実観測

## Decision

### D1: verify チェーンを「file 存在 = phase 状態」原則に揃える

`knowledge/conventions/workflow-ceremony-minimization.md` の Rules「file 存在 = phase 状態」を verify チェーン全体に一貫適用する。各 verify は自身が検査対象とする artifact (`spec.json` / `<layer>-types.json` / `impl-plan.json` / `plan.md` / `task-coverage.json`) の存在を検出してから検査を実行し、不在ならば silent SKIP (warning でも error でもなく PASS) として扱う。phase の自動判定はこの artifact 存在チェックに統一し、`metadata.json` 上の追加マーカーや `phase_override` フィールドのような人工状態は導入しない。

### D2: 対象 3 verify の挙動を file 存在ベースに改修する

#### D2.1: `verify-latest-track-local`

`libs/infrastructure/src/verify/latest_track.rs:42` の `verify` 関数において、track が `impl-plan.json` を持たない (= Phase 0 / Phase 1 / Phase 2) 場合は `spec.md` / `spec.json` / `plan.md` の存在チェックを SKIP する。`impl-plan.json` の存在を検出した時点で従来どおり全 artifact の存在を要求する。これは convention Examples「`verify-latest-track-local` が `impl-plan.json` の存在を検出したときのみ task 項目をチェックする」と同じ条件分岐を spec/plan チェックにも拡張するものである。

#### D2.2: `verify-view-freshness-local`

`libs/infrastructure/src/verify/view_freshness.rs:84-90` の `plan.md` absent FAIL を、`libs/infrastructure/src/track/render.rs:621-624` の `validate_track_snapshots` と同じく `continue` (silent SKIP) に揃える。両者で `plan.md` absent への扱いが非対称な現状の不整合を解消する。

#### D2.3: `verify-catalogue-spec-refs-local`

`apps/cli/src/commands/verify_catalogue_spec_refs.rs:216` の `read_spec_element_hashes` が `spec.json` absent を `CliError` にしている挙動を改め、catalogue file (= `<layer>-types.json`) の存在を先に検出する分岐に変更する:

- catalogue absent → catalogue 検査自体が無意味なので silent PASS
- catalogue present + `spec.json` absent → 引き続き FAIL (catalogue が spec を ref しているのに spec が無い = SoT Chain ② 違反)
- catalogue present + `spec.json` present → 従来どおり ref integrity 検査

`spec.json` の存在は catalogue が存在する場合にのみ意味を持つ、という SoT Chain の上下関係を反映する。

### D3: 既存テストの分割と Phase 0 PASS 確認テスト追加

`apps/cli/src/commands/verify_catalogue_spec_refs.rs` の既存テスト `verify_fails_when_spec_missing` を 2 ケースに分割する:

- `verify_passes_when_catalogue_absent_and_spec_absent` (新規) — Phase 0 状態で PASS
- `verify_fails_when_catalogue_present_and_spec_absent` (改名) — SoT Chain ② 違反検出を残す

加えて `latest_track.rs` および `view_freshness.rs` についても、Phase 0 (`impl-plan.json` absent / `spec.json` absent / `plan.md` absent) で PASS することを確認する unit test を新規追加する。

### D4: スコープ境界

本決定は `cargo make ci` 内の 3 verify (`verify-latest-track-local` / `verify-view-freshness-local` / `verify-catalogue-spec-refs-local`) の挙動変更と関連 unit test の追加 / 分割のみをスコープとする。次の項目は明示的にスコープ外:

- 他の verify (`verify-plan-progress` / `verify-track-metadata` / `verify-track-registry` / `verify-plan-artifact-refs` / `verify-spec-states-current` / `check-catalogue-spec-signals` / `check-approved`) はすでに file 存在ベースに揃っており、本決定では触らない
- `/track:done` ゲート移行や PR-merge 時の strict 検査強化は別 ADR で扱う

## Rejected Alternatives

### A. `metadata.json` に `phase_override` / `intentionally_phase_X_only` マーカーを追加

**内容**: track の `metadata.json` に「この commit は意図的に Phase X only」と記す state field を導入し、verify はこのマーカーを参照して検査を SKIP する。

**却下理由**: `knowledge/conventions/workflow-ceremony-minimization.md` の Rules「人工的な状態フィールドを作らない」(`Status` / `approved` / `approved_at` と同類) に違反する。マーカーを消し忘れると永続的に verify が回避される anti-pattern を生む。本来 phase は artifact 存在で表現できる情報なので、追加 state は冗長。

### B. `cargo make ci` から該当 verify を外し `/track:done` (track 完了) に gate を移す

**内容**: 中間 commit 時には `verify-latest-track` / `verify-view-freshness` / `verify-catalogue-spec-refs` を流さず、track 完了時 (`/track:done` または PR merge gate) でまとめて検査する。

**却下理由**: 中間 commit で全 phase の整合性検査が落ち、Phase 1 / 2 / 3 で artifact 不整合が起こっても CI が検出しなくなる。phase 自動判定なら「該当 phase に到達した時点で検査が動く」のに対し、本案は「最後まで検査しない」ので fail-fast 性が失われる。影響範囲が広すぎる。

### C. `cargo make ci` を `ci-fast` / `ci-final` の 2 段階に分割

**内容**: 中間 commit 用に緩い `ci-fast` を新設し、最終確認用の `ci-final` は全 verify を流す。

**却下理由**: 検査体系が分裂し、開発者が「どちらを通せば commit できるか」を覚える運用負担が増える。`/track:commit` の guarded path との整合性も再設計が必要になる。一方 file 存在ベース判定なら同一 `cargo make ci` で全 phase に対応できる。

### D. 該当 3 verify を warning 格下げ (error → warn)

**内容**: `spec.md` absent / `plan.md` absent / `spec.json` absent を警告のみで通す。

**却下理由**: 短期的には Phase 0 commit が通るが、Phase 1 以降でも warning が出続けて長期的に「warning 無視」文化が定着し、整合性 gate そのものの意味が薄れる。Phase 1 / 2 / 3 でも spec/catalogue が無いと警告が出続けるのは正しい挙動でない。

### E. 環境変数 / コマンドフラグでの個別 skip (例: `SKIP_LATEST_TRACK_VERIFY=1`)

**内容**: 環境変数で verify を bypass する escape hatch を提供する。

**却下理由**: `.env` ファイルや CI 設定で誤って常時有効化されると永続的 skip になる。escape hatch は「正しい挙動」を再定義する代わりに「正しくない挙動を意図的に許可する」ものであり、root-cause 解決にならない。

### F. commit message タグで bypass (例: `[phase-0-only]` を含むと skip)

**内容**: commit message に特殊 tag を含むと verify を SKIP する。

**却下理由**: commit message が gate ロジックの一部になり、可読性 (人間がメッセージを読む役割) と機械判定 (gate ロジック) の責務が混ざる anti-pattern。tag の伝播 (履歴書き換え操作) で gate 判定が壊れる脆弱性も発生する。

## Consequences

### Positive

- Phase 0 only commit が `cargo make ci` を通過するようになり、small task commit guideline と「レビューサーフェース最小化」原則に沿った段階的 commit が可能になる
- `workflow-ceremony-minimization` convention の「file 存在 = phase 状態」原則が verify チェーン全体に一貫適用され、3 箇所の不整合が解消される
- 既存の SKIP パターン (`verify-plan-progress` 等の `validate_track_snapshots` 経路) と同じメカニズムを 3 箇所に揃えるだけなので、開発者が新しい運用ルールを覚える必要がない (検証ロジックが artifact 存在に統一される)
- ADR / Phase 0 init / Phase 1 spec を別 commit に分けられるので、レビュアの認知負荷が phase 単位に縮小される
- `type-designer-tuning-2026-04-25` のような「分割したいが gate に阻まれる」状況が再発しなくなる

### Negative

- `verify-catalogue-spec-refs` の挙動変更で、catalogue absent + spec absent が PASS になる範囲が広がる。理論上は SoT Chain ② 違反検出の感度が低下する余地があるが、catalogue absent 自体が「Phase 2 未到達」を意味するので実害は小さい
- 既存テスト `verify_fails_when_spec_missing` の改名 / 分割が必要。テスト名で意図が伝わるように分けないと、将来 regression が起きたときに原因の切り分けが難しくなる
- Phase 0 only commit が運用上一般化すると、track ごとの commit 数が増える (操作回数増)。ただしこれは設計上の意図 (small task commit guideline) どおり

### Neutral

- `/track:review` の挙動には直接影響しない (Phase 0 でも `review-scope.json` は既存の commit 済グローバル設定を読むだけ)
- `/track:done` / PR merge gate / `track-pr-merge` の strict 検査は別 ADR スコープなので変わらない
- 既存 track の commit 履歴は変わらない (前方互換)

## Reassess When

- track の Phase 構成 (Phase 0-3) が再設計された時 — artifact 存在ベース判定の前提 (`metadata.json` / `spec.json` / `<layer>-types.json` / `impl-plan.json` / `task-coverage.json` の各層対応) が崩れる
- 新しい verify が追加された時 — 新規 verify が「file 存在 = phase 状態」原則に従わない場合、本決定の整合性が崩れるので追加 verify の挙動を本 ADR の方針と揃える必要があるか再評価する
- SoT Chain (ADR → spec → catalogue → impl-plan) の上下関係が変わった時 — D2.3 の「catalogue absent → PASS、catalogue present + spec absent → FAIL」は SoT Chain ② の上下に依存しているため、関係が変わると判定ロジックを再設計する
- PR merge gate / `/track:done` の strict 検査運用が変わった時 — 中間 commit の緩和は「最終 gate で必ず検査される」前提に依存しているため、最終 gate の挙動が変わると本決定の安全性を再評価する
- `workflow-ceremony-minimization` convention の Rules「file 存在 = phase 状態」が改訂された時 — 本決定の根拠 convention が変わると ADR ごと再評価する
- Phase 0 only commit が頻発し `spec.md` absent commit が累積する場合 — 緩和の副作用として「Phase 1 spec を書かない」習慣が定着するなら、運用方針 (例: 最終 PR merge までに spec が必須かなど) を再検討する

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/conventions/workflow-ceremony-minimization.md` — Rules「file 存在 = phase 状態」原則の出典 (本決定の根拠)
- `knowledge/conventions/pre-track-adr-authoring.md` — ADR を track 前段階で作成する運用ルール (ADR 単独 commit を望む動機の補強)
- `.claude/rules/10-guardrails.md` — small task commit guideline (<500 行) の出典
- `track/workflow.md` — Guiding Principles 12「レビューサーフェース最小化」の出典
