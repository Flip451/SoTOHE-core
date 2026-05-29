---
adr_id: 2026-05-29-1118-semantic-dup-detection-discoverability-gate
decisions:
  - id: D1
    review_finding_ref: "deep-research-2026-05-29:semantic-dup-detection"
    user_decision_ref: "chat_segment:adr-add:2026-05-29:semantic-dup-detection"
    candidate_selection: "from:[discoverability-first+soft-gate,hard-block-gate] chose:discoverability-first+soft-gate"
    status: proposed
  - id: D2
    review_finding_ref: "deep-research-2026-05-29:semantic-dup-detection"
    user_decision_ref: "chat_segment:adr-add:2026-05-29:semantic-dup-detection"
    candidate_selection: "from:[caseA-fastembed-jina-lancedb,caseB-nomic-7b,external-api] chose:caseA-fastembed-jina-lancedb"
    status: proposed
  - id: D3
    review_finding_ref: "deep-research-2026-05-29:semantic-dup-detection"
    user_decision_ref: "chat_segment:adr-add:2026-05-29:semantic-dup-detection"
    status: proposed
---
# コード意味重複検出による DRY 防止（discoverability + soft gate）

## Context

2026-05-29 の DRY/SOLID/CQRS 監査で、コードベース全体に「意味的に同じことをするコード片の重複」が広く存在することが判明した（例: `is_valid_rust_identifier` の二重定義と挙動乖離、verify サブシステムの5つの重複ファイルウォーカー、reviewer アダプタ2つにわたる5関数の二重実装）。これらの根本原因は「既存のヘルパや型を発見できないまま再実装してしまう」ことであり、`architecture-rules.json` の `canonical_modules` のような grep ベースの禁止は症状治療にとどまり、意味レベルの重複を捉えられない。

そこで「コード片の意味を検索可能なデータベース（意味DB）に保管し、コード追加時に意味的に重複する既存コード片の有無を事前確認する」仕組みを検討した。**ローカル完結（外部 embedding API / クラウドサービスに必須依存しないこと）**を必須制約とし、2024〜2026年の技術動向を deep-research で調査した。確認できた要点:

- (a) **スタックは構成可能**: Rust ネイティブの埋め込み推論（`fastembed-rs` + ONNX Runtime、Tokio 非依存・同期API）と、ローカルファイルで動くベクトルDB（`LanceDB`, Apache 2.0, 公式 Rust SDK）を組み合わせれば、外部 API 依存ゼロのローカル構成が作れる。
- (b) **精度が壁**: Type-4（構文は違うが意味が同じ）クローン検出は 2024〜2026 でも未解決の研究課題で recall が低い。Rust 特化のコード埋め込みモデルは存在せず、汎用モデル（Jina v2 base code 等）の Rust への転移品質は未検証。
- (c) **非決定性**: FP32 推論は理論上決定的だが、ANN ライブラリ（USearch 等）は検索結果の決定性を文書保証しない。再現可能 CI と緊張する。
- (d) **形骸化リスク**: 誤検出が多い状態でハードブロックを課すと、開発者が override（ack）を機械的に押す rubber-stamp 化が起きる。

本 ADR は、この不確実性を踏まえた上で「意味DBによる重複防止をどう進めるか」の方向性を定める。スコープは方向性・スタック・段階導入であり、ハードゲート化の是非は実測まで保留する。

## Decision

### D1: discoverability を主、soft gate を従とする

意味重複防止を「コードを書く前に意味的に類似する既存コード片を提示する discoverability 補助」を主軸とし、CI/pre-commit での警告（soft gate）を従とする方針で追求する。最初からハードブロック型の強制ゲートは作らない（理由は Rejected Alternatives A）。根本原因（既存実装を発見できず再実装する）を「発見させる」ことで直接叩く。

### D2: ローカル完結を必須制約とし、案A を第一候補とする

