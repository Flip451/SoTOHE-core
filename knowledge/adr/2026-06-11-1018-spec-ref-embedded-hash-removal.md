---
adr_id: 2026-06-11-1018-spec-ref-embedded-hash-removal
decisions:
  - id: D1
    user_decision_ref: "chat:2026-06-11:no-embedded-hash-in-sot"
    candidate_selection: "from:[manual-sync,auto-resync-command,embedded-hash-removal,save-hook] chose:embedded-hash-removal"
    status: proposed
  - id: D2
    user_decision_ref: "chat:2026-06-11:freshness-by-runtime-recompute"
    status: proposed
  - id: D3
    user_decision_ref: "chat:2026-06-11:semantic-layer-single-assurance"
    status: proposed
---
# SoT 本体への参照 hash 埋め込みを廃止し、新鮮度判定を verify-cache の実行時突合に一元化する — spec_refs[].hash の撤去

## Context

spec.json の要素を 1 文字でも編集すると per-element SHA-256 が変わり、全レイヤーカタログ（`<layer>-types.json`）に手動転記された `spec_refs[].hash`（宣言値）が実値と乖離して、`verify catalogue-spec-refs`（CI）と `verify plan-artifact-refs`（Phase 3 ゲート）が SpecRef hash mismatch で block する。復旧手段は手動のみ（`sotp track spec-element-hash` で新 hash を表示し、各カタログ JSON を手で直す）で、typo 修正レベルの編集にも 3 レイヤー × 複数 entry の転記負担が発生する。

hash の現在の保持状況を棚卸しすると、**人手で転記されるのは `spec_refs[].hash` の 1 種類だけ**で、他（catalogue_declaration_hash、verify-cache の claim/evidence hash、review scope hash、DRY ペア hash 等）はすべて機械生成である。また spec.json 自体は hash を一切保持せず、要素 hash は常に canonical JSON から都度計算される。

意味整合の保証は semantic verify-cache が担う設計になっている: Chain①（spec → ADR）と Chain②（catalogue → spec）の各キャッシュには、`ref-verify run` 実行時に**両端ノードから自動計算された** claim_hash / evidence_hash が記録され、`ref-verify run` / `ref-verify check-approved` はゲート判定のたびに現在のペア集合を列挙して両端 hash を再計算し、キャッシュと突合する（miss = stale = 再レビュー / block）。この突合は決定論的で LLM を呼ばない。つまり新鮮度判定に埋め込み宣言値は不要である。

さらに、埋め込み宣言値は実害を生んでいる:

1. **同期穴**: catalogue-spec-signals の評価は presence 判定のみで宣言値の妥当性を見ないため、placeholder hash が Blue のまま通過し、後段の plan-artifact-refs で初めて fail する（実トラックで観測済み）
2. **ノイズ再レビュー**: Chain② の claim_hash は catalogue entry の canonical JSON 全体の SHA-256 であり、`spec_refs[].hash` フィールドを内包する。宣言値を同期し直すたびに、意味内容が無変更でも claim_hash が変わり、意味論レビューが再発火する
3. **保証の錯覚**: hash の一致は「変更を読んで整合を確かめた」ことを保証しない（機械的に再発行すれば通る）。これは意味論レビューゲートの設計自身が認めている

原則として: **hash はキャッシュ記録時に自動計算・自動記録されるべきもので、SoT 本体（人が編集する成果物）に埋め込まれていること自体が誤り**である。

## Decision

### D1: `spec_refs[].hash` を型カタログから撤去する

`<layer>-types.json` の `spec_refs[]` から `hash` フィールドを削除し、`{ file, anchor }` の 2 フィールド構造にする（Chain① の `AdrRef { file, anchor }` と同型）。SoT 本体（spec.json / 型カタログ / ADR）はいかなる参照 hash も保持しない。hash の保管場所は verify-cache（`spec-adr-verify-cache.json` / `<layer>-catalogue-spec-verify-cache.json`）のみとし、記録は `ref-verify run` 実行時の自動計算・自動書き込みに限る。`sotp track spec-element-hash` の「type-designer が出力を転記する」用途は廃止する。

### D2: 新鮮度判定をチェーン両端の実行時再計算 + verify-cache 突合に一元化する

参照の新鮮度は、ゲート判定のたびに各チェーン両端ノード（ADR ファイル / spec 要素 / catalogue entry）の hash を再計算し、verify-cache のエントリと突合することで判定する（既存の `ref-verify run` 差分キャッシュ / `ref-verify check-approved` の機構そのまま）。宣言値と実値の突合を行っていた層②の検査（`verify catalogue-spec-refs` / `verify plan-artifact-refs` の SpecRef hash mismatch 検査）は撤去し、決定論的な新鮮度検査が CI に必要な箇所は `ref-verify check-approved` 相当の cache 突合（LLM 不要）で置き換える。anchor の実在検査（dangling anchor 検出）は hash と独立した構造検査として維持する。

