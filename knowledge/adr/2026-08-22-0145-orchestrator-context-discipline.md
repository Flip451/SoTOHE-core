---
adr_id: "2026-08-22-0145-orchestrator-context-discipline"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:claude-session-01KjrLiixdHPZAezqFdVwGSS:2026-08-22 orchestrator-token adjudication"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:claude-session-01KjrLiixdHPZAezqFdVwGSS:2026-08-22 orchestrator-token adjudication"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:claude-session-01KjrLiixdHPZAezqFdVwGSS:2026-08-22 orchestrator-token adjudication"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:claude-session-01KjrLiixdHPZAezqFdVwGSS:2026-08-22 orchestrator-token adjudication"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:claude-session-01KjrLiixdHPZAezqFdVwGSS:2026-08-22 orchestrator-token adjudication"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:claude-session-01KjrLiixdHPZAezqFdVwGSS:2026-08-22 orchestrator-token adjudication"
    status: proposed
---
# オーケストレーターの文脈摂取を規律化する

## Context

consumer セッションの課金ログ実測（2026-08-22、55 時間・モデル呼び出し 2,452 回）で、オーケストレーターの入力 5.8 億トークンの主因がハーネスの手順自体にあると特定された。ワークフローが成果物本文の全文読みを指示し（同一の巨大 JSON が 100 回超読まれた）、PR 指摘の修正を親コンテキストでの直接編集として指示し（1 ターンで 498 呼び出し・委譲は 55 時間で 2 回）、長いゲートの完了をポーリングで待たせていた（488 回）。実装は委譲が正規である一方、PR 修正だけが親実装を正規経路にしており、手順間の矛盾でもある。

## Decision

### D1: 親の一次情報は CLI サマリとする

オーケストレーションの手順文書を改訂し、進行・レビュー要否・義務状態・カタログ照会は各 CLI の要約出力を一次情報とする。成果物本文（types / review / bindings 等の JSON、サブワークフロー全文、Related Conventions）は、差分やブロッカーの調査時に限って開く。ワークフロー冒頭の一括読み込み指示は廃止し、規約類は briefing に列挙されたパスを委譲先が読む。

### D2: PR 指摘の修正も委譲を正規経路とする

pr-review 手順の「親が直接修正する」指示を、既存の briefing → 委譲（implementer / review-fix-lead）→ ローカル収束 → commit workflow の経路に置き換える。親の直接編集は委譲失敗時の回復に限る。実装委譲の既存原則を PR レーンにも適用すると明記する。

### D3: 長いゲートは 1 呼び出しで待ち、evaluate を親から外す

長時間ゲートはブロッキング 1 回で待つ。ホストがバックグラウンド化した場合は完了通知 1 回で結果を読み、ポーリングをしない。コミットゲートは check 系で足りるため、evaluate は修復作業時に同期実行のみとし、親が投げっぱなしにしない。

### D4: フェーズ境界で親セッションの更新を宣言する

adr2pr 手順に、計画成果物コミット後・最初の実装バッチ後・PR レーン開始時のセッション更新点を明記する。機械状態は git と track 成果物にあり、親コンテキストは捨ててよい。ホストに自動更新が無い場合はユーザーへ更新を要求してよい。

### D5: always-applied 文書を orchestrator 向けに分離する

PR レビュアー向け文書をオーケストレーターの always-applied から外し、orchestrator 向けの短い規則ファイル（委譲・CLI 一次情報・git 直叩き禁止）を provider 別の規則面として新設する。ルート md はポインタに痩身する。provider 側の互換 rules 設定は consumer 設定の領分とし、consumer 向け文書に記載する。

### D6: orchestrator の既定 reasoning effort を下げる

orchestrator profile の既定 effort を中位にする。待機・ゲート確認に高位の思考は不要であり、実装判断は委譲先の effort が担う。

## Rejected Alternatives

- **Skills カタログの入口 1 枚への畳み込み**: 削減余地はあるが、Codex 側の呼び出し面との両立確認が先。確認後に別途判断する。
- **ゲート stdout のサマリ契約化を本 ADR に含める**: CLI・Makefile の出力仕様変更でありコード track の領分。別 ADR とする（本 ADR は手順・設定文書のみ）。
- **ホスト挙動（バックグラウンド化闾値・通知形式・compaction 時期）への対処**: ハーネスから変えられない境界であり、手順は「起こされ方」への適応だけを定める。

## Consequences

- 良: 巨大成果物の反復読み・親実装の怪物ターン・ポーリングが手順から消える。委譲の原則が implement と PR レーンで一貫する。
- 良: 効果は次の track の課金ログ（呼び出し数・非キャッシュ入力・全文 read 回数・委譲回数）で同一指標比較できる。
- 負: 手順文書の広範な改訂。委譲経路が弱い provider では回復経路（親編集）の使用頻度が上がりうる。
- 中立: 摂取規律は prompt-level の強制であり、遵守は計測で監視する（機械強制は将来のワークフロー実行系の領分）。

## Reassess When

- 委譲先の失敗率が高く、回復経路としての親編集が常態化したとき。
- ワークフロー実行系（宣言的定義）への移行時 — 本 ADR の摂取規律はエッジペイロード規律として実行系の契約に翻訳する。
