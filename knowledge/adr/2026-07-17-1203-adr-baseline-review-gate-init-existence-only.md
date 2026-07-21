---
adr_id: 2026-07-17-1203-adr-baseline-review-gate-init-existence-only
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session-01P6BqX8JsHL7ePVmPddtadn:2026-07-17 「ADR の状態で review をブロックするのは ledger が未 init の時だけにする」の 1 決定への縮約裁定（Phase 0 境界裁定 — zero_findings まで無刻印で修正 → user エスカレーション → 承認後に刻印 → コミット — を機構側で成立させる修理として） + chat_segment:session-01P6BqX8JsHL7ePVmPddtadn:2026-07-17 Phase 0 承認エスカレーションで、D1 の承認記録方式を freeze D6 整合形（承認は front-matter refs へ追記・escalation reason は欠落説明のみ・無編集収束時は刻印なし）へ訂正する編集を承認する裁定"
    candidate_selection: "from:[review-entrance-init-existence-only,approved-kind-plus-check-commit-approval-machinery,phase0-draft-state-branch,allow-intermediate-stamping,remove-review-entrance-check-entirely] chose:review-entrance-init-existence-only"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session-01P6BqX8JsHL7ePVmPddtadn:2026-07-17 Phase 0 収束全体の承認（workflow SSoT 追随義務を含む文面の裁定）"
    status: proposed
---
# ADR-baseline の review 入口検査を init 刻印の存在確認のみに縮小する

## Context

2026-07-17 の adr2pr 運転で、orchestrator がレビュー所見を受けて ADR の意味を track 内で書き換え、編集のたびに escalation 刻印を打って byte 照合を自己解消する事故が起きた。この事故を受けた user 裁定により、`knowledge/conventions/pre-track-adr-authoring.md` に「In-track 意味変更の裁定権」節が新設された。Phase 0 の手順は: init 刻印（持ち込み文面の記録 = diff 基準点）→ **ledger に書き込まない**レビューループ（adr-diagnoser の決定保存判定付きで意味変更を含む修正を適用）→ zero_findings 到達で user エスカレーション → 承認された編集があれば対象 decision の front-matter `user_decision_ref` に承認 ref を追記してから刻印 → コミット、ここまでが境界。編集がなければ init 記録が承認文面と一致しているため、追加の刻印は行わない。

しかし現行の `adr-baseline check-review` は「現在文 = 最新 baseline」の byte 一致を全レビューラウンドの前提とするため、この Phase 0 ループは機構的に成立しない — 編集を適用した瞬間に全レビューが block し、中間刻印（禁止された自己洗浄）なしには収束できない。この食い違いは規範制定直後の再走で実際に顕在化した。

freeze 機構を定めた ADR（`2026-07-16-2001-adr-decision-freeze.md`）を精読すると、review 入口での byte 照合 block は D4 が決定しているが、その設計根拠は「**init 刻印忘れ**が fixer の走る前に loud に発覚する」ことにあり、byte 照合の本来の守備範囲は「commit gate は review 中の fixer 編集を含むそれ以降の乖離を捕捉する」と commit / CI 側に置かれている。さらに同 ADR の D7 は「**本機構は user の承認成果物の保護を目的とし、user 承認前の新規成果物の変化を遮らない**」という原則を track 生まれの draft ADR について既に確立している。Phase 0 修正ループの導入は「持ち込み ADR も承認までは draft」という状態を生んだのであり、review を byte 照合で止めることは同機構自身の原則に照らしても過剰である。また同 ADR の D1 は「正規経路の再刻印を経た baseline は merge 時の user 監査を待つ文面」と定めており、Phase 0 承認後の刻印はこの錨定義にそのまま収まる。

したがって、機構修理そのものは D1 の 1 決定で足りる。一方、承認済みの Consequences / Neutral に記録されていた workflow SSoT の追随義務は、下流が機械的に参照できる別決定 D2 として記録する。

## Decision

### D1: review 入口の ADR 状態検査は「init 刻印の存在確認」のみとし、byte 照合の発火点を commit gate と CI に限る

`adr-baseline check-review`（review 入口）の ADR 状態に基づく block 条件を「track 主対象 ADR の init 刻印が ledger に存在すること」の確認のみに縮小する。現在文と最新 baseline の byte 照合は review 入口では行わず、その発火点は従来どおり commit gate（`check-commit`）と track-aware CI に限る。ledger 検証（記録済み複製の整合確認）は現行のまま維持する。

これにより:

- **Phase 0 ループが規範どおり成立する** — init 刻印との乖離（レビュー中の draft 状態）は review を block せず、無刻印のまま zero_findings まで収束できる。編集を伴って収束した場合は、user 承認 → ループで修正した decision の front-matter `user_decision_ref` へ承認 ref を追記 → 既存の escalation kind で刻印（reason は何が欠けていてなぜ変更したかという自己完結の欠落説明だけを記し、承認を重複記録しない）→ コミット（byte 一致）で境界が閉じる。編集がなければ init 記録がそのまま承認文面なので刻印しない。専用の承認 kind は導入しない。
- **無断改変の検出は失われない** — review 中の乖離はその後の commit gate が、hook 迂回は CI が、従来どおり fail-closed で捕捉し、D5 のトリアージ（adr-diagnoser）に入る。review 入口の本来目的である init 刻印忘れの早期検出も維持される。
- **Phase 1 以降も同一規則で動く** — 正規の escalation 編集は編集直後の刻印で一致が保たれ、刻印忘れ・無断改変は commit / CI で block される。review 入口が特別扱いする状態は存在しない。

