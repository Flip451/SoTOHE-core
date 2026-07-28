# Policy: No Upstream Restatement

## Purpose

impl-plan task text / plan.sections and `<layer>-types.json` docs/intent must reference
upstream ADR/spec via anchor, not restate it in prose.

下流 artifact が上流 (ADR / spec) の設計理由・挙動契約を散文で言い直すと、言い直しの数だけ
artifact 間矛盾の発生源が増える。挙動契約の本文は上流だけが持ち、下流は
「変更対象 + 操作 + anchor cite」で記述する。

## Scope

- 適用対象:
  - `impl-plan.json` の task text (`tasks[].description`)
  - `impl-plan.json` の `plan.sections[].description`
  - `<layer>-types.json` entry の `docs` / `intent` フィールド
- 適用外:
  - `spec.json` — ADR を細粒度の挙動契約に書き下すのが spec の本務であり、再記述禁止は適用しない
  - 本書の規律が導入される前に完了した track の artifact — 歴史的記録として原型を保ち、
    遡及的に書き直さない
  - workflow / capability ドキュメント (`.harness/workflows/` / `.harness/capabilities/` と、
    `.claude/commands/` / `.agents/skills/` 等のその provider adapter) — provider 非依存 logic の
    重複禁止 (adapter-SSoT 規則) が同種の懸念を既にカバーしている

## Rules

- 挙動は `AC-NN` / `IN-NN` / `CN-NN` 等の anchor cite で参照し、上流 (ADR / spec) の設計理由・
  挙動契約を散文で再説明しない。task text / plan section は
  「変更対象 (file / symbol) + 操作 + anchor cite」で完結させる

## Examples

- Good: 「`src/config/loader.rs` の `parse_config` に schema-version の fail-closed 検査を
  追加する。AC-03。」— 変更対象 + 操作 + anchor cite で完結している
- Bad: 上と同じ task text に続けて、fail-closed が必要な理由や検査の期待挙動を段落で
  再説明する (AC-03 の本文と食い違う余地が生まれる)

## Exceptions

- anchor が指す内容を識別するための短い名詞句の併記 (例: 「fail-closed 検査 (AC-03)」) は
  再記述に当たらない。禁止するのは設計理由・挙動契約の文単位以上の再説明

## Review Checklist

- [ ] task text / plan section が「変更対象 + 操作 + anchor cite」で書かれているか
- [ ] `<layer>-types.json` entry の `docs` / `intent` が上流の設計理由・挙動契約を散文で
      再説明していないか

## Related Documents

- [knowledge/adr/README.md](../../knowledge/adr/README.md) — 設計判断の索引（履歴を確認する必要がある場合）
- [.harness/custom/review-prompts/impl-plan.md](../custom/review-prompts/impl-plan.md) /
  [types.md](../custom/review-prompts/types.md) — 本書はこの 2 つの reviewer severity policy が
  持つ finding class とセットで review gate から強制される
