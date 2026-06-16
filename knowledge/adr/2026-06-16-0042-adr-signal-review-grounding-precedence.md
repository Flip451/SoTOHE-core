---
adr_id: 2026-06-16-0042-adr-signal-review-grounding-precedence
decisions:
  - id: D1
    user_decision_ref: "chat_segment:2026-06-16"
    status: accepted
  - id: D2
    user_decision_ref: "chat_segment:2026-06-16"
    candidate_selection: "from:[None正規化, 値オブジェクトでの構築時エラー] chose:値オブジェクトでの構築時エラー"
    status: accepted
  - id: D3
    user_decision_ref: "chat_segment:2026-06-16"
    status: accepted
  - id: D4
    user_decision_ref: "chat_segment:2026-06-16"
    status: accepted
---
# ADR decision 根拠信号機: review grounding が一件でもあれば 🟡 とする優先規則への修正

## Context

ADR decision の根拠 trace 信号機（`bin/sotp verify adr-signals`）は、各 ADR decision の根拠フィールド（`user_decision_ref` / `review_finding_ref`）から 🔵🟡🔴 を評価する。この機構は `2026-04-27-1234-adr-decision-traceability-lifecycle.md` の D1 で導入された。

しかし旧 D1 の信号表とその実装（`libs/domain/src/adr_decision/evaluator.rs` の `classify_grounds`）は `user_decision_ref` を先に判定するため、**`user_decision_ref` と `review_finding_ref` の両方を持つ decision が 🔵 青になり、review 由来の根拠が信号上は無視される**。doc コメント（`grounds.rs`）とテスト `test_evaluate_adr_decision_user_ref_takes_priority_over_review_ref` も「user 優先」を固定している。さらに同じ旧 D1 の文面は convention `knowledge/conventions/adr.md` の YAML front-matter 表（`review_finding_ref` 行「`user_decision_ref` 未設定なら 🟡」）にも反映されている。

本来意図していた仕様は「**review による grounding が一件でもあれば 🟡**」であり、現挙動はこれと正反対（黄になるべきものが青）である。

この優先規則は、もう一方の信号機である spec → ADR 信号 `evaluate_requirement_signal`（`libs/domain/src/spec.rs`）と整合させるべきである。そちらは ADR `2026-04-19-1242` §D3.1 に基づき「**`informal_grounds` が一件でもあれば `adr_refs` の有無に関係なく 🟡**」を既に実装している。両信号機は「未昇格の弱い根拠が一件でも残るなら注意色（🟡）に留める」という共通原理を持つべきで、ADR decision 信号機だけがこれを破っている。

あわせて、根拠フィールドには**型安全性の欠落**がある。`user_decision_ref` / `review_finding_ref` は `AdrDecisionCommon`（`common.rs`）上で生の `Option<String>` として保持され、**grounding 参照文字列に対応するドメイン値オブジェクト（newtype）が存在しない**。`AdrDecisionCommon::new` は `id` の非空のみ検証し参照文字列の中身を検証しないため、空文字 `""` が `Some("")` となり `is_some()` を満たして信号を誤誘発する。同種の参照（`AdrAnchor` / `SpecRef` / `InformalGroundSummary` / `ConventionRef`）は既に `plan_ref` 配下で `try_new` が空・空白を拒否する検証付き値オブジェクトになっているのに、ADR decision の grounding 参照だけ生 `String` のままで、厳格さが非対称である。

関連:

- `knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md` — 信号機機構の初出。本 ADR が D1 を supersede する。
- `knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md` §D3.1 — spec → ADR 信号の「informal 一件でも 🟡」原理。整合先。
- `knowledge/conventions/adr.md` — ADR front-matter 仕様。grounds 表が本決定で更新対象になる。

## Decision

### D1: ADR decision 根拠信号機の優先順位を review 優先へ反転する（旧 2026-04-27-1234 D1 を全面 supersede）

各 decision の信号は次の順序で評価する。`review_finding_ref` が `user_decision_ref` より優先される。

| 優先 | 条件 | 信号 |
|---|---|---|
| 1 | `grandfathered: true` | _(スキップ — 評価対象外)_ |
| 2 | `review_finding_ref` あり（`user_decision_ref` の有無を問わず） | 🟡 黄 |
| 3 | `user_decision_ref` あり かつ `review_finding_ref` なし | 🔵 青 |
| 4 | いずれの根拠もなし | 🔴 赤 |

- 🔵 青の意味を「ユーザーが明示承認し、**かつ未解決の review 由来根拠が残っていない**」に限定する。
- 🟡 黄の意味を「review 由来の根拠が残る（正式なユーザー承認への昇格が未完）」とする。両方の ref を持つ decision はこの状態に該当するため 🟡 に畳む。
- 🔴 赤が CI を block する挙動、`grandfathered: true` が評価対象外となる挙動（最上位スキップ）は**不変**。
- 本 D1 は旧 `2026-04-27-1234` D1 の信号表を取り消し、現行の唯一の真とする。

