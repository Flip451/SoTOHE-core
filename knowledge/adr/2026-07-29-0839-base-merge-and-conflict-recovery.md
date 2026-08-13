---
adr_id: "2026-07-29-0839-base-merge-and-conflict-recovery"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:sotohe-issues-discussion:2026-07-29; chat_segment:phase0-converged-adr-approval:2026-08-01"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:sotohe-issues-discussion:2026-07-29; chat_segment:track-recover-dual-adapter-ssot:2026-08-01; chat_segment:phase0-converged-adr-approval:2026-08-01"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:sotohe-issues-discussion:2026-07-29; chat_segment:phase0-converged-adr-approval:2026-08-01"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:baseline-hash-freshness-decision:2026-08-01; chat_segment:phase0-converged-adr-approval:2026-08-01"
    status: proposed
---
# base→track 方向の guarded merge コマンドとコンフリクト回復レーンを設ける

## Context

`git merge` は AI に対して一律ハードブロックされているが、ブロックすべき実体は **track → base（develop）方向**（merge 承認は PR 経由のみ）であって、**base → アクティブ track 方向**（base の更新の取り込み）ではない。後者まで塞がれているため、base 追随のたびにユーザーの手動 merge が必要になっている。

さらに merge 後の後始末（derived views 再生成・TDDD baseline 再取得・sync-base stamp）は手順が散在し、特に baseline 再取得には「対応する type-signals 成果物を削除しないと古い評価が再利用される」という既知の罠がある。conflict 発生時の解消 → レビュー → コミットの手順も定型化されていない。git stash にも guarded wrapper がなく、作業退避ができない。

## Decision

### D1: guarded な `sotp track merge-base` コマンドを追加する

base → アクティブ track 方向の merge を実行する guarded コマンドを設ける。guard は 2 点: 現在 branch が `track/<id>` であること、merge 元が `branch_strategy_snapshot` の base branch であること。逆方向（track → base）は引き続き拒否する。clean merge 時は後始末（views 再生成・baseline recapture・sync-base stamp）まで一括実行する。

### D2: conflict 解消はオーケストレーター主導とし、手順を専用 skill に定義する

conflict 発生時もユーザーに委ねず、オーケストレーターが「コンフリクト解消の編集 → 正規の review workflow（zero_findings まで）→ guarded commit」を実行する。

この手順と merge 後の後始末の operational SSoT は `.harness/workflows/track/recover.md` に置く。同じ実装単位で、Claude Code の `/track:recover` adapter と Codex の `$track-recover` skill adapter を提供し、片方だけの提供を完了としない。

両 adapter は起動形態、provider 固有の tool 制約、報告形式だけを所有し、手順、gate、状態遷移、failure recovery を重複定義しない。D1 の conflict 分岐は、この共通 workflow SSoT に接続する。

### D3: guarded な `sotp git stash` wrapper を追加する

stash / stash pop の guarded wrapper を設ける。stash は履歴を汚さないため guard は軽くてよいが、untracked な track artifacts の扱い（-u 相当）を明示する。

### D4: type-signals の新鮮度判定に baseline hash を含める

現在アクティブなトラックを対象として、type-signals cache に `baseline_hash` を記録する。信号機評価は実際の baseline を読み、その hash を計算してから cache の再利用可否を判定する。

catalogue declaration hash、implementation input hash、baseline hash の 3 値がすべて一致する場合に限り既存評価を再利用する。いずれかが不一致なら cache stale として信号機を再評価し、成功後に評価結果と現在の 3 hash で cache を原子的に置き換える。

type-signals cache が存在しない場合、または schema version 不一致、必須 field 欠損、不正値、JSON 破損を含む decode 不良がある場合は、cache miss として信号機を再評価する。再評価に成功した場合だけ、現行 schema の cache を原子的に書き戻す。decode 不能な内容を既存評価として再利用しない。

実際の baseline の欠損・読み取り失敗、その他の権威入力の取得失敗、信号機評価の失敗、cache 書き込みの失敗は fail-closed とする。inactive・archived track の既存成果物は走査・移行・検証しない。type-signals の手動削除を baseline 再取得の通常手順には含めない。

## Rejected Alternatives

### A: 全方向 merge のハードブロック維持（現状）

base 追随のたびにユーザー手動作業が発生し、後始末の手順漏れ（baseline 罠）も再発し続けるため却下。方向 guard で安全性は保てる。

## Consequences

- 良: base 追随と conflict 回復が自律レーンに載り、ユーザー介入は不要になる。baseline 再取得の罠が手順に埋め込まれ再発しない。
- 負: merge という不可逆性の高い操作を AI が実行する範囲が広がる（方向 guard と review 必須で緩和）。

## Reassess When

- branch strategy が変わり base の概念が複数になったとき。
- D2 の自律解消が誤った conflict 解決を review で検出できなかった事例が出たとき。