### D3: 「読んで確かめた」保証は意味論層に一本化し、fail-closed を維持する

整合確認の実体は verify-cache の再レビュー発火（両端 hash 変化 → stale → 意味論レビュー）に一本化する。hash 変化で stale になったペアは、意味論レビューを通過するまでゲートが通らない。いかなるコマンドも verify-cache の verdict を機械的に書き換えてはならない（hash の再記録は許すが、Pass/Fail 判定の改変は禁止）。hash の一致を整合確認の代替として扱う運用・文書記述（`spec-element-hash` の help テキスト、type-designer agent 定義の転記手順等）は撤去する。

## Rejected Alternatives

### A. 手動同期の維持（現状）

typo 修正にも 3 ファイル以上の手修正が要る。Chain①（ADR 参照）では「typo 修正で過敏に無効化される」ことを理由に canonical hash の導入を却下しており、spec 要素にだけ同じ負担を残すのは一貫しない。却下。

### B. 下流 hash の一括再同期コマンド（`sotp track sync-spec-refs`）の追加

旧草案（tmp/adr/02-spec-ref-hash-auto-resync.md）の案。転記の摩擦は消えるが、埋め込み宣言値そのものが残るため: (1) Chain② claim_hash が宣言値を内包し続け、resync のたびに意味不変のノイズ再レビューが発火する、(2) フィールドが手書き可能なまま残り placeholder の余地が構造的には消えない、(3) カタログ（type-designer 専有ファイル）を CLI が書き換える writer-ownership 上の整理が別途必要になる。撤去すればこれら全てが原理的に消える。却下。

### C. spec 編集時に下流を自動で書き換える保存フック

暗黙の連鎖書き換えは事故時の原因切り分けを難しくし、1 ファイル 1 writer の原則とも衝突する。却下。

### D. spec_refs から hash を撤去すると差分検知粒度が失われる、という旧草案の懸念

旧草案が撤去案を却下した理由だが、事実誤認。Chain② の evidence_hash は spec.json から**要素単位で実行時に計算**されており、埋め込み宣言値に依存しない。フィールドを撤去しても per-element 粒度の再発火はそのまま機能する。懸念自体が成立しないため、撤去を妨げない。

## Consequences

### Positive

- spec 編集の追従コストがゼロになる（転記も resync コマンドも不要。次回 ref-verify run が自動で差分を検出して該当ペアだけ再レビュー）
- placeholder hash という故障モードがフィールドごと消滅する（同期穴の構造的解消）
- Chain② claim_hash から hash 保守ノイズが消え、意味のある変更だけが再レビューを発火する
- hash の保持場所が「機械生成のキャッシュ・signals のみ」に統一され、メンタルモデルが単純になる
- catalogue schema が AdrRef と同型の `{ file, anchor }` になり、参照表現が SoT Chain 全体で一貫する

### Negative

- カタログ schema の breaking 変更（schema_version 更新 + 既存トラックのカタログ移行が必要）。進行中トラックが少ないタイミングでのマージが必要
- 層②（構造・新鮮度）の決定論的検査が cache 突合方式へ置き換わるため、verify-cache が存在しない状態（初回）では新鮮度を主張できない — ただしその場合は意味論レビュー自体が未実施なので check-approved が block し、fail-closed は保たれる
- 整合確認の防衛線が意味論レビュー 1 層に集約されるため、レビュー品質（モデル・プロンプト・known-bad probe 校正）への依存が上がる

### Neutral

- catalogue-spec-signals の presence 判定（spec_refs の有無による 🔵🟡🔴）は hash と無関係のため影響なし。catalogue_declaration_hash（signals ファイル側、機械生成）も存続

## Reassess When

- 意味論レビューの誤通過（裏付けのない Pass）が観測された時 — 決定論的な宣言値突合の再導入を検討
- canonical JSON の仕様変更など、hash 計算方式自体を変える時
- verify-cache を持たない外部ツールが catalogue 単体で新鮮度を判定する必要が生じた時

## Related

- `knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md` — 意味論レビューゲートと 3 層モデル（presence / 構造・新鮮度 / 意味論）。D4 の Chain② claim_hash 定義は本 ADR D1 により「hash フィールドを含まない entry canonical JSON」へ実質変更される
- `knowledge/adr/2026-06-10-1335-ref-verify-existence-based-scope-resolution.md` — 同系列の「申告・転記より実行時導出」方針（スコープ解決の存在ベース化）
- `knowledge/conventions/workflow-ceremony-minimization.md` — 人工的な状態・形骸化する宣言値を作らない原則
- 旧草案 `tmp/adr/02-spec-ref-hash-auto-resync.md` は本 ADR で置き換え（破棄予定）