### D2: 根拠 ref を検証付きドメイン値オブジェクトにし、空・空白を構築時エラーにする

`user_decision_ref` / `review_finding_ref` の grounding 参照文字列に対応する**ドメイン値オブジェクト（newtype）を domain 層に新設**し、その fallible な構築関数（`try_new`）で空文字・空白のみを `Err`（`ValidationError`）として弾く。`AdrDecisionCommon` は当該フィールドを `Option<String>` ではなく `Option<該当 newtype>` で保持する。

- 現状: `AdrDecisionCommon` の `user_decision_ref` / `review_finding_ref` は生の `Option<String>` で、対応する値オブジェクトが存在しない。これ自体が型安全性の欠落であり、検証を内在させる先（値オブジェクト）が無いことが根本問題。まず値オブジェクトを新設する。
- 空 → `None` への正規化は**採らない**。silent に不正入力を受理すると「空でない参照」という不変条件を型で保証できず、Make Illegal States Unrepresentable に反するため。空・空白は fail-closed で**エラー**にする。
- 目的: 空の placeholder（`Some("")`）が `is_some()` を満たして信号を誤誘発するのを型レベルで排除する。D1 の反転後は、空の `review_finding_ref` が誤って 🟡 を誘発したり、空の `user_decision_ref` が 🔵 を偽装したりするのを防ぐ。
- infrastructure の DTO→domain 変換（`decision_dto_to_entry` / `AdrDecisionCommon` 構築）で値オブジェクトの構築エラーを `AdrFrontMatterCodecError::InvalidDecisionField` として伝播させ、`verify adr-signals` を fail させる（空・未記入 placeholder の slip-through を防ぐ fail-closed、CN-04）。
- 既存の `InformalGroundSummary::try_new` / `AdrAnchor::try_new` と同じ「空・空白 reject」パターンに揃える。
- newtype を user / review で共有する単一型（例: `DecisionGroundRef`）にするか、`UserDecisionRef` / `ReviewFindingRef` の 2 型に分けるかは Phase 2 type-design で確定する。バリデーション規則は共通（非空・非空白。必要に応じ single-line 制約も検討）。

### D3: 旧 ADR 2026-04-27-1234 の D1 を取り消し（supersede）処置する

旧 `2026-04-27-1234-adr-decision-traceability-lifecycle.md` の D1 を lifecycle 上 supersede する。

- 旧 D1 の front-matter を `status: superseded` に遷移させ、`superseded_by: 2026-06-16-0042-adr-signal-review-grounding-precedence.md#D1` を付帯する。
- 旧 D1 の MD body は歴史 record として残す（post-merge ADR は immutable record）。信号機の現行定義は本 ADR の D1 が担う。
- これは partial supersession であり、旧 ADR の D2（decision 個別 status）/ D3（MD + YAML front-matter フォーマット）/ D4（grandfathered による gradual back-fill）には触れない。それらは本決定の対象外で有効なまま。
- 旧 ADR の front-matter 編集は 1 ファイル 1 writer 原則に従い adr-editor 経由で行う。

### D4: 本修正で 🔵→🟡 に反転する既存 decision は一件ずつユーザーが内容確認し grounding を確定する

D1 の優先規則反転により、`user_decision_ref` と `review_finding_ref` を両方持つ既存 decision は 🔵 → 🟡 に変わる。これらを実装時に機械的に一括反転させず、**該当 decision を 1 件ずつユーザーが内容確認し、正しい grounding を与える処理**を行う。

- 各 decision について、ユーザーが「本当に review 由来の未昇格根拠として 🟡 が妥当か」「実態はユーザー承認済みで `review_finding_ref` は付随情報に過ぎず 🔵 を維持すべきか（その場合は `review_finding_ref` を除く等で grounding を整える）」を個別に判断する。
- 一括自動反転（blanket auto-flip）は採らない。信号は「その decision の根拠の由来」を表すため、移行で取り違えると ADR の来歴記録が歪む。人手の adjudication を必須とする。
- 対象（本 ADR 起票時点で新ルール適用により 🔵→🟡 になる both-ref decision、計 6 件 / 3 ファイル）:
  - `2026-05-29-1118-semantic-dup-detection-discoverability-gate.md`: D1 / D2 / D3 / D4
  - `2026-05-18-1223-make-catalogue-schema-permissive.md`: D3
  - `2026-05-26-1813-track-id-default-active-track.md`: D6
- この per-decision 確認・grounding 付与は本 ADR 実装 track の code/artifact 変更には含めず、別途の手動 follow-up として扱う（既存 ADR の front-matter 編集が必要になった場合は adr-editor 経由、1 ファイル 1 writer 原則）。

## Rejected Alternatives

### A. 旧仕様（user 優先）を維持し、ADR 文面と convention のみ現実装に合わせる

