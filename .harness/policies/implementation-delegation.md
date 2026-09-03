# Policy: Implementation Delegation

## Purpose

source 編集を capability へ委譲するとき、設計で確定した型配置が実装で崩れるのを構造的に防ぐ。委譲時注入 → 実装後検証 → review 時検査の 3 段で、設計と実装の一貫性を保証する。

委譲する側が制約を渡さず、実装後にも review にも配置を検査する手順が無ければ、設計に無い層へ実装が流れても誰も検知できない。本 policy はその 3 つの穴を塞ぐ委譲元の実行責務を定義する。

## Scope

- 適用対象: source を編集する capability への委譲と、その成果物を受け取る review briefing
- 適用外: 委譲時点で編集を許す source が test code だけに限定されている委譲 (例: 既に enroll 済みの obligation に対して test binding を書く `implementer` 委譲)

適用外の根拠は R1 が注入する節の中身にある。`## Architecture Constraints` 節は型・trait impl をどの layer に置くかという配置決定の表であり、production source を 1 行も書けない委譲には決めるべき配置が最初から存在しないため、節は毎回空のまま形式だけが残る。

この境界は委譲時に許した編集範囲で決まり、結果として何を編集したかでは決まらない。production source を編集しうる委譲は適用対象であり、test code を併せて編集する委譲もそこに含まれる。また、その変更が型の追加・移動・trait impl の配置を伴うかどうかは適用可否に影響しない — 配置を動かさない変更にも R1 は働く。編集を許す source が test だけだと言い切れない委譲は適用対象として扱う。

layer の id、path、依存方向は `architecture-rules.json` が持つ。本 policy は具体的な layer 名を持たず、委譲元が対象 project の `architecture-rules.json` から読む。role と layer の対応規則は本 policy の所有ではない。

## Rules

### R0. 委譲時の context intake

委譲元は、track の進行・review 必要性・obligation 状態・catalogue 状態を、workflow が指定する CLI summary (`bin/sotp track resolve`、`bin/sotp track task-counts`、`bin/sotp track next-task`、`bin/sotp review results`、`bin/sotp test-obligation results`、`bin/sotp catalog check`、`bin/sotp ref-verify results`) と task briefing から取得する。これらを一次情報とし、`*-types.json`、`review.json`、bindings JSON、full sub-workflow texts、`Related Conventions` list を委譲開始時に一括で開かない。差分または blocker を調査する場合に限り対象 artifact body を開く。convention path は dispatcher が briefing に記載し、委譲元は `Related Conventions` list を列挙・読まず、delegated capability がその path を読む。

### R1. 委譲時に architecture 制約を注入する

source を編集する capability への依頼には `## Architecture Constraints` 節を必ず含める。節の内容は task briefing と CLI summary を一次情報とし、設計上の配置を確定する必要がある場合に限り、briefing が示す track の ADR と rendered plan view の該当部分から抽出して、次を明示する。

| 項目 | 抽出元 |
| --- | --- |
| 新規 trait / struct を置く layer | ADR の配置決定 |
| trait impl を置く layer | ADR の配置決定 |
| 呼び出しフロー | ADR / plan |
| 各 layer の crate / path と依存可能先 | `architecture-rules.json` |

依頼文には、ある layer が依存先 layer の logic を自層で再実装しないという制約を含める。

### R2. review 起動前に配置を検証する

実装完了後、review を起動する前に委譲元が次を実行する。

1. ADR が配置を指定した型・trait impl が、指定 layer の path 配下に存在することを確認する
2. `architecture-rules.json` が宣言する依存方向と層境界からの逸脱が入っていないことを確認する。特に、依存先 layer の port 実装を bypass する no-op 代替型と、依存先 layer の logic の自層での再実装 — どちらも依存 edge を増やさないため `check-layers` では捕まらない
3. `cargo make check-layers` を実行する

### R3. review briefing に検査項目を渡す

review briefing には `## Architecture Verification Checklist` 節を含め、少なくとも次を検査対象として渡す。

