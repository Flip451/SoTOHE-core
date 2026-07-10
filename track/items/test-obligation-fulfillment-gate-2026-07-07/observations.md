# Observations — test-obligation-fulfillment-gate ドッグフード記録

本トラック自身の ~63 義務（最終 edge 数 518）に対して bindings 著作 → evaluate → check の
修復ループ（R1〜R29）を回し、`check` exit 0（pass=518 / fail=0 / pending=0、calibration
100%）まで収束させた際の観測記録。方法論の恒久版は
`.harness/workflows/track/obligation-fulfillment.md` と implementer capability 契約に反映済み。

## 1. derive のタイミング: Phase 2 直後が最適、後付けは最弱形

義務導出はカタログ確定（Phase 2）直後に可能であり、そこで導出して実装タスクと並走させるのが
最適。本トラックは実装が概ね済んだ後に義務を後付け適用する「最弱形」となり、初回 evaluate で
440 edge 中 277 件が拒否された。実装と同時に義務を消化していれば、各バッチの差分 edge のみを
判定する定常運転になっていたはず（後述のキャッシュ特性）。遅く感じたのは並列度の問題以前に、
先送り自体が原因。

## 2. 判定者の D6 準拠バグ（whole-anchor 判定）

fulfillment 判定 prompt の `central_unverified` 定義が「anchor の中心的挙動」を要求する文言に
なっており、D6 の edge-locality（当該 entry に関わる promise の部分だけを判定する）を判定者が
破っていた。prompt を entry-relevant 化して修正。ただし fail-closed stand-in のような
「anchor の中心挙動を原理的に示せない」entry に対しては、修正後も whole-anchor 傾向の
substitution 判定が残った（→ §7）。

## 3. verifier-prompt fingerprint（D16）

prompt 修正が hash 三つ組で凍結済みの verdict に伝播しないギャップが露見。当初キャッシュを
手動削除して対処したが、ユーザー裁定で「キャッシュは生かしつつ失効分は無視」の設計に是正され、
D16（verifier-prompt fingerprint を verdict に付与、不一致・不在は存在しないものとして再評価）
として実装（T029）。以後 prompt 変更は自動で全件再判定に落ちる。

## 4. 人間裁定の記録機構が無い

fast → final → 人間 のエスカレーションは 2 度人間に到達したが、人間の verdict をキャッシュに
記録する機構が存在しない。裁定結果は bindings の書き換え（転換・文言修正）として間接的にしか
反映できず、同一 edge が再び final 拒否 → 人間、と循環し得る。follow-up 候補。

## 5. evaluate の並列化と一時故障リトライ

当初 evaluate は直列で、後付け一括消化（数百 edge）では律速だった。N=8 の bounded
multiplexer（usecase `evaluate/concurrency.rs`）+ spawn_blocking オフロード +
一時故障（capacity / rate-limit / launcher race）への 3 回 backoff リトライを実装。

## 6. キャッシュ有効性の実測

- ラウンド間で verdict は安定（束ね直した record の edge のみ再判定される diff-scale 挙動）。
- 全キャッシュ破棄実験では同一入力でも ~10% の判定揺れを観測。凍結キャッシュは再現性装置と
  して本質的（コスト削減だけではない）。
- record の tests 集合を編集するとその record の**全 edge** の set hash が変わり再判定になる。
  逐次追加はダイス再ロールの繰り返しになるため、record 単位で全 edge の要求を一括で満たす
  evidence set を組むのが正しい（R28→R29 で学習）。

## 7. 構造的に充足不能な継承 edge（FailingObligationFulfillmentVerifier × AC-06）

port trait の spec_refs を trait_impl が継承して生成される edge のうち、fail-closed stand-in
× 引用規律 (AC-06) は両レーンで原理的に詰んでいた:

- fulfillment レーン: stand-in は pass verdict を生成できず、常時エラーの証拠は
  substitution として拒否される。
- waiver レーン: 判定者は「宣言か anchor が『verdict を生成できない』を確立せよ」と要求するが、
  `TraitImplDeclV2` には docs フィールドが無く宣言側では確立不能（trait_impl の宣言描画は
  Debug 出力のみ）。

根本原因は spec 側の欠落（provider 未解決時の挙動をどの要素も規定していなかった）。AC-06 に
fail-closed no-provider 条項を追記する anchor 側接地で解消（grounding escalation chain の
spec-designer カスケード）。教訓: 継承 edge の各実装クラス（正規 adapter / stand-in）の役割は
anchor 本文が名指しする必要がある。trait_impl 宣言に docs を持たせるスキーマ拡張は follow-up
候補。

## 8. anchor 修正の失効コストと収束特性

AC-06 追記で同 anchor の 43 edge が失効・再判定され、5 件が flake（~12%）。ただし拒否 reason
は多くの場合**不足しているテストを名指し**しており（例: 「waiver 側 cache-key 三つ組の検証が
無い」→ `test_waiver_cache_key_changes_with_any_component` が既存）、reason 駆動の再束ねで
3 ラウンド（R27〜R29）で収束した。anchor 修正は「失効コストは中程度・収束は速い」。

## 9. IN-10 準拠ギャップの検出（ゲートの実利確認）

`results` の record block が IN-10 要求（義務 id / claim source / evidence source / reason）を
描画できない実装ギャップを、ゲート自身が `central_unverified` として検出した（T030 で
`EdgeVerdictRecord` を拡張して修正）。「テスト不足」拒否の中に本物の spec 乖離が混ざっている
ことがあり、拒否 reason を実装ギャップの検出器としても読むべき。

## 10. check の総量性は evaluate との直列 && で隠れる

`evaluate && check` で連結すると evaluate が落ち続ける間 check の未解決 edge 一覧が一度も
見えず、カタログ進化で増えた ~130 edge の存在が evaluate 全通過まで発覚しなかった。
`evaluate; check` と独立実行し、check の unresolved 一覧を常に観測すべき。
