# Observations — agent-dispatch-cost-reduction-2026-07-13

自由記述の手動観測ログ（SoT ではない）。

## 2026-07-14/15: ADR 増補の全棄却と設計 revert

- ADR-baseline review の fixer が既存 user_decision_ref の傘の下で D2/D3/D4 と Rejected A/E
  の意味論を増補（execution-contract fingerprint / build-input closure / rustdoc snapshot
  検証 / track key の機械的失効）。user 裁定は「精緻化ではなく user 決定からの逸脱」であり
  **全増補を棄却**。ADR は tmp/adr 草案（逐語原本）へ完全復元（SHA-256 一致）し、
  spec → catalogue（4 層）→ source → plan artifacts を順次縮減整合した。
- 再発防止の pre-track ADR 草案: `tmp/adr/adr-decision-freeze.md`（track init による ADR
  逐語 baseline 刻印 + commit gate バイト照合 + hunk 単位 user 裁定 + amendment 提案 lane）。

## 2026-07-15: 原文 D3 の「rustdoc 抽出の再実行を要しない」の解釈裁定（user 承認）

catalogue / spec のみの変更時に、cargo が既に生成している `target/doc/<crate>.json` を
読み直して signal 再評価だけを行う経路（読取失敗 → 再抽出へ fail-closed）は、
**原文 D3 の「catalogue や spec の変更は…rustdoc 抽出の再実行を要しない」の最小実現として
user が明示承認済み**（2026-07-15）。棄却されたのは「検証付き snapshot キャッシュの新設」
（snapshot hash field / symlink guard / 検証 lane / 新規 cache file）であり、既存 build
産物の読み直しはこれに該当しない。実装 hash の入力は対象 crate `src/` + `Cargo.lock` +
nightly toolchain 識別子の 3 つのみ（`build_inputs.rs`）。

## 2026-07-14/15: revert 後 review cycle の特記事項

- types final round が棄却設計の残骸を検出: `ProviderSessionCacheKey::TrackCapability` に
  `target_artifacts` が残存（復元 D4 は track 層 key = track × capability のみ。artifact
  identity は workspace 層限定）。全語彙 sweep をすり抜けた意味論残骸を reviewer が捕捉した
  好例。catalogue → source の順で除去済み。
- infrastructure review は trim が削った既存防御（trusted-root 封じ込め / 祖先込み symlink
  拒否 / 有界読み込み・走査 / 有界 subprocess）を段階的に復元させた。「設計の縮減」と
  「防御の縮減」を分離する briefing 制約は今後の trim 系 dispatch の必須事項。
- usecase final round が `ProviderSessionCacheError` の Io/Symlink/Path/Codec 変種による
  hexagonal 境界漏出を検出 → diagnoser routing: type → StorageUnavailable / EntryInvalid /
  IdentityBoundaryViolation の 3 概念へ再設計。

## 2026-07-15: dogfooding 状況の棚卸し

- D1（effort 明示）: 全 review round / dispatch で常用中（fast=low / final=xhigh 解決を実測）。
- D3（signal skip）: 全 gate チェーンで常用中（skip / 再抽出の分岐が毎回実行される）。
- D2 / D4（resume）: T003 の cache 基盤は完成したが consumer（T004 reviewer resume /
  T007 capability resume）が todo のため未 dogfooding。**revert の review cycle 実測**:
  同一 scope × 同一 round 種別の fix → 再 review 再入が types 6 回・infrastructure 5 回・
  usecase 4 回発生し、毎回新規 session の全文脈再構築を支払った — D2 が狙う削減対象の
  実測例。残 batch は T004 を先頭に置き、この track 自身の残り review で resume を
  dogfooding する。

- CN-05 workspace-wide hash 違反（closure 実装が対象 crate 外を走査）
- obligation evaluate の cached-Pending replay deadlock（`evaluate/cache.rs`）
- fulfillment planner の waiver precedence 無視（`evaluate/plan.rs`、check 側と非対称）
- D3 calc-impl-catalog の fail-closed の向きの誤り（評価中断ではなく再抽出へ倒すべき）
- voluntary binding の複数 obligation 帰属漏れ（`evaluate/plan.rs` が edge を最初の 1 obligation
  にしか帰属させず、check の (edge × obligation) 対要求と非対称。trait_method 系 entry で
  StaleVerdicts として顕在化。2026-07-15 修正 + regression テスト）

## 2026-07-15: revert 後の義務 gate 再収束の実測

- 収束軌跡: 未解決 130（bindings 再構築時の過剰破棄）→ semantic fail 60 → 20 → 8 → 1 → 42
  （backup 復元で universe 復帰）→ 13 → 2 → 0。implementer round 計 8 + spec 修正 1（AC-05
  の宣言不一致 routing が AC-06 と矛盾する reconciliation 欠陥）+ host 側機構修正 1。
- 主因 3 分類: (a) spec reconciliation による anchor 再キー化で全凍結 verdict が無効化、
  (b) bindings 再構築 round が revert 非対象 entry の受理済み record まで破棄、
  (c) 汎用 CRUD/伝播テストを特定挙動 anchor に bind する楽観割当（verifier は一貫して具体
  lane の実証を要求）。教訓: bindings の再構築は常に「既存 record の最大保存 + 差分修正」で
  行い、ゼロベース再生成をしない。
- 最終状態: evaluate pass=200 / fail=0 / pending=0（プローブ検出率 100%）、check
  resolved_edges=212、todo-lane 未解決 55 は許容 WARN、`cargo make ci` 全通過。
