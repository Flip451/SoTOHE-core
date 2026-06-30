---
adr_id: 2026-06-30-1549-per-sot-review-scope
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session_01QZTawCUgs9Vjbb6mceJKp3:2026-06-30"
    candidate_selection: "from:[A-monolith,B-views-operational,C-per-layer-types] chose:per-sot-4-scope"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session_01QZTawCUgs9Vjbb6mceJKp3:2026-06-30"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session_01QZTawCUgs9Vjbb6mceJKp3:2026-06-30"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:session_01QZTawCUgs9Vjbb6mceJKp3:2026-06-30"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:session_01QZTawCUgs9Vjbb6mceJKp3:2026-06-30"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:session_01QZTawCUgs9Vjbb6mceJKp3:2026-06-30"
    status: proposed
---
# 内容レビューの SoT 別スコープ化

## Context

SoTOHE は SoT chain（ADR → `spec.json` → `<layer>-types.json` → `impl-plan.json` / `task-coverage.json` / `task-contract.json` → source）を phase ごとに分離し、各 phase に専用の writer capability と gate を持たせている。SoT 境界を意識した検証軸は既に複数存在する:

- **signal**（🔵🟡🔴）: chain⓪ adr-user / ① spec-adr / ② catalog-spec / ③ impl-catalog と phase 別に「参照の存在」を機械判定する。
- **ref-verify**: chain1（spec → ADR）/ chain2（catalogue → spec）とエッジ別に、専用プロンプトで「参照の意味整合」を LLM 判定する。
- **task-contract**（PreReviewGate）: 実装計画の遂行と型契約の履行が同時に進むことを強要する。
- **rollback-diagnoser**: finding の起源を `adr` / `spec` / `type` / `impl_plan` / `impl` の 5-class に分類し、対応する phase writer へ差し戻す。

このように検証・差し戻しの各軸は SoT/phase/エッジ境界を一級市民として扱っている。ところが**内容レビュー**（reviewer capability の scope, `review-scope.json`）だけは非対称な状態にある。ソースコードは `domain` / `usecase` / `infrastructure` / `cli` / `cli_composition` / `cli_driver` と layer 別に分割され各々が専用 briefing（severity policy）を持つのに対し、上流 SoT（ADR / `spec.json` / `<layer>-types.json` / Phase 3 artifacts）はすべて `plan-artifacts` 単一スコープ（`2026-04-18-1354-review-scope-prompt-injection.md` が新設）に一括され、severity policy も `plan-artifacts.md` 1 枚に留まる。

この非対称により、SoT ごとに本質的に異なる評価観点 ── 振る舞い契約の十分性（spec）、型設計の健全性（types）、タスク分解の実行可能性（impl-plan）、意思決定の妥当性（ADR）── を、最大公約数の薄い共通 policy で見ざるを得ない。

## Decision

### D1: 内容レビュースコープを SoT 別の 4 スコープに分割する

`review-scope.json` の `plan-artifacts` 単一スコープを廃止し、上流 SoT を `adr` / `spec` / `types` / `impl-plan` の 4 スコープに分割する。目的は、各スコープが当該 SoT 固有の評価観点（severity policy / briefing）を持てるようにすることであり、signal・ref-verify・task-contract・rollback-diagnoser が既に持つ SoT 境界認識に内容レビュー軸を揃える。

### D2: 各スコープのファイル割当

| スコープ | 割当ファイル |
|---|---|
| `adr` | `knowledge/adr/**`, `knowledge/research/**` |
| `spec` | `track/items/<track-id>/spec.json`, `spec.md` |
| `types` | `track/items/<track-id>/*-types.json`（全 layer 一括）, `contract-map.md` |
| `impl-plan` | `track/items/<track-id>/impl-plan.json`, `task-coverage.json`, `task-contract.json`, `plan.md`, `observations.md` |

`knowledge/research/**`（planner 調査ノート）は ADR の設計背景・根拠調査として `adr` スコープに同梱する。`metadata.json`（identity-only）と `<layer>-types.md`（型カタログの Markdown 化ビュー）は、レビュー価値が薄い / SSoT と重複するため `review_operational` に退避しレビュー対象外とする。`track/items/<track-id>/` 配下のうち上記スコープにも `review_operational` にも該当しないファイルは暗黙の `other` スコープに落ちるため、移行時に網羅性を検証する。

### D3: 生成ビューは可読性に資するものを対応 SoT スコープに同梱する

`spec.md` を `spec` スコープに、`plan.md` を `impl-plan` スコープに、`contract-map.md`（全層カタログを統合した型間関係の mermaid ビュー）を `types` スコープに含める。これらは SSoT からの read-only render であり二重レビューの懸念があるが、レビュワーが SSoT と render を同じスコープで併読できる可読性上の利益を優先する。一方 `<layer>-types.md` は対応する `<layer>-types.json` の機械的な Markdown 化で付加情報が乏しいため同梱せず、`review_operational` に退避する（D2）。

### D4: types スコープは当面 layer 一括とする