意味DBスタックは**外部 embedding API / クラウドサービスに必須依存しないこと**を必須制約とする。第一候補（案A）は `fastembed-rs`（ONNX Runtime 経由・同期API・Tokio 非依存）× コード埋め込みモデル `Jina v2 base code`（約137M, ~550MB）× `LanceDB`（ローカルファイルDB, Apache 2.0, 公式 Rust SDK）。`run --rm` の再現可能 CI と依存最小方針に適合させる。重量モデル（7B 級）や外部 API は採らない（Rejected Alternatives B/D）。

### D3: 段階導入とし、ハードゲート化は実測まで保留する

導入は段階的に行う。(1) まず discoverability（例: `sotp find-similar` 相当の、類似既存フラグメント top-k を提示する情報提供のみのサブコマンド）。(2) PoC で Jina の Rust コードへの転移品質（cosine 類似度分布・false positive 率）を実測する。(3) 実測後に、追加・変更された差分フラグメントのみを対象とする soft gate（warning 止まり・ack 付き override 可。`module_size` 検証が warning 止まりである前例と整合）。(4) ハードブロック化は、実測で十分な精度が確認できるまで保留し、本 ADR では判断しない。

## Rejected Alternatives

### A. 最初からハードブロック型の強制ゲートを作る

却下する。Type-4 意味クローン検出は未解決で recall が低く、誤検出が多い状態でのハードブロックは開発者の rubber-stamp 化を招く。さらに ANN の非決定性が再現可能 CI と緊張する。精度を実測してからゲートの強度を決めるべきで、soft gate から始める。

### B. 外部 embedding API / クラウドサービスに依存する

却下する。ローカル完結（外部 API 非依存）という必須制約に反する。再現可能 CI・依存最小・オフライン動作の方針とも相容れない。

### C. canonical_modules の grep 禁止だけで対処する

却下する。`forbidden_patterns` による grep 禁止は症状治療で、関数名の文字列照合にすぎず回避が容易で、意味（振る舞い）レベルの重複を捉えられない。なお `canonical_modules` 機構自体の要否は別トピックで concern ごとに評価する。

### D. 重量級モデル（nomic-embed-code 7B 等）を採用する

却下する。nomic-embed-code は対応言語に Rust を含まず、7B（約14GB）で再現可能 CI 環境のメモリ・推論コストに見合わない。軽量な案A の優位性が高い。

## Consequences

### Positive

- 「コードを書く前に既存の類似実装を提示する」ことで、再実装の occasion（機会）そのものを減らせる（根本原因への対処）。
- ローカル完結のため、再現可能 CI・依存最小・オフライン動作の方針と両立する。
- 段階導入（discoverability → PoC → soft gate）により、誤検出のリスクを制御しながら価値を確かめられる。
- 既存の TDDD（型レベルの意味の構造DB）と地続きで、関数/impl 本体レベルへ拡張できる。

### Negative

- 埋め込み推論とベクトルDBの新規依存（`fastembed-rs` / `ort` / `lancedb`）が増える。
- 埋め込みモデル重み（~550MB）のキャッシュ・配布管理が必要（ビルドキャッシュ同梱の要否は要確認）。
- Rust への転移品質が未検証のため、PoC コストが先行する。
- インデックス鮮度の管理と ANN の非決定性に対する運用上の工夫（しきい値・固定スナップショットのハッシュ管理・差分のみ判定）が要る。

### Neutral

- ハードゲート化の是非は本 ADR では保留する（実測後・別途）。
- `canonical_modules` 機構の要否評価は本 ADR では扱わない（別トピック）。

## Reassess When

- PoC で Jina の Rust 転移品質（cosine 分布・false positive 率）が実測でき、ハードゲート化の是非を判断できるとき。
- Rust 特化のコード埋め込みモデルが登場し、精度が大きく改善したとき。
- workspace 規模が拡大し、LanceDB（案A の採用スタック）の ANN インデックスが性能・運用上の限界に達したとき（別の ANN バックエンドや全スキャン型（sqlite-vec 等）との比較検討）。

## Related

- `knowledge/adr/` — ADR 索引
- `architecture-rules.json` — `canonical_modules` / `module_limits` の SSoT
- `knowledge/conventions/` — プロジェクト規約
