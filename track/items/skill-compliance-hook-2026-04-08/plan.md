<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# WF-67: agent-router 廃止 + skill 遵守フック導入（Rust）

agent-router.py（intent 検出 + ルーティングヒント注入の Python フック ~850行）を廃止する。
代わりに sotp hook dispatch skill-compliance を Rust で新設し、/track:* コマンド検出時の SKILL.md フェーズ遵守リマインドと external guide injection を担う。
intent 検出・ルーティングヒントは rules + agent-profiles.json に委譲し、フックでは行わない。

## agent-router 廃止（クリーンアップ）

agent-router.py と test_agent_router.py を削除し、settings.json / orchestra.rs / DESIGN.md から参照を除去する。
test_agent_router.py は agent-router.py と同時に削除しないと pytest collection が失敗するため、原子的に削除する。
settings.json の UserPromptSubmit キーは一旦削除し、新フック実装後に再登録する。

- [x] settings.json — UserPromptSubmit の agent-router エントリを sotp hook dispatch skill-compliance に置換
- [x] agent-router.py + test_agent_router.py の削除（同時削除必須 — pytest collection 失敗を防ぐ）
- [x] orchestra.rs — EXPECTED_HOOK_PATHS から agent-router を除去
- [x] DESIGN.md — Python advisory hooks テーブルから agent-router を除去

## skill 遵守フック — domain 層

/track:* コマンドパターン検出と SKILL.md フェーズ要件リマインド生成を domain 層に配置する。
external guide マッチングロジック（guides.json keyword match）も domain 層に移植する。
TDD で Red → Green → Refactor サイクルを遵守する。

- [x] domain 層 — /track:* コマンドパターン検出ロジック（SkillComplianceCheck）の定義
- [x] domain 層 — SKILL.md フェーズ要件のリマインドメッセージ生成
- [x] domain 層 — external guide マッチングロジック（guides.json keyword match）の移植

## skill 遵守フック — infrastructure + CLI 層

guides.json の serde codec を infrastructure 層に配置する。
sotp hook dispatch skill-compliance サブコマンドを CLI 層に実装する。
settings.json と orchestra.rs に新フックを登録する。

- [x] infrastructure 層 — guides.json codec（serde deserialize + keyword matcher 実装）
- [x] CLI 層 — sotp hook dispatch skill-compliance サブコマンド実装
- [x] settings.json + orchestra.rs に新フック skill-compliance を登録

## テスト + CI 検証

domain/infrastructure/CLI 各層のユニットテストを作成する。
cargo make ci 全チェックを通過させる。

- [x] テスト — domain/infrastructure/CLI 各層のユニットテスト（TDD: Red → Green → Refactor）
- [x] cargo make ci 全チェック通過確認
