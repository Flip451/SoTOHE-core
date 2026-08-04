# Observations

## 2026-08-02 — types re-entry の暫定二段階帰属 workaround

types review 入口で current catalogue に対する `task-contract coverage` / `check` が、通常の
Phase 3 再生成より先に完全な下流帰属を要求する循環を回避するため、ユーザー裁定
`tmp/handoff/2026-08-02-lane-d-delta-adjudication.md` に基づく一時的な運用 workaround を適用した。

- 根本原因はレーン A の `scope-conditional-pre-review-gates`（G22）で修理中である。
- 恒久 ADR として提案された `2026-08-02-0917-provisional-repair-task-bridge.md` は棄却・削除済みであり、
  この記録を ADR または policy へ昇格させない。
- impl-planner が暫定 `todo` repair task `T010` を作成し、変更後 contract と新規 catalogue entry の
  帰属を done task `T002` から分離した。done task の status、commit hash、履歴上の意味は変更していない。
- `T010` は帰属評価のためだけの暫定 task であり、この段階では `in_progress` / `done` へ遷移させず、
  実装および source 編集を開始しない。
- 適用直後の `bin/sotp task-contract coverage` と `bin/sotp task-contract check` はともに通過した。
- types review 収束後の通常の full Phase 3 再生成で、repair task、batch、coverage、task-contract を
  一体として再検証・正規化する。
- G22 merge 後に `develop` を取り込み、gitignored な `bin/sotp` を `cargo make build-sotp` で再構築すれば、
  scope 条件付き gate によりこの workaround は不要になる。

## 2026-08-03 — guarded base-merge conflict source-repair exception

D2 の既裁定に従い、conflict hunk の選択だけを pre-gate 実行可能化のために許す source-repair
境界を適用した。対象はこの workflow/policy 文書群のみで、placeholder・意味追加は行わない。

- 対象ファイル: `.harness/workflows/track/recover.md`, `.harness/policies/sot-reentry-sequencing.md`,
  `.harness/policies/pre-track-adr-authoring.md`, `CLAUDE.md`, `.agents/skills/track-recover/SKILL.md`
- 選択した辺: 既存文書の conflict-recovery/one-writer 境界を、D2 限定の hunk 選択と上流再収束後の
  designated-writer 再実行として明文化した。
- 理由: `task-contract-check` / signal / `track-active-gate` が multi-SoT conflict の未解消入力で
  起動不能になるため。semantic authorship は designated writer に残し、レビュー・commit gate は維持する。
- 追加適用: `spec-designer` / `type-designer` / `impl-planner` / `adr-editor` / `adr-diagnoser` の
  capability contract に `conflict-preparation` mode と即時 guardian/re-entry 条件を同期した。

## 2026-08-03 — guarded base merge (develop 92142af7) の conflict 準備と D2 hunk 選択

`bin/sotp track merge-base` が Conflicted を返し、unmerged path は
`libs/infrastructure/src/tddd/type_signals_codec.rs` の 1 件のみだった。D2 例外に基づき
orchestrator が既存 hunk の選択のみを行った。

- 対象ファイル: `libs/infrastructure/src/tddd/type_signals_codec.rs`（`sample_doc()` fixture）
- 選択した辺: HEAD（track 側）。`TypeSignalsCacheKey::new(CatalogueDeclarationHash,
  ImplementationInputHash, BaselineHash)` + 空 signal リスト。
- 理由: 現行の `TypeSignalsDocument::new` は `TypeSignalsCacheKey` を取るシグネチャであり、
  develop 側 hunk（旧 positional 引数）はコンパイル不能。hunk 選択のみで意味追加なし。
- 残余の base 由来ドリフト（conflict hunk なし）: (1) develop 追加の
  `libs/infrastructure/src/signal_report/mod.rs` が旧 accessor
  `document.declaration_hash()` / `document.implementation_input_hash()` を呼ぶ（現行 API は
  `cache_key()` 経由）。(2) develop 追加の
  `test_encode_canonicalizes_json_keys_and_is_byte_stable` は signal オブジェクトのキー順を
  検査するため、空 signal fixture と両立しない。いずれも recover workflow の
  normal implementer reconciliation 経路で再整合する。

### merge-base 3 連続失敗（11:31/11:33/11:43Z）の診断

canonical wrapper の再実行（11:59Z、Claude Code 環境）は成功し conflict 状態を確立した。
`GIT_TRACE` / `GIT_TRACE2_EVENT` の捕捉により、reference-transaction hook（prepared /
committed）は guarded トークンを認識して allow、merge は conflict で exit 1、adjudication は
正しく `Conflicted` を返すことを確認した。

- 失敗 3 回のエラー `guarded git merge failed`（`base_merge.rs:134`）は「merge 非ゼロ exit +
  unmerged paths なし + MERGE_HEAD なし」の経路であり、merge が preflight で即座に失敗して
  いたことを示す（所要 102-113ms）。
- 失敗時刻に HookBlock テレメトリはなく（11:27/11:40/11:51 の HookBlock は別事象 —
  トークンなしの直接 git ref 更新の遮断）、hook 拒否説は棄却。
- 有力仮説: 旧オーケストレーション（Codex CLI）の workspace-write sandbox は `.git` への
  書き込みを遮断するため、その環境内で走った merge-base は ref/index 書き込みで即失敗した。
  環境依存であり、コード上の決定的欠陥ではない。
- ただし adapter が git の stderr を握り潰す error observability の欠陥は実在する
  （失敗理由が `guarded git merge failed` に潰れ、診断に外部トレースを要した）。実 hook 構成
  での E2E 回帰テスト欠落と併せ、修理候補として別途裁定に付す。

