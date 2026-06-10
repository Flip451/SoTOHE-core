---
adr_id: 2026-06-10-1129-track-workflow-telemetry
decisions:
  - id: D1
    user_decision_ref: "chat:2026-06-10:post-hoc-track-observation"
    status: proposed
  - id: D2
    user_decision_ref: "chat:2026-06-10:tracing-subscriber-jsonl"
    candidate_selection: "from:[otel-otlp-backend,otlp-blocking-http,tracing-subscriber-jsonl] chose:tracing-subscriber-jsonl"
    status: proposed
  - id: D3
    user_decision_ref: "chat:2026-06-10:track-dir-colocation"
    candidate_selection: "from:[tmp-dir,top-level-logs,track-dir-colocation] chose:track-dir-colocation"
    status: proposed
  - id: D4
    user_decision_ref: "chat:2026-06-10:block-and-advisory-only"
    candidate_selection: "from:[hook-full-recording,block-and-advisory-only] chose:block-and-advisory-only"
    status: proposed
  - id: D5
    user_decision_ref: "chat:2026-06-10:default-on-env-opt-out"
    status: proposed
  - id: D6
    user_decision_ref: "chat:2026-06-10:telemetry-report-initial-scope"
    status: proposed
  - id: D7
    user_decision_ref: "chat:2026-06-10:per-line-schema-version"
    candidate_selection: "from:[per-line-field,file-header-event] chose:per-line-field"
    status: proposed
---
# track ワークフロー telemetry の導入 — tracing + JSONL による事後観測

## Context

track ワークフロー（Phase 0–3 → 実装 → review → commit）の進行は、SoT 成果物
（metadata.json / review.json / git notes）に「最終状態」としては残るが、
時系列イベントとしては残らない。そのため:

- どの phase / gate に時間がかかったか（ボトルネック）
- gate・レビューの誤検出（false positive）の頻度と内訳
- 外部 provider 呼び出しの retry / verdict parse 失敗などテンプレート自体のバグの兆候

を事後に再構成できない。

`track/tech-stack.md` はロギングとして tracing + tracing-subscriber を記載済みだが、
実際には workspace のどの crate も未導入。観測対象の sotp は同期・短命 CLI で、
hook dispatch 経由の高頻度呼び出しがある。

本決定は、戦略草案「純粋テンプレートを抽出した並行レーンで別トポロジーの外部実証を行う」
の D3（ハーネスの定点観測指標の記録）の実装手段にもあたる。なお本 telemetry が観測できる
のは sotp プロセス内のイベントのみで、チャット / orchestrator 層で起きる人手介入
（human rescue）そのものは観測できない。人手介入率を扱う場合は blocked 終端・同一コマンド
の再実行などの代理指標で近似する。

## Decision

### D1: 目的 — track ワークフローの事後観測

ボトルネック発見 / false positive 検出 / テンプレートバグ洗い出しを目的とした
telemetry を導入する。コンプライアンス監査ではない（イベントの稀な欠落は許容。
SoT 成果物は telemetry と独立に無傷で残る）。

### D2: 技術 — tracing + tracing-subscriber のみ

OpenTelemetry crate は導入しない。イベントに timestamp / duration / track_id 等の
相関フィールドを持たせ、将来 OTel Collector（filelog receiver 等）へ接続する余地は残す。

### D3: 出力 — `track/items/<id>/logs/telemetry.jsonl`（track dir への colocation）

telemetry は track dir 配下の `logs/` に置く（gitignore: `track/items/**/logs/`）。
retention は track のライフサイクルに追随する: archive 時は gitignored の `logs/` も
track dir と一緒にファイルシステム上で移動する（archive ツール側の実装要件）。
subscriber 初期化は apps/cli-composition の composition root のみ。domain 層は計装しない。

並行 worker（Agent Teams）からの同時 append に備え、書き込みは O_APPEND フラグを立てた
単発 write syscall で行う。1 イベント行の上限を 4096 バイトとする（Linux ext4 等の通常
ファイルシステムにおける O_APPEND write の単発アトミック性は POSIX 仕様の範囲外だが、
ほとんどの Linux カーネル実装では同一マウント内の通常ファイルへの O_APPEND write は
カーネルが排他ロックを取るため実際にはアトミックに動作する。本 telemetry は診断用途で
稀な行の乱れは許容するため、この実装依存を意図的に受け入れる）。超過しうる可変長フィールド
は要約して収める。ユーザーランドの lock 機構は使わない（fire-and-forget の書き込みに
レイテンシを足さない）。

### D4: 記録範囲

記録する: track 操作 subcommand（command・exit code・所要時間・track_id）/
gate 評価（gate 名・verdict・理由要約・入力 hash）/ review・dry round
（provider・model・round 種別・所要時間・findings 件数）/ 外部 subprocess
（所要時間・retry 回数・verdict parse 失敗）/ hook dispatch の block 時および
advisory（注入型）hook の発火時 / 非ゼロ exit（error chain）。

記録しない: track を解決できないコマンド実行（目的が track ワークフロー観測のため、
全量を記録する `_global` ファイルは設けない）/ hook の素通り（block も advisory 注入も
ない allow）/
純表示系コマンド / 関数レベルの細粒度 span / findings・briefing 本文の複製
（SoT への参照のみ）/ プロンプト本文等の機密になりうる引数。

