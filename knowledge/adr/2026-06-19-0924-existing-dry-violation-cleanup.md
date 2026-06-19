---
adr_id: 2026-06-19-0924-existing-dry-violation-cleanup
decisions:
  - id: D1
    user_decision_ref: "chat:/adr:add existing-dry-violation-cleanup hearing (2026-06-19)"
    candidate_selection: "from:[A,B,C,D] chose:none-of-rejected (remediation track)"
    status: proposed
  - id: D2
    user_decision_ref: "chat:/adr:add existing-dry-violation-cleanup hearing (2026-06-19)"
    status: proposed
  - id: D3
    user_decision_ref: "chat:/adr:add existing-dry-violation-cleanup hearing (2026-06-19)"
    status: proposed
  - id: D4
    user_decision_ref: "chat:/adr:add existing-dry-violation-cleanup hearing (2026-06-19)"
    status: proposed
  - id: D5
    user_decision_ref: "chat:/adr:add existing-dry-violation-cleanup hearing (2026-06-19)"
    status: proposed
---
# 既存 DRY 違反の一掃 — 横断・既存重複を正典へ集約する

## Context

DRY ゲート（`sotp dry` / dry-checker）は **PR の diff を embedding 類似度で corpus 照合し、新規の意味的重複をブロックする** diff スコープの予防ゲートである。2026-06-19 に独立した DRY 違反 census（ゲート導入直前 `c4da67a4` と最新 main `9270de33` の before/after 比較）とゲート自身のキャッシュ verdict 評価を実施した結果、以下が判明した:

- **ゲートは既存・横断・データ重複を構造的に取りこぼす**。違反密度は before 0.942 → after 0.842 件/KLoc とわずかに低下したが、その低下は cli / cli-composition のクレート分離クリーンアップ（人手リファクタ）にほぼ全て起因し、**ゲートが新規 diff を統治する成長層（usecase +11%、infrastructure +10%）はむしろ密度が上昇**した。
- ゲートのキャッシュ（16 トラック・4,752 verdict）では violation 273 件を捕捉する一方、**最重要違反である `validate_track_id` の5重複を一度も head-to-head で評価していない**（diff に現れず、層をまたぐコピーは embedding 類似度が低く候補化されないため）。judge の判断品質は高いが、射程がローカル near-clone に限られる。

したがって、ゲートに委ねても解消されない既存重複は、意図的な remediation 作業として別途一掃する必要がある。census が確認した残存重複のうち、本 ADR では影響度の高い 4 クラスタを対象とする。

## Decision

### D1: track-ID / slug 検証を domain の正典に一本化する

`validate_track_id` / slug 検証ロジックが domain（`libs/domain/src/ids.rs` の `is_valid_track_id`）を正典としながら、usecase 3 モジュール（`catalogue_impl_signals` / `type_signals` / `baseline_capture`）+ CLI 2 箇所 + `apps/cli-composition/src/verify.rs` の `validate_track_id_str` に計 5 つ以上の独立実装として散在している。全コピーを削除し、`TrackId::try_new`（domain）への委譲に置き換える。これは cross-layer knowledge-dup であり、文法変更時の乖離バグリスクが最も高い。

### D2: 空/空白禁止の不変条件を NonEmptyString に集約する

「フィールドが空・空白のみであってはならない」という不変条件が、既に `NonEmptyString` 型が存在するにもかかわらず domain の 8 箇所以上（`ids.rs` / `plan.rs` / `spec.rs` / `impl_plan.rs` / `review_v2/types.rs` 等）でインライン再実装されている。各箇所を `NonEmptyString` への委譲に置換し、不変条件の単一定義を回復する。

### D3: review_v2 と dry_check で重複する subprocess / lock / git 処理を共通化する

`review_v2` と `dry_check` の間で、Codex subprocess 管理（`spawn_codex` / timeout-poll ループ / `drain_pipe` / `tee_stderr_to_file`）、排他ロック取得パターン（`WriteGuard` ↔ `FsDryCheckStore::acquire_write_lock`）、4-source git-diff union（merge-base + cached + worktree + untracked）、SHA-256 → 小文字 hex エンコードが重複している。共通モジュールへ抽出する。**hexagonal の層配置は尊重**し、共通化が層境界（domain / usecase / infrastructure）を侵さない形にする。