ユーザーの意図（review 一件でも 🟡）に反する。さらに spec → ADR 信号（informal 一件でも 🟡）と優先規則が逆のまま残り、2 つの信号機の設計が非一貫になる。「未昇格の弱い根拠が残るなら注意色」という共通原理を ADR 信号機だけが破る状態を温存するため却下。

### B. 「user と review の両方あり」を第 4 の信号（別バリアント / 別色）として表現する

信号機は 🔵🟡🔴 の 3 値モデルが固定の設計前提であり、verify 出力・CI 判定・各種レンダリングがこの 3 値に依存している。色を増やすと複雑度が上がり、「未解決の根拠が一件でも残れば 🟡」という単純な不変条件を壊す。両方あり＝review 由来の未昇格根拠が残る状態として 🟡 に畳むのが整合的なため却下。

### C. 空 ref を `None` に正規化する（silent normalization）

空文字・空白を黙って `None` に畳む案（本 ADR 初稿の D2 案）。不正入力を暗黙に受理するため「空でない参照」という不変条件を型で保証できず、Make Illegal States Unrepresentable に反する。空 placeholder が記入漏れか意図的な無根拠かを区別せず握り潰すため、未記入の検出力も弱い。fail-closed の構築時エラー（D2）を採用するため却下。

### D. 値オブジェクトを導入せず `AdrDecisionCommon::new` に inline の非空チェックだけ足す

newtype を作らず構築関数内で空チェックする案。検証が構築経路に閉じて再利用できず、他の grounding 参照利用箇所で同じ検証を再実装するリスクが残る。`prefer-type-safe-abstractions.md` の newtype パターン（検証を型に内在させる）に沿わないため却下。

## Consequences

### Positive

- ユーザーの意図（review grounding が一件でもあれば 🟡）と実装が一致する。
- ADR decision 信号機と spec → ADR 信号機の優先規則が「未解決の弱い根拠が残れば注意色」で一貫する。
- 空・空白 ref による誤信号（黄→青 / 青の偽装）を、値オブジェクトの不変条件として型レベルで排除する。
- grounding 参照に値オブジェクトが導入され、`plan_ref` 配下の他の参照型と型安全性の粒度が揃う。

### Negative

- 実装更新が必要: `classify_grounds` の判定順反転（`evaluator.rs`）、doc コメント（`grounds.rs`）、テスト `test_evaluate_adr_decision_user_ref_takes_priority_over_review_ref` の期待値反転、grounding 参照のドメイン値オブジェクト新設（`try_new` で空・空白を `Err`）、`AdrDecisionCommon` のフィールド型を `Option<String>` → `Option<newtype>` に変更、DTO→domain 変換でのエラー伝播追加、検証テスト（空・空白で `Err` / 正常値で `Ok`）の追加。
- convention `knowledge/conventions/adr.md` の YAML front-matter 表（`review_finding_ref` 行）を「review あり → 🟡（`user_decision_ref` の有無を問わず）」に更新する必要がある。
- D1 の反転により `user_decision_ref` と `review_finding_ref` を両方持つ既存 decision が 🔵 → 🟡 に変わる。**本 ADR 起票時点で該当は 6 件（3 ファイル、D4 に列挙）**。`verify adr-signals` 上は 🟡 は warning（CI を block しない）として可視化される。来歴の正確性のため、D4 に従い一件ずつユーザーが確認して grounding を確定する。

### Neutral

- `grandfathered` の最上位スキップは不変。
- 3 値信号モデル（🔵🟡🔴）は不変。
- 旧 ADR `2026-04-27-1234` の D2 / D3 / D4 は不変で、本 ADR の supersede 対象外。

## Reassess When

- briefing 機械生成（WF-67 Phase B）が完成し、orchestrator 独断 decision の発生頻度が大きく下がった場合、信号機の厳格さ（🔴 block / 🟡 警告）の意義を再評価する。
- spec → ADR 信号 `evaluate_requirement_signal` の優先規則が変わった場合、2 信号機の整合前提が崩れるため本 ADR を再評価する。
- 信号機の色数モデル（🔵🟡🔴 の 3 値）を変更する別 ADR が出た場合。
- `user_decision_ref` と `review_finding_ref` を両方持つ decision が実運用で多数現れ、両者の優先を文脈依存で切り替えたい要求が生じた場合。

## Related

- `knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md` — 本 ADR が D1 を supersede する旧 ADR（信号機機構の初出）。
- `knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md` — spec → ADR 信号（informal 一件でも 🟡）の根拠 ADR。優先規則の整合先。
- `knowledge/conventions/adr.md` — ADR front-matter 仕様。grounds 表が本決定で更新対象。
- `.claude/rules/04-coding-principles.md` / `knowledge/conventions/prefer-type-safe-abstractions.md` — 検証付き値オブジェクト（newtype）パターン。D2 の根拠。
- `knowledge/adr/` — ADR 索引。
