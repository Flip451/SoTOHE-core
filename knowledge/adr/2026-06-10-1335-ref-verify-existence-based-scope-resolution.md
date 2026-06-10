---
adr_id: 2026-06-10-1335-ref-verify-existence-based-scope-resolution
decisions:
  - id: D1
    user_decision_ref: "chat:2026-06-10:phase-from-artifact-existence"
    status: proposed
  - id: D2
    user_decision_ref: "chat:2026-06-10:context-flag-removal"
    candidate_selection: "from:[A,B,C,D,remove-context] chose:remove-context"
    status: proposed
  - id: D3
    user_decision_ref: "chat:2026-06-10:adr-add-hearing"
    status: proposed
---
# ref-verify のスコープ解決を artifact 存在ベースに一本化する — --context / --layer の削除と Phase 0 コミットゲート誤爆の解消

## Context

`bin/sotp ref-verify run --context commit-gate`（commit ゲート `track-commit-message` が呼ぶ）が、Phase 0（`/track:init` 直後、spec.json 不在）で決定論的に fail する。pre-track-adr-authoring の標準フロー（init 直後に ADR + metadata.json を review → commit）が全新規トラックで block される。

原因は RefVerifyScopeResolver の CommitGate / Standalone arm が無条件に spec.json の存在を要求すること。`check-approved` も内部で CommitGate コンテキスト固定で同じ resolver を通るため同様に fail する。

これは既確立の「file 存在 = phase 状態」原則（`knowledge/conventions/workflow-ceremony-minimization.md` → `2026-04-27-0324-phase-aware-verify-gates.md` D1 で verify チェーン全体に適用、兄弟系列 `2026-06-01-0406` / `2026-06-03-1241`）に違反する実装。

また --context は (1) どの Chain を見るか (2) phase 直後の発火面でのコスト絞り込み の 2 情報を運ぶが、(1) は artifact 存在から導出可能（catalogue 全不在 → Chain② zero pairs は実装済み）、(2) は差分キャッシュが同じ仕事をしており機構が二重化している。commit-gate と standalone は現状完全に同一挙動。

## Decision

### D1: スコープ解決を artifact 存在ベースに一本化する

ref-verify の検証ペア集合は、発火面の申告ではなく track ディレクトリの SoT 成果物の存在有無から導出する:

- spec.json 不在 → Chain① zero pairs（Phase 0 は両 Chain zero → ゲート通過）
- catalogue 全不在 → Chain② zero pairs（実装済み挙動を追認）
- 両方存在 → 両 Chain。差分キャッシュが再レビューを増分に限定

`2026-05-27-1601-sot-chain-semantic-review-gate.md` D9 のうち「呼び出しコンテキストからのスコープ自動解決」を本決定で置き換える。D9 の他の構成要素 — 専用 `/track:ref-verify` skill + `bin/sotp ref-verify` サブコマンドという呼び出し口、発火点ごとに別実装を持たない 1 コマンド構成、差分キャッシュによる増分再レビュー — は維持する。

### D2: --context / --layer を削除する

CLI 引数と RefVerifyInvocationContext を削除し、`bin/sotp ref-verify run` の一形態に統一。check-approved 内部の CommitGate 固定も削除。plan.md の Phase 1/2 first-line 発火・Makefile・/track:ref-verify skill も同じ一形態に簡素化（Phase 2 の per-layer N 回実行 → 1 回）。

### D3: fail-closed は維持する

skip が効くのは「整合的な不在」のみ:

- 部分 catalogue（ある層だけ存在）→ fail
- catalogue 存在 + spec.json 不在（SoT Chain 上下関係違反）→ fail
- 存在するが壊れている（parse 不能等）→ fail

## Rejected Alternatives

### A. CommitGate のみ SKIP、Standalone は fail 維持（--context を活用）

spec.json 不在時に commit-gate コンテキストだけ skip し、手動診断用の standalone は loud に fail させる案。「file 存在 = phase 状態」原則の下ではコンテキスト分岐自体が不要で、機構の二重化（context 申告 vs 存在導出）と発火面ごとの挙動差を運用者が覚える負担が残る。却下。

### B. metadata.json に phase マーカーを追加

`2026-04-27-0324-phase-aware-verify-gates.md` Rejected Alternative A で既に却下済みの anti-pattern（人工状態・形骸化リスク）。再度却下。

### C. Makefile / シェル側で phase 判定して ref-verify を skip

ゲート挙動の判定ロジックが Rust の resolver とシェルに分散し SSoT が壊れる。却下。

### D. --context を維持したまま CommitGate arm の検証だけ緩和

Phase 0 は通るが、commit-gate ≡ standalone の縮退と機構二重化が温存される。最小修正としては成立するが、原則への準拠を徹底するなら削除が自然。却下。

## Consequences

### Positive

- Phase 0 で commit ゲートが通り、ADR 初回コミットの標準フローが全新規トラックで機能する（抽出テンプレートの fresh repo も同様）
- CLI 表面が `ref-verify run` 一形態に簡素化。発火面ドキュメント（plan.md / skill / Makefile）も簡素化
- Phase 2 発火が per-layer N 回 → 1 回になり probe 本数が床 2 本に減る
- Phase 2 中の spec.json 変更による Chain① 再検証を Phase 2 発火時点で拾える（現状は commit gate まで検出が遅れる）
- スコープ解決機構が 1 つになり RefVerifyInvocationContext 関連コードが消える

### Negative

- Phase 2 直後の発火で変更ペアがある場合、Chain① 用 calibration probe が 1 本余分に乗る（軽微なコスト増）
- spec.json 不在の手動実行が zero pairs で OK 表示になる（SKIP 理由を出力して誤認を緩和する）
- マージ済み ADR `2026-05-27-1601` D9 の部分上書きが発生する

### Neutral

- Chain 別 verifier capability / prompt template（同 ADR D11）は cache_scope で決まるため影響なし。probe 床ロジックもペア存在ベースのためそのまま機能

## Reassess When

- 発火面ごとに検証の厳しさ（モデル tier、probe 率等）を変えたい要求が実証されたとき — context 引数の再導入を検討
- 一部レイヤーのみ catalogue を作る track 運用（部分 catalogue が正当な状態）が導入されたとき — fail-closed 判定 (D3) の再設計が必要
- 差分キャッシュ（ペア単位 hash）の前提が変わったとき — All 常時実行のコスト前提が崩れる
- track の Phase 構成（Phase 0-3 と SoT 成果物の対応）が再設計されたとき

## Related

- `knowledge/conventions/workflow-ceremony-minimization.md` — 「file 存在 = phase 状態」原則の出典
- `knowledge/adr/2026-04-27-0324-phase-aware-verify-gates.md` — 同原則の verify チェーン適用（D1）
- `knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md` — 兄弟系列1（catalogue 欠損寛容）
- `knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md` — 兄弟系列2（spec 欠損寛容）
- `knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md` — D9（コンテキスト解決）を本 ADR D1/D2 が部分上書き。専用コマンド surface と差分キャッシュは維持