- ADR が配置を指定した型が指定 layer にあるか
- `architecture-rules.json` が宣言する依存方向と層境界に従っているか
- 依存先 layer の logic が呼び出し側 layer へ漏れていないか
- port 実装を bypass する no-op 代替が入っていないか (設計が明示的に許可した no-op 実装を除く)

### R4. review / PR finding の修正を委譲する

PR review で actionable finding が source または review-scope の編集を要求する場合、委譲元は finding ごとに `dispatch_mode: delegated-pr-finding`、comment、対象 path / line、track context、requested correction を含む focused briefing を作成し、対象 artifact の owner に委譲する。実装変更と implementer の boundary 内の通常の policy / documentation は `implementer` に委譲する。`spec.json` とその生成 view は `spec-designer` の `spec-design`、`<layer>-types.json` とその生成 view は `type-designer` の `type-design`、`impl-plan.json`、`task-coverage.json`、`task-contract.json`、`batch-plan.json` は `impl-planner` の `impl-plan` の通常 writer workflow に戻す。生成された plan view は sanctioned views-sync operation で更新する。writer-owned artifact を implementer の focused dispatch に入れてはならない。`review-fix-lead` は通常の `scope-review` 専用であり、wrapper が typed focused mode をサポートするまでは PR finding の transport として使用しない。source 編集を許す briefing には、R1 が定める `## Architecture Constraints` 節も必ず含める。委譲元が親コンテキストで修正を inline edit することを通常経路にしてはならない。

委譲先が修正を完了した後、writer-owned artifact の修正であれば完了した owner workflow を影響フェーズの dispatch とみなし、workflow SSoT の partial-reentry / post-routing descent でそのフェーズを再収束させてから downstream まで完了させる。生成された plan view は sanctioned views-sync operation で更新する。その後、委譲元は local review workflow を `zero_findings` まで収束させ、`commit` workflow を実行してから PR review を再実行する。委譲が失敗した場合だけ親の直接編集を recovery として行えるが、これは implementer-owned non-ADR finding に限る。writer-owned artifact はその owner workflow に戻し、親が inline edit してはならない。`knowledge/adr/*.md` の編集を要する finding は親も `review-fix-lead` も決して適用せず、review workflow SSoT の `ADR-scope repair lane` section に従って guardian lane へ route する。その lane の完了後も同じ local review の収束と `commit` workflow を経てから再レビューする。

### R5. 長時間処理の待機

委譲先 capability、workflow、または gate wrapper が長時間実行される場合、orchestrator は 1 回の blocking call として実行し、terminal result を 1 回だけ読む。host が call を background 化した場合も、1 回の完了通知後に result を読むだけとし、ログの polling、status probe の再実行、fire-and-forget launch を行わない。内部の loop / poll は呼び出された capability または workflow の責務である。`bin/sotp test-obligation evaluate` は orchestrator host の repair round 内でだけ同期実行し、委譲元が commit prerequisite として launch してはならない。commit gate は `check` を使う。

## Enforcement

| 検証手段 | タイミング | 自動化 |
| --- | --- | --- |
| 依頼文の `## Architecture Constraints` 節 | 委譲時 | 委譲元が手動 |
| 型配置の検索確認 | 実装後 | 委譲元が手動 |
| `cargo make check-layers` | 実装後 + CI | 自動 |
| review briefing の `## Architecture Verification Checklist` 節 | review 時 | 委譲元が手動 |

## Anti-patterns

| パターン | 問題 | 対処 |
| --- | --- | --- |
| no-op 代替型 + 呼び出し側 layer への直接実装 | 依存先 layer の bypass | 本来の layer に adapter を実装する |
| 依存先 layer の計算・判定を呼び出し側 layer で再実装 | logic 漏洩 | その logic を所有する layer の型へ委譲する |
| ADR 指定と異なる layer への型追加 | 設計不適合 | ADR の配置に戻すか、ADR を改訂してから実装する |

## Related Documents

- `architecture-rules.json` — layer の crate / path / 依存方向の machine-readable SSoT。`cargo make check-layers` が強制する
- `.harness/capabilities/` — 委譲先 capability の運用契約。本 policy の R1-R3 は委譲元の責務であり、委譲先の scope ownership とは別に働く