track 帰属は branch-bound に解決する（現在の git ブランチに紐づく track）。track 引数を
取らないリポジトリ全域ゲート（layers / orchestra 等）も、track ブランチ上での実行は当該
track に帰属させる。`track/*` 以外のブランチ（`main` 等）では、`track_resolution.rs` の
`updated_at` fallback で最新 track が解決される場合でも telemetry は記録しない（許容）。
理由: telemetry の目的は track ワークフロー進行の時系列観測であり、main ブランチ上の
作業は track フェーズに帰属しない操作（マージ後確認等）が混在する。branch-bound 帰属に
絞ることで track 外ノイズを排除し、`_global` ファイルも設けない。

### D5: 有効化 — 既定 ON

env による opt-out を備える: `SOTP_TELEMETRY=0` で全記録を無効化（kill switch）、
`SOTP_TELEMETRY_DIR` で書き出し先を差し替える。テスト実行時は強制隔離とし、
Makefile の test 系タスクと integration test helper の両方でこれらの env を設定する
（spawn された実バイナリには `cfg(test)` が効かないため env で隔離する）。
初期化は lazy とし、イベントが 1 件も発生しなければ file open を行わない
（hook の allow 経路にファイル IO を足さない）。

### D6: 分析 — sotp telemetry report を初期スコープに含める

track-id 指定でフェーズ別所要時間・エラー一覧・hook block 一覧を集計する
subcommand を初期スコープに含める（集計ロジックは Rust native。cargo make に置かない）。
読み手は破損行・未知 `schema_version` の行を fail せず skip し、skip 件数を報告する
（fail-open — lock なしの並行 append で稀に生じうる破損行を読み手側で吸収する）。

### D7: イベントスキーマのバージョン管理 — 各行に schema_version

各イベント行（JSONL の 1 行）に `schema_version` フィールドを持たせ、スキーマ変更時の
互換性判定を行単位で可能にする。file header イベント方式は並行 append の初回書き込み
競合があるため採らない。

## Rejected Alternatives

### A. OpenTelemetry OTLP export + バックエンド（Jaeger / Tempo）

分析が事後・バッチ型でライブ可視化の強みが活きない。1 track が数時間〜数日・
多数の短命プロセスにまたがる実行モデルとトレースビューの相性が悪い。
collector 運用と per-invocation 接続コストも重い。

### B. usecase 層への AuditSink port 新設（監査 grade の確実書き込み）

目的が診断であり、イベントが欠落しても SoT は無傷。port 新設は過剰設計。

### C. opentelemetry-otlp の blocking HTTP による直接 export

バージョン依存の細部検証コストがあり、offline-first 要件（collector 不在で
失敗・遅延しない）との摩擦がある。

### D. tmp/ 配下への出力

使い捨ての含意があり（gitignore で丸ごと無視 + 随時削除運用）、
事後分析用データの保持と矛盾する。

### E. hook dispatch の全量記録

通過（allow）が圧倒的多数でノイズ。block と advisory 注入の発火のみを記録すれば
guard の誤爆は検出できる。

### F. top-level `logs/telemetry/<track-id>.jsonl` + `_global.jsonl` への集約出力

全データが一箇所に集まる単純さはあるが、retention を track のライフサイクルと
別に決める必要が残り、archived track との突き合わせも手動になる。テンプレートの
リポジトリルートに top-level dir が増える点も不利。track dir への colocation なら
retention が自動で解決し、`track/items/**/.commit_hash` 等の「track dir 内の
gitignored ランタイム成果物」という既存パターンにも沿う。

## Consequences

### Positive

- track 進行の時系列が再構成可能になり、ボトルネック・収束の遅いレビューを定量化できる
- gate / guard の false positive を記録から検出でき、テンプレートバグの早期発見につながる
- 依存追加が tracing 系 2 crate のみで、同期 CLI 方針・レイヤー構成への侵襲が小さい
- telemetry の保持期間が track のライフサイクル（active → archive）に自動追随する
- 外部実証用テンプレートの抽出前に本体へ実装すれば、抽出テンプレートで走る外部実証 track
  が追加作業なしで自動計装される

### Negative

- 計装ポイント（usecase サービス・adapter 境界）の保守コストが増える
- JSONL のイベントスキーマに互換性管理が必要になる
- テスト隔離の実装義務が生じる（怠るとテスト由来イベントが track dir を汚染する）
- archive ツールが gitignored の `logs/` をファイルシステム移動する実装要件が増える
  （怠ると archive 後に logs が孤児化する）

### Neutral

- エージェント作業時間そのものは span にならず、隣接イベント間の時間差として現れる

## Reassess When

- バックエンドでの waterfall 可視化の需要が実際に発生したとき（OTel 接続を別 ADR で検討）
- track を解決できないイベント（track 外コマンド）の可視性が必要になったとき
- tech-stack で async runtime が採用されたとき（exporter の選択肢が変わる）
- 記録範囲がボトルネック分析に不足すると判明したとき

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/conventions/hexagonal-architecture.md` — 計装の層配置の前提
- `track/tech-stack.md` — オブザーバビリティ欄（本 ADR 採用時に更新対象）
- 戦略草案「純粋テンプレートを抽出した並行レーンで別トポロジーの外部実証を行う」（未採択・未配置）— 本 ADR はその D3（定点観測指標の記録）の実装手段。草案の正式採用時に配置先パスを追記する
