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

## 2026-07-16: D3 hash 入力の解釈裁定（user 承認）

PR #195 round-3 の Codex P1（manifest 変更で stale signal が fresh 扱いされる）への
対応として、**crate 自身の `Cargo.toml` + `build.rs` は「対象 crate の source」の
一部として implementation-input hash に含める**ことを user が承認（2026-07-16）。
crate 境界内の解釈であり、棄却済みの workspace build-input closure（他 crate /
workspace root / 抽出契約の hash 化）とは別物。crate 境界の外へは広げない。

## 2026-07-16: PR round-8 rustdoc 再利用 finding の Accepted Deviation（user 裁定）

Codex review round-8 の「catalogue-only 変更での既存 rustdoc JSON 再利用が
implementation hash に束縛されていない（別 source での target/doc 上書き→元 source
への revert で hash 一致のまま stale JSON を信頼し得る）」に対し、user 裁定
（2026-07-16）で **Accepted Deviation（現状維持）**。理由: リスク条件は限定的で、
対策候補の content-hash field は棄却済みの snapshot hash 検証 lane の復活、常時
再抽出は承認済みの D3 最小実現（既存 JSON 読み直し）の放棄にあたるため。
最悪ケースの明示（user 確認済み）: 偽 🔴（無害・時間損失のみ）に加えて**偽 🔵
（構造整合 gate の誤通過）が低確率で起き得る**。発火には「別 source での
target/doc 上書き → 厳密 revert で hash 一致 → source 無変更のまま catalogue-only
変更」の 3 条件が必要で、次の実装変更（hash 変化→再抽出）で自己回復する有界汚染。
user 裁定: 「一旦許容。問題が顕在化したら対処」（2026-07-16）。

## 2026-07-15/16: PR #195 review cycle と resume dogfooding の副産物

- Codex Cloud review が実バグ 3 件を検出: waiver 裁定の多重 owner 欠落（T019 の
  voluntary 修正と同型の欠陥が waiver 側に残存）、codex reviewer の fresh 再試行時
  stale verdict、capability stdout の破棄退行。
- stdout 破棄退行は本日の capability dispatch「silent death」多発の根本原因だった
  （T007 の session-id 捕捉が specialist の最終報告を飲み込んでいた）。診断力の
  低下がバグ修正を遅らせる悪循環を生んだ — collector の tee 化で解消。
- reviewer resume dogfooding: fast 再入で同一 session id 継続を実測（16:29 entry が
  019f649f 世代の id を保持）。capability resume は T009 の再 dispatch で --resume
  を実運用（silent death 起因で報告は未観測、fix 後の dispatch から有効に観測可能）。
- review-fix-lead が古い briefing 記述（「空 Resume の degrade は意図的」）を根拠に
  types 裁定済みの fix を差し戻す事故を観測 — cross-scope fix では briefing 間の
  整合性維持が前提条件。scope hash の自己失効（.provider-sessions / obligation
  cache の review_operational 未登録）も dogfooding が発見。