### D2: workflow SSoT を裁定権規範に追随させる

review / plan / adr2pr の workflow SSoT を、`knowledge/conventions/pre-track-adr-authoring.md` の裁定権規範に追随させる。追随範囲は、同規範が要求する守護者判定の配線と判定出力のレビューへの還流、および Phase 0 user 承認エスカレーションの carve-out を各 workflow に反映することに限る。裁定権規範の SSoT は同 convention に留め、workflow SSoT では規範を再定義しない。

## Rejected Alternatives

### A. 承認刻印 kind の新設 + check-commit の承認必須化 + draft 状態分岐（本 ADR の初稿 = 5 決定案）

snapshot kind に `approved` を追加し、`check-review` を approved 記録の有無で分岐させ、`check-commit` に承認記録を必須化して「承認 → 刻印 → コミット」の順序を機械強制する設計。裁定点の無断通過を機構で封じられるが、kind enum 拡張・check 二種の改修・既存 committed track との互換判定を要し、目的（Phase 0 ループの収束）に対して過大。承認の存在は ADR 自身の front-matter refs（chain ⓪ の検査対象）・累積複製の PR diff・merge 監査で十分に可視であり、freeze 機構 ADR が初版ドラフトを縮小したのと同じ minimization 判断で却下。承認前刻印による自己洗浄が再発した場合に再訪する（Reassess When）。

### B. 中間刻印の容認（編集ごとに escalation 刻印してループを回す）

事故前の実運用そのもの。承認前の刻印は user 監査の diff 基準を編集者自身が洗浄する行為であり、規約（In-track 意味変更の裁定権）が禁止済み。却下。

### C. review 入口検査の全撤去

byte 照合だけでなく init 刻印の存在確認まで失う。刻印忘れが commit 時点まで発覚せず、freeze 機構 D4 が review 入口検査に置いた本来目的（fixer が走る前の loud な検出と安全な回復）を損なう。却下。

### D. finding 単位で user 承認を都度仰いでから編集・刻印する

ループ収束前の割り込みが多発し自律実行の価値を損なう。収束後の一括レビューが user の裁定コストを最小にする（規約の Phase 0 手順として確立済み）。却下。

## Consequences

### Positive

- 変更が check 一箇所の縮小に閉じる — kind enum・`check-commit`・CI・ledger 形式はすべて無変更。
- Phase 0 の無刻印修正ループが機構的に成立し、規範（In-track 意味変更の裁定権）と機構の食い違いが解消する。
- freeze 機構の役割分担が D4 自身の設計根拠と一致する形に整う: review 入口 = 刻印存在の早期検出 / commit・CI = byte 照合による無断改変の遮断。

### Negative

- 承認前の中間刻印（自己洗浄）は機械では止まらず、規約 + ledger 累積の可視性 + merge 監査（init 版 vs 最終版の diff 一発）に委ねる（Rejected A の再訪条件）。
- review 中に発生した無断改変の検出が commit 時点まで遅延し、その間のレビューラウンドが無駄になりうる（永続化は commit / CI が遮断するため被害は手戻りに限定）。

### Neutral

- workflow SSoT の追随義務と規範の所在は D2 に記録した。
- 承認の記録は、ループで修正した decision 自身の front-matter `user_decision_ref` が担い、chain ⓪ が検証する。編集がある場合、承認 ref を含む文面を escalation kind で刻印し、reason には変更へ至った自己完結の欠落説明だけを記録するため、ledger は承認記録を重複して持たない。編集がなければ init 記録が承認文面と一致するため、追加の刻印は行わない。

## Reassess When

- 承認前刻印による自己洗浄が再発したとき（Rejected A — 承認刻印 kind + `check-commit` 承認必須化 — を再訪）。
- commit 時点検出への遅延コスト（無駄レビューラウンド）が実測で問題化したとき（review 入口 byte 照合の条件付き復活を検討）。
- Phase 0 の user 承認エスカレーションが運用上のボトルネックになったとき。

## Related

- `knowledge/conventions/pre-track-adr-authoring.md` — 規範の正本（In-track 意味変更の裁定権）
- `knowledge/adr/2026-07-16-2001-adr-decision-freeze.md` — freeze 機構の原典（D4 の発火点設計と設計根拠、D6/D7 の承認記録と欠落説明の分離、D7 の「承認前の成果物を遮らない」原則、D1 の錨定義）
- `.harness/capabilities/adr-diagnoser.md` — 守護者 capability の定義（編集判定モード）