`domain-types.json` / `usecase-types.json` / … を `types` 単一スコープに束ねる。layer 別分割（ソースの layer 分割や signal chain② / task-contract の layer 別構造と対応させる案）は、必要性が実証されるまで行わず Reassess に回す。

### D5: 各スコープに専用 briefing を新設し、plan-artifacts.md を分解する

`.harness/custom/review-prompts/` に `adr.md` / `spec.md` / `types.md` / `impl-plan.md` を新設し、既存 `plan-artifacts.md` の severity policy を各 SoT の評価観点に分解・特化させる。各 briefing は当該 SoT 固有の観点を持つ。特に `types.md`（型カタログ）は、SOLID / CQRS / DRY などの一般コーディング原則に照らして型設計をレビューする旨を含める。

### D6: plan-artifacts を参照する既存箇所を新スコープ名へ移行する

`plan-artifacts` を前提とする箇所 ── full-cycle の lifecycle tail commit（`--scope plan-artifacts`）、rollback-diagnoser のトリガー記述、`fixpoint_resolve.rs` / `track_phase.rs` のテストフィクスチャ等 ── を新スコープ構成へ同時更新する（maintainer-checklist 整合）。

## Rejected Alternatives

### A. plan-artifacts 一括スコープを維持する（現状維持）

上流 SoT を 1 スコープのまま据え置く案。実装コストはゼロだが、(1) 評価観点を SoT ごとに変えられず薄い共通 policy に留まる、(2) スコープ内のどれか 1 ファイルの変更で scope 全体の content-hash が `StaleHash` になり、spec の 1 行修正でも types / impl-plan / ADR の承認まで再レビューに引きずられる（部分再レビューが効かない）。本 ADR が解こうとしている非対称そのものを温存するため却下。

### B. 生成ビュー（spec.md / plan.md）を operational に退避する

生成ビューは SSoT からの render なので二重レビューを避けるべく `review_operational` に逃がす案（当初の検討案）。理屈は通るが、レビュワーが JSON だけを読むより render された Markdown を同じスコープで併読する方が可読性が高いという実利を優先し、却下（D3 を採用）。

### C. types を最初から layer 別に分割する

`domain-types` / `usecase-types` / … を個別スコープにする案。ソースの layer 分割や signal chain② / task-contract の layer 別構造と綺麗に対応するが、スコープ数と briefing 保守コストが先行して増える。当面は `types` 一括で運用し finding 傾向を見て判断するため見送り（Reassess に回す、D4）。

## Consequences

### Positive

- 各 SoT 固有の評価観点を briefing に持てる（振る舞い契約 / 型設計 / タスク分解 / 意思決定の妥当性）。薄い共通 policy による誤検出・見逃しを減らせる。
- 部分再レビューの局所性が効く。content-hash が scope 単位なので、変更した SoT のスコープだけ再レビュー対象になり、他 SoT の承認は維持される。
- 内容レビュー軸が signal / ref-verify / task-contract / rollback-diagnoser と同じ SoT 境界認識に揃い、設計の対称性が回復する。
- plan-artifacts 肥大による diff 増を緩和し、per-scope 独立並列レビューとの相性が良い。

### Negative

- briefing ファイルが 1 枚（plan-artifacts.md）から 4 枚（adr/spec/types/impl-plan）へ増え、保守対象が増える。
- reviewer 呼び出し（Codex, 課金）の回数が増えうる。ただし走るのは変更のあった SoT のみで、空スコープは `NotRequired(Empty)` で自動 skip されるため、増分は変更範囲に比例する。
- plan-artifacts を参照する複数箇所（full-cycle / rollback-diagnoser / テスト）の同時更新が必要で、移行時の整合リスクがある。
- `track/items/<track-id>/**` の包括 glob を明示列挙に置き換えるため、列挙漏れファイルが `other` スコープに落ちる可能性がある（移行時の網羅性検証が必要）。

### Neutral

- types は当面 layer 一括であり、layer 別の解像度は将来の判断に委ねる。

## Reassess When

- `types` スコープの finding が層をまたいで頻発し、layer 別の評価観点が必要になったとき（→ D4 を見直し layer 別分割へ）。
- SoT 分割による reviewer 呼び出しコスト（Codex 課金 / wall-clock）が無視できない負担になったとき。
- SoT 別 briefing の評価観点が実運用で薄く、共通化した方が誤検出・見逃しが減ると判明したとき。
- track 内に新種の SoT 成果物が増え、4 スコープへの割当方針や `other` 落ちの扱いを見直す必要が出たとき。

## Related

- `knowledge/adr/2026-04-18-1354-review-scope-prompt-injection.md` — `plan-artifacts` スコープと scope 別 briefing 注入機構を新設した ADR。本 ADR はその plan-artifacts を SoT 別に分割する。
- `knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md` — layer 別 reviewer briefing prompt 導入の先例。
- `knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md` — scope 独立型レビュー（per-scope 独立並列・StaleHash）の基盤。
- `knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md` — ref-verify chain1/chain2 による SoT エッジ別検証（SoT 境界認識の先例）。
- `knowledge/conventions/workflow-ceremony-minimization.md` — 事後レビュー方式 /「file 存在 = phase 状態」原則。