### D4: test ヘルパ・定数を単一定義へ集約する

`CwdGuard` / `init_git_repo`（6 箇所）/ stub bindings（usecase test 3 モジュール）などの test-only ヘルパを `#[cfg(test)]` の共通 test-support モジュールへ集約する。`POLL_INTERVAL`（5 箇所）/ `"tmp/reviewer-runtime"`（4 つの const 定義 + 1 つの inline literal）/ track ディレクトリ系のマジック文字列などの定数を単一の `const` 定義へ統合する（後者は embedding ゲートが拾えない data-dup）。

### D5: 進め方 — クラスタ別・小コミットで挙動不変を保証する

各クラスタについて「正典を決める → コピーを正典へ委譲 → `cargo make ci` で挙動不変を確認 → 小さく分割してコミット」の手順を踏む。4 クラスタは互いに独立しているため別タスク/別トラックに分割し、1 コミットあたりの diff を小さく保つ（レビューコストは diff サイズに対し約 O(N^2) で増大するため、guardrails の small-task-commit 方針に従う）。

## Rejected Alternatives

### A. ゲートに任せて何もしない

却下。ゲートは diff スコープかつ embedding 候補ゲートのため、既存重複・cross-layer knowledge-dup・data-dup を構造的に検出しない（キャッシュ評価で `validate_track_id` 5重複が未評価であることを実証）。放置すれば残存し続ける。

### B. ゲートを full-corpus 化して自動修正させてから一掃する

却下（本 ADR の範囲外）。diff 非依存の full-corpus sweep や cross-layer 閾値調整はゲート再発防止として有用だが、ゲート本体の大改修であり、既存重複の即時一掃という目的とは軸が異なる。ゲート拡張は別 ADR で検討し、本 ADR は remediation に限定する。

### C. 全クラスタを 1 トラックで一括大規模リファクタする

却下。レビューコストが diff サイズに対し超線形（約 O(N^2)）で増大し、レビュー往復が膨らむ。クラスタ別・小コミットに分割する（D5）。

### D. 重複を許容し clippy 等の lint 強化だけで対応する

却下。lint は near-clone やセマンティックな cross-layer knowledge-dup を捕捉できず、「どのコピーを正典とし他を委譲させるか」という設計判断を代替しない。定数 data-dup の一部は lint で補完しうるが、それは D4 の補助に留まる。

## Consequences

### Positive

- 変更増幅の解消（例: track-ID 文法変更が正典 1 箇所で完結）と、層をまたぐ乖離バグの予防。
- `NonEmptyString` 等の既存型が本来の用途に回帰し、不変条件が単一の真実源を持つ。
- ゲートの構造的死角（既存・横断・データ重複）を人手で補完し、コードベース全体の DRY 衛生が底上げされる。

### Negative

- 一時的なリファクタ工数とレビュー負荷が発生する。
- 共通化により層間・テスト間の結合がわずかに増える（hexagonal 境界を尊重し、過剰な共通化は避けて最小化する）。

### Neutral

- 本 ADR はゲート本体を変更しない（remediation のみ）。再発防止のためのゲート拡張は別 ADR の関心事とする。

## Reassess When

- 一掃後に DRY 違反 census を再実行し、密度（特に cross-layer / knowledge-dup）が目標水準に低下したことを確認したとき（完了判定）。
- ゲートが full-corpus / cross-layer 検出に拡張され、既存重複を自動検出・修正できるようになったとき（本 ADR の前提が変わる）。
- 共通化が過剰結合を生み hexagonal 境界を侵し始めたとき（揺り戻しの検討）。

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/adr/` 配下の dry-checker / DFP⇄RFP 系 ADR — DRY ゲート本体。本 ADR が補完する対象
- `knowledge/conventions/coding-principles.md` — エラーハンドリング / 命名 / モジュール規約
- `knowledge/conventions/prefer-type-safe-abstractions.md` — Newtype / Enum-first パターン（`TrackId` / `NonEmptyString` への集約根拠）
- `knowledge/conventions/dry-check-workflow.md` — DRY ゲートの運用
