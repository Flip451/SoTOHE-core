---
adr_id: 2026-03-24-0930-adr-auto-derivation-design
decisions:
  - id: 2026-03-24-0930-adr-auto-derivation-design_grandfathered
    status: accepted
    grandfathered: true
---
# ADR 自動導出: SSoT ファイルから判断候補を検出する設計

## Status

Accepted (設計のみ。実装は knowledge/strategy/ 移動 + CC-SDD-01 完了後)

## Context

ADR の記録が人間の記憶に依存している。設計判断が発生しても「ADR に記録して」と依頼しなければ記録されない。

会話テキストの NLP パターンマッチで自動検出する案（hook C 案）は false positive が避けられない。

## Decision

**会話ではなく SSoT ファイルの書き込みを監視する。** 設計判断が重要であれば、必ず SSoT ファイルに痕跡を残す。痕跡がない判断は ADR の対象外（手動 `/adr:add` で補完）。

### スキャン対象と検出パターン

| SSoT ファイル | 検出パターン | ADR 候補の種類 |
|--------------|-------------|---------------|
| `spec.json` `out_of_scope` | 各項目 = 「やらないと決めた」判断 | Deferred / Rejected |
| `spec.json` `constraints` | 各項目 = 「制約として受け入れた」判断 | Constraint acceptance |
| `knowledge/strategy/TODO-PLAN.md` | 「スキップ」「先送り」「延期」パターン | Deferred |
| `knowledge/strategy/TODO-PLAN.md` | Phase 間の依存関係変更 | Strategic reorder |
| `.claude/docs/DESIGN.md` Key Design Decisions | ADR リンクがない行 | Missing ADR |
| `sotp review resolve-escalation --decision` | 解決判断の記録 | Escalation resolution |

### 実装方針: `sotp adr suggest`

```
$ sotp adr suggest

ADR candidates found:

[spec.json out_of_scope] req-task-traceability-2026-03-24
  "未宣言遷移の検出 (code→spec 逆方向チェック)"
  → Deferred to Phase 3 (source: inference — Phase 3 spec ↔ code 整合性で対応)
  → Suggested: knowledge/adr/YYYY-MM-DD-HHMM-deferred-reverse-transition-detection.md

[TODO-PLAN] Phase 2
  "2-6 SSoT-07 二重書き込み解消 — スキップ"
  → Already has ADR: No
  → Suggested: knowledge/adr/YYYY-MM-DD-HHMM-ssot07-skipped.md

[DESIGN.md] Row without ADR link
  (none found — all rows linked)

Total: 2 candidates, 0 already covered
```

### CI 統合

- `sotp verify adr-coverage` として CI に追加
- 未カバーの判断候補が N 件以上 → warning（error にはしない。判断の全てが ADR に値するわけではない）
- ADR 不要と判断した候補は `adr-ignore.json` で除外

### `/adr:add` による手動補完

SSoT に痕跡を残さない ad-hoc 判断（会話内のみの判断）は `/adr:add <slug>` で手動作成。索引は `conventions-update-index` と同パターンで自動更新。

## Rejected Alternatives

- **会話テキスト NLP（hook C 案）**: 「却下」「選んだ理由」等のキーワード検出。false positive が不可避。設計判断と日常的な選択（「この変数名にした」等）の区別が困難
- **PostToolUse[Write] で全ファイル監視**: スキャン対象が広すぎる。コード書き込みでも発火し、false positive の温床
- **ADR 必須化（全判断を ADR 化）**: 過剰。軽微な判断まで ADR にするとノイズが増え、重要な判断が埋もれる

## Consequences

- Good: false positive ゼロ（SSoT の構造化フィールドのみスキャン）
- Good: 既存の SSoT パターン（spec.json, TODO-PLAN）を活用。新しいデータモデル不要
- Good: CI で「カバーされていない判断」を可視化。記録漏れを構造的に防止
- Bad: SSoT に書かれない判断は検出できない（`/adr:add` で補完が必要）
- Bad: TODO-PLAN が Markdown なのでパースが必要（spec.json のような構造化 SSoT ではない）

## Prerequisites (実装前に必要)

1. `knowledge/strategy/` 移動 — tmp/ (gitignore) の戦略文書を git 管理下に
2. CC-SDD-01 完了 — spec.json の task_refs 構造が確定してからスキャンパターンを設計

## Reassess When

- `knowledge/strategy/` 移動 + CC-SDD-01 完了後に実装着手を判断
- TODO-PLAN を構造化 JSON（metadata.json パターン）に移行する場合、パース方式を見直し
- LLM の structured output が十分信頼できるようになった場合、会話テキスト検出を再検討
