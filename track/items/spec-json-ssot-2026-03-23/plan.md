<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# spec.json SSoT 化 — spec.md を rendered view に降格

spec.md を metadata.json → plan.md と同じ SSoT パターンに移行する。
spec.json が SSoT、spec.md は render_spec() による read-only rendered view に降格。

解決する問題:
- Markdown パースの edge case (code block 内誤検出、frontmatter drift)
- Multi-source タグ未対応 (JSON 配列で自然解決)
- spec.md frontmatter signals のドリフト (rendered view で解消)
- spec_signals が metadata.json extra (untyped) に格納されている問題

## Domain 型定義

SpecDocument (aggregate), SpecRequirement (text + sources), DomainStateEntry, SpecScope, SpecSection を domain 層に定義。
Multi-source ポリシー: 各要件の信号 = sources 中の最高信頼度 (Blue > Yellow > Red)。sources 空 = Red。

- [x] Domain 型定義: SpecDocument, SpecRequirement, DomainStateEntry, SpecScope, SpecSection

## Infrastructure: codec + rendering

spec.json codec (SpecDocumentV1 serde) と render_spec() (spec.json → spec.md)。
sync_rendered_views に spec.md 生成を統合。

- [x] spec.json codec: SpecDocumentV1 serde (deserialize + validate + serialize)
- [x] render_spec(): spec.json → spec.md レンダリング
- [x] sync_rendered_views 統合: spec.md を plan.md と並行して生成

## 信号評価の構造化

SpecDocument から直接 SignalCounts を算出。Markdown パース不要。
sotp track signals を spec.json ベースに更新。

- [x] 構造化信号評価: evaluate_from_spec(doc: &SpecDocument) -> SignalCounts
- [x] sotp track signals 更新: spec.json 読み込み → 信号計算 → spec.json 書き戻し

## Verifier 移行

全 spec verifier を spec.json 対応に移行。旧 track 用 legacy fallback 付き。
spec-frontmatter → spec-schema にリネーム。

- [x] Verifier 移行 (1): spec-signals + spec-states を spec.json ベースに (legacy fallback 付き)
- [x] Verifier 移行 (2): spec-frontmatter → spec-schema + latest-track + spec-attribution 更新

## ワークフロー統合

/track:plan スキルが spec.json を生成するよう更新。ドキュメント・CI 確認。

- [x] /track:plan スキル更新 + ドキュメント + CI 確認