### base 由来ドリフトの再整合（conflict hunk なし・機械的 API 整合のみ）

recover workflow の implementer reconciliation 経路を `bin/sotp capability exec implementer` で
起動しようとしたが、旧 `bin/sotp` バイナリが merge で入った develop 版 `agent-profiles.json` の
新フィールド `supported_reasoning_efforts` を解析できず、profile 解決が fail-closed した。
`cargo make build-sotp` は workspace のコンパイル（当時 E0599 で失敗）を要するため、capability
dispatch は再整合完了まで構造的に使用不能（循環）だった。このため orchestrator が以下の機械的
整合のみを直接適用した（意味追加なし・全て既存 API / 既存 develop 内容への追従）:

- `libs/infrastructure/src/signal_report/mod.rs`: develop 追加コードの旧 accessor 2 箇所を
  `document.cache_key().declaration_hash()` / `document.cache_key().implementation_input_hash()`
  に更新（比較セマンティクスは不変。baseline_hash 比較の追加はしない — 要否は post-merge の
  通常 writer 判断に委ねる）。
- 同ファイルのテストフィクスチャ `fresh_impl_catalog_signals`: schema_version 3→4、
  `baseline_hash` フィールド追加（現行 decoder は必須・deny_unknown_fields）。
- `libs/infrastructure/src/tddd/type_signals_codec.rs`: `sample_doc()` に develop 版の既存
  `TypeSignal` フィクスチャを復元（develop 追加の canonical-JSON テストは signal のキー順を
  検査するため空リストと両立しない）。同テストの先頭キー assertion を `declaration_hash` →
  `baseline_hash` に更新（DTO への baseline_hash 追加によりソート順が変わったため）。

適用後 `cargo check --workspace` clean、`type_signals_codec` / `signal_report` の 38 テスト全通過。

### TDDD baseline の base-commit 再捕捉（impl_catalog chain の再整合）

pre-review gate（`task-contract coverage` / `check`）が、merge で入った develop の新型
（SignalReport* 系ほか）を「帰属なし entry」、develop が変更した既存型（TrackStatus ほか）を
🔴 Mismatch として全 scope でブロックした。原因は track 開始時に捕捉した TDDD rustdoc
baseline の陳腐化であり、conflict hunk のない base 由来ドリフトである。

- 設計上の正規救済は clean-merge cleanup の「exact base commit からの baseline 置換」だが、
  conflicted 経路では cleanup が commit 後まで走れない一方、commit gate（`track-commit-message`
  の regen sequence）は signal を必ず再計算してブロックする — **conflicted 経路の gate 循環**。
  これは本 track 成果物の dogfooding で発見された設計ギャップとして、error observability
  欠陥と併せて修理裁定に付す。
- 再整合は baseline-capture CLI の文書化された再捕捉手順（baseline 削除 → 再実行）で行った。
  まず merged worktree から捕捉したところ、catalogue の `action=Add` 宣言と矛盾して
  `ActionContradiction` で fail-closed した（本 track の型まで baseline に入るため）。正しくは
  cleanup と同じ base 側のみの捕捉が必要で、scratchpad への使い捨て clone（base commit
  92142af7 で detach）を `--source-workspace` に与えて再捕捉した。この clone は本 repo の
  guarded 経路を一切迂回しない読み取り専用の rustdoc export 用である。
- 再捕捉後、`signal calc-impl-catalog` / `task-contract coverage` / `task-contract check` は
  すべて通過。catalogue・task-contract・impl-plan への編集は一切行っていない。

## 2026-08-04 — PR レビュー P1 対応の非収束とアプローチ転換（艦隊処方箋）

PR #236 の review cycle round 8 の 2 P1（publication の retry ブロック窓 / stash snapshot の
16KiB 上限）への fix ループが、infrastructure scope 累計 115 レビューラウンド・findings 1〜4 の
振動状態に陥った。直近の findings はすべて fix 中に発明された `retain_open_file_inodes` /
descriptor-retention hardlink 機構の別々の穴であり、艦隊処方箋
（`tmp/handoff/2026-08-04-stash-retention-prescription.md`）に基づき以下を実施した。

- fix ループを即時停止し、未コミットの retention 精巧化（`publication.rs` 441→1141 行、
  `base_merge.rs` の付随変更）を破棄して HEAD（`fef26ec6`）へ復元した。破棄 diff は
  scratchpad に退避済み。
- stash 側の未コミット変更は P1 の原方針（porcelain 出力の保持をやめ SHA-256 streaming
  digest 化、400-file 回帰テスト付き）どおりのため維持した。
- publication 側は「保持をやめる」設計で最小に再設計した: track dir 直下の `logs/`
  （機械追記の telemetry、gitignored 運用状態）を publication drift 比較から構造的に除外
  （`cleanup_tree.rs` の snapshot 収集で skip）。これにより「telemetry 追記が retained tree
  との等価比較を恒久的に破る」問題クラス全体が stateless に消滅し、retention 機構は不要。
- 既知のトレードオフ: capture 中の並行 telemetry 追記は exchange で失われ得る（診断ログ
  のみ・gitignored）。実体保持が本当に要る要件が示されるなら、それは実装判断ではなく
  User 裁定として提起する。
- 蓄積 findings（世代重複・並行 writer・file↔dir 変化等）は hash/除外設計では構造的に
  発生しないクラスであり、回帰観点チェックリストとしてこの記録に保持する。
