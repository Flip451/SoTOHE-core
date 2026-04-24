# No Backward Compatibility Convention

## Purpose

新しい schema / behavior / rule を導入するとき、archive 済み / completed track や旧 schema の成果物に
遡及適用しない。変更コストを active scope に限定することで、template プロジェクトとしての multi-track
workspace の lifecycle 整合性を保ち、暫定 compatibility layer の長期運用負担を避ける。

## Scope

- 適用対象: track 成果物 (metadata.json / spec.json / 型カタログ / impl-plan / task-coverage)、schema
  定義、CI gate の規則、codec フォーマット、CLI サブコマンドの interface
- 適用外:
  - 純粋なバグ修正 (semantic 同値だが実装誤りの訂正)
  - crate 公開 API の semver を保つ範囲の変更 (adopter 側の compile 互換を別規約で保証する場合)

## Rules

- **新 schema / behavior を導入する際、archive 済み / completed track には遡及適用しない**。対象は
  「新 schema の下で作業しうる active track」に限定する
- **non-active track (branch を持たない / merged 済み / archive 済み) は write で protect する**。
  filesystem / codec / CLI / active-track guard レベルで、write 経路自体が非 active を拒否する設計と
  する
- **active track は新 rule 即時適用**。grace period や opt-out を default に組み込まない (暫定
  migration toggle は Exceptions 参照)
- **migration 用の暫定 compatibility layer / alias / 旧 schema 読み込みは最小限**に抑える。一度導入
  した暫定 layer は撤去 trigger を Reassess When に明記する
- **ADR で schema / rule 変更を決定する際は、遡及非適用の方針を Consequences に明示**し、
  legacy track / 旧 schema への挙動を記録する

## Examples

- Good (rename, no alias): `TypeDefinitionKind::TraitPort` → `SecondaryPort` のリネーム。alias は
  作らず、active track の catalogue 宣言を一括書き換え
  (ADR `2026-04-11-0002-tddd-multilayer-extension` Phase 1)
- Good (schema version bump, reject old): `metadata.json` v4 → v5 で `status` field を削除、v4 は
  decode 拒否、legacy track は verify gate が skip することで共存
  (ADR `2026-04-19-1242-plan-artifact-workflow-restructure` Follow-up §D1.4)
- Good (adopter-scoped opt-in flag): `catalogue_spec_signal.enabled` は暫定 migration toggle ではなく、
  template 採用者が恒久的に選択する opt-in flag
  (ADR `2026-04-23-0344-catalogue-spec-signal-activation` §D5)
- Bad: 旧 schema の opt-in flag を permanent に保ち、新旧 2 つの code path が永続的に並存する
  (codec 保守 / test surface / documentation の負担が増え続ける)
- Bad: completed track の metadata を一律書き換えて新 schema に migrate する (non-active track
  protect の原則違反、archive 済み状態の改変)

## Exceptions

- **template 採用者向けの恒久的 opt-in flag** (`tddd.enabled`, `catalogue_spec_signal.enabled` 等) は
  migration 用の暫定 toggle ではなく「利用者が恒久的に選択する設計自由度」として残す。これは本
  convention の撤去対象外 (ADR 2026-04-11-0002 §D1 / 2026-04-23-0344 §D5)
- **security-critical な fix** (既存脆弱性の修正、認証境界の修正) は本 convention より優先し、必要なら
  非 active track にも遡及適用する。ただし遡及適用を行う際は、全 write guard 層 (filesystem / codec /
  CLI / active-track guard) を協調してバイパスする経路を別 ADR で定義してから実施する。バイパス機構は
  「security-critical fix 専用」の制約を明示した設計にし、汎用 admin 権限として開放してはならない。
  override ADR なしに write guard を迂回することは禁止
- **探索的 drafting 段階** の throwaway artifact は本 convention の対象外 (production merge 前の試行
  コードは自由に捨てて良い)

## Review Checklist

- [ ] 新 rule / schema の導入で archive / completed track を書き換えようとしていないか
- [ ] active track への適用に不必要な grace period / opt-out を default で設けていないか
- [ ] 暫定 compatibility layer / alias が ADR の撤去 trigger なしに積み残されていないか
- [ ] template 採用者向けの恒久 opt-in flag を「暫定 migration toggle」と混同していないか
- [ ] ADR Consequences で遡及非適用と legacy 挙動を明示しているか
- [ ] security-critical 遡及適用を行う場合、全 write guard 層のバイパス経路を定義した override ADR が先行して存在するか
- [ ] そのバイパス機構が「security-critical fix 専用」に設計されており、汎用 admin 権限として開放されていないか

## Decision Reference

- [knowledge/adr/README.md](../adr/README.md) — ADR 索引
- [workflow-ceremony-minimization.md](./workflow-ceremony-minimization.md) — 形式手順の最小化原則
  (本 convention と対になる、暫定 toggle の排除と方向性が一致)
- [pre-track-adr-authoring.md](./pre-track-adr-authoring.md) — ADR lifecycle (遡及 amendment は
  adr-editor back-and-forth で明示的に扱う)
