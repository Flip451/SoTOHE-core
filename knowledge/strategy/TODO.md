# テンプレート基盤 改善 TODO リスト

> **出典**: `tmp/review-2026-03-10.md`（Gemini による包括的レビュー）
> **作成日**: 2026-03-11
> **最終更新**: 2026-04-06
> **アーカイブ**: 解決済み項目は `tmp/TODO-archived-2026-03-16.md` に移動済み
> **全体計画**: [`knowledge/strategy/TODO-PLAN.md`](TODO-PLAN.md)（v3: ハーネス vs テンプレート出力の区別）
> **全体計画 (旧版)**: `tmp/archive-2026-03-20/`
> **SDD 比較レポート**: `tmp/sdd-comparison-report-2026-03-17.md`（Tsumiki vs CC-SDD vs SoTOHE-core）
> **ハーネスサーベイ**: `tmp/agent-harness-survey-2026-03-17.md`（Spec Kit, OpenSpec, SpecPulse, ECC, Anthropic 公式, Symphony）
> **取り込み推奨一覧**: `tmp/adoption-candidates-2026-03-17.md`（全 35 件、ロードマップ付き）
> **リファクタリング計画**: [`tmp/refactoring-plan-2026-03-19.md`](./refactoring-plan-2026-03-19.md)（対象タスク + ドメインモデリング + 再発防止メカニズム）
> **レビュープロセス監査**: [`knowledge/research/2026-03-31-2112-review-process-audit.md`](../research/2026-03-31-2112-review-process-audit.md)（RVW-37〜43 採番元、legacy 深刻度再評価）
> **進捗管理**: [`knowledge/strategy/progress-tracker.md`](progress-tracker.md)（v3）

---

## 凡例

- [ ] 未着手
- 優先度: **CRITICAL** / **HIGH** / **MEDIUM** / **LOW**
- カテゴリ接頭辞: SEC=セキュリティ, CON=並行処理, SSoT=データ整合性, RTR=ルーティング, ERR=エラー処理, INF=インフラ, WF=ワークフロー

---

## A. セキュリティ・ガードレール (SEC)

### A-2. サンドボックス境界の強化

- [x] ~~**SEC-09** (HIGH): キャッシュパス検証の不備 — 任意ファイル上書き (§A-483)~~ ✅ Python 側で修正済み
  - **対応**: `validate_cache_path` に `resolve()` + `relative_to()` によるパス境界検証を実装。テスト 2 本（絶対パス拒否 + トラバーサル拒否）追加済み。Rust 移行は未実施

### A-3. パストラバーサル

- [ ] **SEC-10** (MEDIUM): パス検証の `".." in path` 依存 (§A-84)
  - **課題**: `verify_plan_progress.py`, `lint-on-save.py` で文字列判定に依存
  - **提案**: `pathlib.Path.resolve()` + `is_relative_to()` に移行

- [ ] **SEC-11** (MEDIUM): guard の `"git"` 部分文字列検出の過剰ブロック
  - **根拠**: `libs/domain/src/guard/policy.rs` は非 git コマンドでも argv/redirect に `"git"` を含むだけで block する設計。
  - **提案**: 部分文字列ではなく、argv 境界・AST 上のコマンド名・`git` 実行意図に限定した判定へ縮退する。

### A-3a. Guard 再帰パース強化（bash-write-guard 残留リスク）

- [ ] **SEC-14** (LOW-MEDIUM → HIGH): shell `-c` payload の再帰パース不足 → [詳細](./refactoring-plan-2026-03-19.md) §3
  - **追加根拠** (2026-03-24 CC-SDD-01 review): `bin/sotp` 上書きガード (`guard/policy.rs:191`) がトップレベルコマンドのみ検査するため、`sh -c 'cp target/release/sotp bin/sotp'` でバイパス可能。`-c` payload の再帰パースが必要

- [x] ~~**SEC-15** (LOW-MEDIUM): heredoc body の再帰パース不足~~ ✅ bash-write-guard で対応済み
  - **対応**: `redirect_texts` フィールドが heredoc body（`Redirect::Heredoc`）を収集し、`command_contains_git` が `redirect_texts` を検査。compound command の `compound_redirect_texts` も内部コマンドに伝播される設計
  - **残留リスク文書**: `project-docs/conventions/bash-write-guard.md`

- [ ] **SEC-16** (LOW): `sed -ni` / `sed -i.bak` 等の combined short flag / attached suffix 検知不足
  - **課題**: `has_sed_inplace_flag` は standalone `-i`、`-i=suffix`、`--in-place`、`--in-place=suffix` のみ検知。combined flags（`-ni`）や attached suffix（`-i.bak`）は false positive 回避のため見送り
  - **対応方針**: sed の `-e`/`-f` が引数を消費する仕様を考慮した option-aware parser を実装。combined flags 内で `-e`/`-f` より前に `i` が出現する場合のみ in-place と判定。または `sed` を `permissions.deny` に追加して全面禁止（aggressive だが安全）
  - **トレードオフ**: 精度向上 vs false positive リスク。現状は false positive 回避を優先

### A-4. サプライチェーン・依存管理

- [ ] **SEC-12** (MEDIUM): External Guides のチェックサム/staleness 検知欠如
  - **課題**: `external_guides.py` の `fetch_guides()` にハッシュ検証・ETag/Last-Modified 比較なし。改ざんや古いキャッシュを検知できない
  - **提案**: レジストリに `sha256` フィールドを追加し、ダウンロード後に検証。staleness 検知には Last-Modified ヘッダ比較を導入
  - **出典**: NotebookLM 2026-03-16 指摘 1.3 + 4.1

- [ ] **SEC-13** (MEDIUM): `conch-parser` ベンダリングの保守方針
  - **課題**: `vendor/conch-parser/` は Rust 2015 Edition で `#![allow(warnings)]` により全警告を抑制。上流のセキュリティパッチ追従が不可能
  - **提案**: 上流フォークの定期確認体制を整える、または代替パーサー（`tree-sitter-bash`, `shlex`）への移行を検討
  - **出典**: NotebookLM 2026-03-16 指摘 4.5

- [ ] **SEC-17** (LOW): `serde_yaml` の deprecated 状態の経過観察
  - **課題**: `serde_yaml` は 2023 年に作者（dtolnay）により deprecated。ただしコミュニティでは引き続き広く使用されており、実質的に壊れているわけではない（Reddit r/rust 2025 年議論で確認）
  - **提案**: 定期的に `cargo audit` で脆弱性を監視。問題が出た場合の移行候補: (1) `serde_yml`（コミュニティフォーク、即使用可）、(2) `saphyr` + `saphyr-serde`（YAML 1.2 完全準拠、serde 統合は未リリース — リリース後に再評価）
  - **出典**: spec-template-foundation-2026-03-18 で導入 (2026-03-18)

---

## B. 並行処理・排他制御 (CON)

- [x] ~~**CON-02** (MEDIUM): ログローテーション競合 (§B-14)~~ ✅ 修正済み
  - **対応**: `log-cli-tools.py` に `fcntl.flock(LOCK_EX | LOCK_NB)` ロックガードを実装。`.lock` サイドカーファイルで排他制御。Windows では graceful skip

- [ ] **CON-03** (MEDIUM): 自然言語依存の排他制御 — Cargo.lock (§B-295)
  - **課題**: `Cargo.lock` 変更の直列化をプロンプト指示に依存
  - **提案**: `flock` による OS レベルロックを `cargo make` タスクに組み込み

- [x] ~~**CON-07** (HIGH): `Bash` 経由のファイル更新が file-lock hook を完全に迂回~~ ✅ `bash-write-guard-2026-03-18` (PR #35)
  - **対応**: 3層防御 — permissions.deny（touch/cp/mv/install/chmod/chown）+ AST output redirect 検知（既存 guard 拡張）+ 残留リスクドキュメント
  - **残留リスク**: `-c` payload / heredoc body の再帰パース不足 → SEC-14/SEC-15 で別途対応予定

- [ ] **CON-08** (MEDIUM): `tmp/track-commit/` の singleton scratch file による競合・残骸汚染
  - **根拠**: `tmp/track-commit/add-paths.txt`, `commit-message.txt`, `note.md`, `track-dir.txt` は固定パス。CI 失敗時は scratch file が残留する。
  - **提案**: `tmp/track-commit/<track-id>/` または `tmp/track-commit/<run-id>/` に分離し、失敗時も cleanup する専用ラッパーへ変更する。

---

## C. SSoT・データ整合性 (SSoT)

- [ ] **SSoT-02** (MEDIUM): SSoT リカバリ・障害耐性の不在 (§C-173)
  - **課題**: `metadata.json` 破損時にシステム全体がクラッシュ
  - **提案**: Read-only フォールバックモード、Git Notes からの自動復元

- [ ] **SSoT-03** (MEDIUM): SSoT と AI テキスト編集の不一致 (§C-259)
  - **課題**: AI が `plan.md` を直接編集 → 次回レンダリングで消失
  - **提案**: `plan.md` の OS レベル ReadOnly 化、または双方向バインディング

- [ ] **SSoT-04** (LOW): View 層の破壊的レンダリング (§C-304, §C-372)
  - **課題**: 人間の編集が `sync_rendered_views()` でサイレントに上書き
  - **提案**: 楽観的ロック（ハッシュ比較）またはマーカータグによる領域分離

- [ ] **SSoT-05** (LOW): SSoT 分散と同期オーバーヘッド (§C-395)
  - **課題**: `architecture-rules.json`, `deny.toml`, `check_layers.py` 等の5箇所同時更新
  - **提案**: JSON SSoT からの自動生成（コード生成）

- [ ] **SSoT-06** (LOW): 状態遷移の監査証跡欠如 (§C-452)
  - **課題**: metadata.json がスナップショット型、誰がいつ変更したか不明
  - **提案**: イベントソーシングモデル（Append-only ログ）

- [ ] **SSoT-07** (MEDIUM): SSoT と AI プロンプトの矛盾 — Split-brain (§C-464) → [詳細](./refactoring-plan-2026-03-19.md) §5

- [x] ~~**SSoT-09** (MEDIUM): `TrackDocumentV2` の未知フィールドサイレント消失~~ ✅ 修正済み
  - **対応**: `TrackDocumentV2` に `#[serde(flatten)] pub extra: serde_json::Map<String, Value>` を追加。`DocumentMeta` にも `extra` フィールドを追加し、read-modify-write 全経路で未知フィールドを保全。テスト 3 本追加済み
  - **出典**: NotebookLM 2026-03-16 指摘 10.1

- [x] ~~**SSoT-10** (MEDIUM): `collect_track_branch_claims` の単一破損ファイルによる全停止~~ ✅ 修正済み
  - **対応**: `match` + `continue` パターンで破損ファイルを `warning:` 付きでスキップし、正常なトラックの処理を継続。テスト `collect_track_branch_claims_skips_invalid_metadata_and_returns_valid` で検証済み

- [ ] **SSoT-11** (LOW): `spec_frontmatter.rs` の closing delimiter が末尾空白を許容
  - **根拠** (2026-03-24 CC-SDD-01 review): `verify/spec_frontmatter.rs:42` で `line.trim_end() == "---"` を使用。`---   ` (末尾空白付き) が正常な frontmatter 終端として通過してしまう
  - **提案**: `line.trim_end()` → `line == "---"` に変更（厳密一致）
  - **出典**: NotebookLM 2026-03-16 指摘 10.2

---

## D. ルーティング・コンテキスト管理 (RTR)

- [ ] **RTR-01** (MEDIUM): 正規表現スコアリングによるルーターハイジャック (§D-270)
  - **課題**: 外部ガイド文脈中のキーワードで意図しないエージェントにルーティング
  - **提案**: Tool Calling ベースのセマンティック意図分類に移行

- [ ] **RTR-02** (MEDIUM): ステートレスルーティングの文脈喪失 (§D-423)
  - **課題**: 最新プロンプトのみでルーティング判定、「はい」等の短い返答で誤作動
  - **提案**: 過去 N ターンのスライディングウィンドウまたは LLM Tool Calling

- [ ] **RTR-03** (LOW): マルチモーダル判定の意図ハイジャック (§D-526)
  - **課題**: `.pdf` 拡張子だけで multimodal_reader にルーティング
  - **提案**: 拡張子よりユーザー意図（設計/実装/デバッグ）を優先

- [ ] **RTR-04** (LOW): O(N) コンテキスト手動ロードの非効率性 (§D-474)
  - **課題**: Implementer が規約ファイルを1つずつ手動で読み込み
  - **提案**: オーケストレーター側でコンテキストインジェクション

---

## E. エラーハンドリング・可観測性 (ERR)

- [ ] **ERR-01** (MEDIUM): ハードコード設定値とフォールバック (§E-27)
  - **課題**: タイムアウト値が固定、タイムアウト時の部分出力が握りつぶし
  - **提案**: 環境変数オーバーライド、部分出力をエラーレポートに含める

- [ ] **ERR-02** (MEDIUM): LLM 応答 JSON パースの堅牢性 (§E-95)
  - **課題**: LLM が Markdown コードブロックで囲むとパース失敗
  - **提案**: コードブロックマーカー除去の前処理、または Structured Outputs API

- [ ] **ERR-03** (MEDIUM): 孤立子プロセスのタイムアウト管理 (§E-105)
  - **課題**: `TimeoutExpired` 時に孫プロセスが残留
  - **提案**: `os.setsid` + プロセスグループ全体 SIGKILL

- [ ] **ERR-04** (MEDIUM): サーキットブレーカーの自己監視欠陥 (§E-183)
  - **課題**: ループ判定を LLM 推論に依存、非決定論的
  - **提案**: 同一エラーコード連続 + Git Diff ゼロなどの決定論的ヒューリスティック

- [ ] **ERR-05** (MEDIUM): ログ切り捨てによるデバッグ阻害 (§E-225)
  - **課題**: 末尾20行のみ切り取り、根本原因が先頭/中間に出力される
  - **提案**: `cargo test --message-format=json` で構造化エラー抽出

- [ ] **ERR-07** (LOW): ヒューリスティック文字列照合の脆さ (§E-363)
  - **課題**: コマンド文字列に `"cargo test"` が含まれるかで判定
  - **提案**: 終了コードや構造化出力ベースのトリガー判定

- [x] ~~**ERR-08** (MEDIUM): `/track:pr-review` の同期ポーリングが中断耐性を持たない~~ ✅ done (`pr-review-flow-fix-2026-03-31`)
  - **対応**: trigger state (PR番号, comment ID, trigger_timestamp, head_hash) を `tmp/pr-review-state/<track-id>.json` に永続化。`--resume` フラグで既存 state からポーリング再開可能に

---

## F. プラットフォーム・インフラ (INF)

- [ ] **INF-01** (LOW): ファイルパス解決のプラットフォーム依存 (§F-48)
  - **課題**: Windows パス判定が正規表現依存、UNC パス未対応
  - **提案**: `pathlib` / `os.path` のネイティブ判定に移行

- [ ] **INF-02** (MEDIUM): Tarpit / Slowloris 脆弱性 (§F-130)
  - **課題**: `urlopen(timeout=30)` はソケット無通信タイムアウトのみ
  - **提案**: チャンク読み取り + 全体経過時間の絶対タイムアウト

- [ ] **INF-03** (MEDIUM): ホスト vs コンテナのパス解決の構造的亀裂 (§F-164)
  - **課題**: パス変換ロジックが脆く、シンボリックリンクやドライブレターで破綻
  - **提案**: DevContainer 内にカプセル化して境界を単一化

- [ ] **INF-05** (MEDIUM): インフラ状態管理の LLM 過剰委譲 (§F-235)
  - **課題**: `tools-daemon` クラッシュ時にAIがインフラ再起動を推論できない
  - **提案**: `*-exec` タスク内でヘルスチェック + 自動再起動

- [ ] **INF-06** (LOW): Docker Compose UID/GID フォールバック (§F-403)
  - **課題**: 環境変数未設定時にファイル所有者不一致
  - **提案**: `cargo make bootstrap` で自動検知 → `.env` 書き込み

- [ ] **INF-07** (LOW): 動的 Python インタプリタ解決のレイテンシ (§F-411)
  - **課題**: タスク実行ごとにサブシェルでパス解決
  - **提案**: 初期化時に解決済みパスを `.env` にキャッシュ

- [ ] **INF-08** (MEDIUM): 動的 Python 環境の分離不全 (§F-654)
  - **課題**: `.venv` 未アクティベート時にフックが機能不全
  - **提案**: フックコマンドを `uv run` 経由に変更

- [ ] **INF-09** (MEDIUM): `.venv` の Bootstrapping Paradox (§F-791)
  - **課題**: `.venv` 未構築状態でフックがクラッシュ → 初期化すら開始不可
  - **提案**: フック層を標準ライブラリのみ or コンパイル済み Rust バイナリに
  - **進捗**: STRAT-03 Phase 1 で security-critical hook 3本の Python launcher を除去。必須経路からの Python 依存は大幅縮小。advisory hook はまだ Python 依存

- [ ] **INF-10** (MEDIUM): `tools-daemon` のステートフルネスによるフレイキーテスト (§F-800)
  - **課題**: 常駐コンテナに一時状態が蓄積 → CI との不一致
  - **提案**: エフェメラル指向 (`run --rm`)、sccache / tmpfs 活用

- [ ] **INF-11** (LOW): 外部依存 URL の GitHub 密結合 (§D-433)
  - **課題**: `derive_raw_url` が GitHub URL 構造にハードコード依存
  - **提案**: `raw_url` の明示的登録、またはプロバイダアダプタ層

- [ ] **INF-12** (MEDIUM): Claude Code hook の cold build timeout
  - **課題**: `.claude/settings.json` の PreToolUse hook が timeout 10-15s で fail-closed する設計だが、`sotp` バイナリが未ビルド（fresh clone 等）の場合、`cargo run -p cli` フォールバックが cold build を伴い timeout を超える
  - **提案**: hook timeout の引き上げ、または bootstrap 前の hook スキップ機構（`SOTP_HOOK_SKIP=1` 等）を導入

- [ ] **INF-13** (LOW): `atomic_write_file` のファイルパーミッション未転写
  - **課題**: `atomic_write.rs` の `fs::File::create(tmp_path)` は umask 依存でパーミッションを設定。元ファイルのパーミッションを転写しないため、保存のたびに固有の権限が初期化される
  - **提案**: `fs::metadata(target)` でパーミッションを取得し、rename 前に `set_permissions` で転写
  - **出典**: NotebookLM 2026-03-16 指摘 7.2

- [ ] **INF-14** (MEDIUM): ホスト Rust ツールチェーン依存によるコンテナ完結性の破綻
  - **課題**: `build-sotp` タスクがホスト `cargo build` を直接要求し、「Docker だけで開発開始」のポータビリティが崩れている
  - **提案**: コンテナ内ビルド + bind mount でバイナリを共有するか、マルチステージビルドで `bin/sotp` を生成
  - **関連**: INF-12（cold build timeout）
  - **出典**: NotebookLM 2026-03-16 指摘 2.2

- [x] **INF-15** (MEDIUM): ~~`sotp verify usecase-purity` — usecase 層のヘキサゴナル純粋性 CI 検証~~
  - **完了**: syn AST ベースで実装。std I/O を網羅的にブロック（`std::fs`, `std::net`, `std::process`, `std::io`, `std::env` + `std::time::SystemTime`/`Instant` + `println!`/`eprintln!`/`print!`/`eprint!` + `chrono::Utc::now`）。use import / alias / glob / self import も検出。warning-only。
  - **関連**: `project-docs/conventions/hexagonal-architecture.md` の "Usecase Layer Purity Rules"
  - **完了日**: 2026-03-22

- [x] **INF-16** (SMALL): ~~`pr_review.rs` hexagonal リファクタリング — `std::fs` / `std::io` を CLI 層に移動~~
  - **完了**: `resolve_reviewer_provider(&Path)` → `resolve_reviewer_provider(&str)` に変更。`PrReviewError` から `Io`/`ProfilesNotFound` 削除。CLI でファイル読み込み。usecase-purity warning ゼロ達成。
  - **完了日**: 2026-03-22 (PR #51)

- [x] **INF-17** (SMALL): ~~`usecase-purity` warning → error 昇格 — CI ブロック化~~
  - **完了**: `Finding::warning` → `Finding::error`。CI で usecase 層の hexagonal violation をブロック。
  - **完了日**: 2026-03-23 (PR #52)

- [ ] **INF-18** (SMALL): verify ルール定義の外部設定化 — ドメインロジックの infrastructure 流出防止
  - **課題**: `usecase_purity.rs` の禁止パターン定義（`FORBIDDEN_PATH_PREFIXES` 等）はドメイン知識だが infrastructure にハードコードされている。他の verify モジュール（`domain_strings.rs` 等）も同様。`module_size.rs` のみ `architecture-rules.json` から読み込み済み
  - **提案**: `docs/layer-purity-rules.json` を新設し、禁止パターンを外部設定化。infrastructure は設定を読んで適用するだけの「エンジン」に限定。`architecture-rules.json` への詰め込みは責務混在になるため別ファイルとする
  - **関連**: INF-15, `module_size.rs` の `architecture-rules.json` 読み込みパターン
  - **追加日**: 2026-03-22

- [x] ~~**INF-19** (SMALL): `sotp verify domain-purity` — domain 層 I/O purity CI~~
  - **完了**: `usecase-purity` と共通の `check_layer_purity` エンジンで実装。即 error モード。
  - **完了日**: 2026-03-23 (PR #53)

- [x] ~~**INF-20** (MEDIUM): `conch-parser` を domain から infrastructure に移動~~
  - **完了**: ShellParser port trait を domain に定義し、conch-parser 実装を infrastructure に移動。domain の I/O purity CI ゲートで今後の混入を防止。
  - **完了日**: 2026-03-23 (PR #54)

- [x] ~~**INF-21** (HIGH): TODO ID 自動採番~~ ✅ `/todo-add` スキルとして実装（`.claude/commands/todo/add.md`）。既存最大 ID を grep で自動取得し次番号を付与。CI ゲートは費用対効果から見送り（発生頻度低、影響小）。2026-03-28

- [ ] **INF-22** (LOW): `agent-profiles.json` を config ディレクトリに移動
  - **課題**: `.claude/agent-profiles.json` は `.claude/` 配下にあるが、本来は設定ファイルとして config/ 等の専用ディレクトリに配置すべき
  - **提案**: `config/agent-profiles.json` に移動し、参照箇所（`_agent_profiles.py`, `CLAUDE.md`, `.claude/rules/` 等）を一括更新
  - **追加日**: 2026-04-09

---

## G. ワークフロー・TDD (WF)

### G-1. TDD ステートマシン

- [ ] **WF-01** (MEDIUM): TDD テスト名の非決定性 (§G-492)
  - **課題**: LLM が完全修飾テスト名を誤推測 → 0件実行で成功扱い
  - **提案**: 構造化出力パースで実行件数を決定論的に検証

- [ ] **WF-02** (MEDIUM): コンパイルエラーとテスト失敗の混同 (§G-537)
  - **課題**: Red フェーズでコンパイルエラーも「テスト失敗」と誤認
  - **提案**: `--message-format=json` でコンパイルエラー 0件 + テスト FAIL を確認

- [ ] **WF-03** (LOW): 厳格すぎる状態遷移モデル (§G-552)
  - **課題**: `todo → done` 直接遷移が禁止、軽微タスクでも2回のAPI呼び出し必須
  - **提案**: Fast-path 遷移の許可、または一括更新 API

### G-2. トラック管理

- [ ] **WF-04** (MEDIUM): 最新トラック推論のコンテキスト競合 (§G-562)
  - **課題**: `updated_at` 最新のトラックをグローバルに選択 → 並行作業で誤参照
  - **提案**: ブランチ名とトラック ID の紐付け、または明示的セッション変数

- [x] ~~**WF-04b** (MEDIUM): metadata.json タイムスタンプの逆転・不整合~~ ✅ 修正済み
  - **対応**: `fs_store.rs` の `save()` で新規作成時に `created_at: Self::now_iso8601()` を自動生成。既存更新時は `created_at` を保持し `updated_at` のみ更新。手書きは排除済み

- [ ] **WF-05** (LOW): Git Notes のローカル制約 (§G-572)
  - **課題**: Notes はデフォルトでリモートと同期されない
  - **提案**: `cargo make bootstrap` で refspec 自動設定

- [ ] **WF-06** (LOW): `.gitignore` 責務の漏出 (§G-581)
  - **課題**: 一時ファイル除外を `:(exclude)` スクリプトに依存
  - **提案**: `.gitignore` に一時パスパターンを明示的定義

- [ ] **WF-08** (MEDIUM): 人間の検証（Verification）の形骸化 (§G-624)
  - **課題**: AI が `verification.md` に適当な文字列を書いて CI 通過可能
  - **提案**: AI からの書き込みをハードブロック、または人間の対話的承認ゲート

- [ ] **WF-08a** (MEDIUM): Track 完了状態と Verification 完了の型不整合
  - **課題**: `metadata.json` が `done` なら `registry.md` の Completed Tracks に載るため、`verification.md` が未実質完了でも完了扱いが表示されうる
  - **提案**: `done` 遷移に verification 完了を必須化する、または `VerifiedDone` 相当の状態/フラグを SSoT に導入

- [ ] **WF-08b** (HIGH): Track 終了条件の厳格化 — acceptance criteria の実動作未検証
  - **課題**: review-json-per-group-review-2026-03-29 で「record-round は review.json に記録」「metadata.json に review state を保存しない」という acceptance criteria が明記されていたにもかかわらず、実際には record-round が metadata.json にしか書いておらず review.json が一度も作成されない状態で Done になった。CI パス + reviewer zero_findings のみでは acceptance criteria の実動作を保証できない
  - **根拠**: build 漏れ（bin/sotp 未リビルド）が直接原因だが、acceptance criteria に対する end-to-end 検証がワークフローに組み込まれていないことが構造的問題
  - **提案**: (1) `/track:commit` 時に spec.json の acceptance_criteria を人間に明示的確認させるインタラクティブゲート（`verification.md` の実行を強制）、(2) acceptance criteria のうち自動検証可能なものは `sotp verify acceptance-criteria` として CI に組み込み、手動確認が必要なものは `verification.md` の `verified_at` 記入を `done` 遷移の前提条件にする、(3) `build-sotp` を `/track:commit` の前提条件に追加して bin/sotp の陳腐化を防止
  - **出典**: 2026-03-30 review-json-per-group-review 不具合調査

- [ ] **WF-26** (LOW): `find_open_pr_with` の First-Match Bias（一意性検証欠如）
  - **課題**: `gh_cli.rs` の `find_open_pr_with` が `.[0].number` で先頭要素を無条件取得。同一 head branch に複数 PR がある場合に意図しない PR を操作するリスク
  - **提案**: 配列長を検証し、複数 PR 存在時はエラーまたは警告を返す
  - **出典**: NotebookLM 2026-03-16 指摘 4.2

- [ ] **WF-27** (MEDIUM): `parse_dirty_worktree_paths` のクォート/エスケープ未処理
  - **課題**: `worktree_guard.rs` のパーサーが Git のクォート付きファイル名（スペース・日本語等の非 ASCII）を未処理。`"path with spaces.rs"` がクォート込みで記録され、許可リストと不一致
  - **提案**: Git の C-style unquote を実装し、クォートを除去してからパス比較する
  - **出典**: NotebookLM 2026-03-16 指摘 6.1

- [ ] **WF-28** (LOW): `git notes add -f` ハードコードによるデータロスリスク
  - **課題**: `apps/cli/src/commands/git.rs:255` で `-f` が無条件使用。既存の手動 Notes や別プロセスの Notes をサイレント上書き
  - **提案**: `--force` をオプション化、または append モード（`git notes append`）を検討
  - **出典**: NotebookLM 2026-03-16 指摘 6.2

- [ ] **WF-29** (LOW): commit → note の partial failure ロールバック欠如
  - **課題**: `commit_from_file` と `note_from_file` が独立サブコマンド。コミット成功後の note 失敗でトレーサビリティ欠損状態になるが、自動ロールバックなし
  - **提案**: note 失敗時の警告強化、または `track-commit-message` 内で commit+note をアトミック化
  - **出典**: NotebookLM 2026-03-16 指摘 6.5

- [x] ~~**WF-40** (MEDIUM): `done → done` 遷移で commit_hash 埋め戻しができない~~ ✅ 修正済み (done-hash-backfill-2026-03-20 Phase A: DonePending/DoneTraced split + BackfillHash transition)

### G-3. CI・検証

- [ ] **WF-12** (LOW): テストランナー設定の混在 (§G-387)
  - **課題**: nextest と cargo test の混在でデバッグ時の挙動が変化
  - **提案**: `nextest run --nocapture` に統一

- [ ] **WF-25** (MEDIUM): CI ゲートにカバレッジ目標の強制が存在しない
  - **根拠**: ルール文書では「新規コード 80% 以上」を掲げているが、`ci-local` / `ci` は `llvm-cov-local` を依存に含めていない。
  - **提案**: PR/track 完了時のみ coverage gate を opt-in 導入し、しきい値違反を fail させるか差分カバレッジをレポート必須化する。

### G-3a. PR レビュー

- [x] ~~**WF-30** (MEDIUM): PR body findings パーサーが限定バレット形式のみ認識~~ ✅ 修正済み (T004)
  - **対応**: `- `, `* `, `•`, `+ `, 番号付きリスト (`1. `) をすべて認識。CommonMark 準拠コードブロック除外（backtick/tilde フェンス対応）も同時実装。テスト多数追加済み
  - **出典**: NotebookLM 2026-03-16 指摘 7.3

- [x] ~~**WF-42** (HIGH): `/track:pr-review` が Codex Cloud のレビューを検出できない~~ ✅ 修正済み (PR #38)
  - **対応**: `chatgpt-codex-connector[bot]` を `CODEX_BOT_LOGINS` に追加。`list_reactions` API で zero-findings 検出（reaction + comment text fallback）。`poll_review_for_cycle` に timeout recovery + commit-scoped フィルタ追加。
  - **出典**: PR #38 (review-workflow-fixes-2026-03-18)

- [ ] **WF-42-residual** (LOW): zero-findings 検出の commit scope 制約
  - **課題**: GitHub Reactions/Issue Comments API に `commit_id` フィールドがない。`+1` reaction や "Didn't find any major issues" コメントが PR レベルのシグナルであり、特定コミットに紐付けできない。`trigger_dt` フィルタで緩和しているが、新コミット push 後にトリガー前のレビュー結果が残っている場合に理論的に誤検出の可能性がある。
  - **緩和策**: `head_commit` が既知の場合のみ zero-findings を受け入れ、standalone `poll_review` ではスキップ。`trigger_dt` フィルタで時間窓を最小化。
  - **根本解決**: GitHub API が reactions/comments に `commit_id` を含めるまで不可能。または review API のみに依存する設計に移行。
  - **出典**: Codex Cloud PR review rounds 11-24 (2026-03-18〜19)

- [ ] **WF-42-residual-2** (LOW): private index swap 時の concurrent index update 喪失
  - **課題**: `PrivateIndex` は操作開始時にインデックスをスナップショットし、最終的に `fs::rename` で実インデックスを置き換える。この間に他プロセスが `git add` した変更は失われる。
  - **緩和策**: Claude Code の `block-direct-git-ops` hook が通常ワークフローでの uncontrolled staging を防止。`swap_into_real` が git の `index.lock` プロトコル（`O_CREAT|O_EXCL` → copy → rename）を使用して concurrent git 操作と排他。
  - **根本解決**: git index の3-way merge（snapshot と real の差分を統合）。ただし実装コストが高く、単一オペレーターワークフローでは不要。
  - **出典**: Codex Cloud PR review rounds 14-24 (2026-03-18〜19)

- [ ] **WF-80** (MEDIUM): PR review finding parser が Codex Cloud ボイラープレートを P1 finding として誤検出
  - **課題**: `sotp pr review-cycle` の finding parser が、Codex Cloud の初回レビュー時に付与される使い方説明（「Open a pull request」「Mark a draft as ready」「Comment @codex review」）を `[P1] general:` finding としてパースする。これにより orchestrator が不要な Accepted Deviations を PR body に追加してしまう
  - **提案**: `apps/cli/src/commands/pr.rs` の finding 抽出ロジックで Codex Cloud ボイラープレートパターンをフィルタするか、body セクション境界で「レビュー結果」と「使い方説明」を分離する
  - **出典**: PR #80 planner-claude-migration-2026-04-07 (2026-04-06)

### G-3a2. ローカルレビュー整合性

- [ ] **WF-61** (MEDIUM): `record-round` に渡す verdict の虚偽申告を検出できない
  - **根拠** (2026-03-24 CC-SDD-01 review): LLM が reviewer session log の実際の verdict と異なる値を `record-round` に渡した場合、domain は検出できない。`check-approved` は `Approved` を返してしまう
  - **提案**: `record-round` が reviewer session log のパス（`tmp/reviewer-runtime/codex-session-*.log`）を受け取り、log 末尾の JSON verdict を独自にパースして、渡された verdict と一致するか検証する（verdict attestation）
  - **緩和策**: 現在は `check-approved` の code hash 検証 + `/track:review` skill の Step 4.3 (Review state guard verification) で間接的に防止

### G-3b. アクティベーション

- [ ] **WF-31** (LOW): `track:activate` の部分失敗による main 汚染
  - **課題**: activation commit を main 上で作成した後、`git switch` が失敗すると main に不要なコミットが残留。自動ロールバックなし
  - **提案**: ブランチ作成を先に検証するか、switch 失敗時の `git reset --soft HEAD~1` ロールバックを導入
  - **出典**: NotebookLM 2026-03-16 指摘 7.5

### G-3c. 入力バリデーション

- [ ] **WF-32** (LOW): `TrackId` / `TaskId` の最大長制限なし
  - **課題**: `ids.rs` に文字数上限チェックがなく、LLM が極端に長い ID を生成するとファイルシステムのパス長制限超過で I/O エラー
  - **提案**: `TrackId` に 80 文字程度の上限バリデーションを追加
  - **出典**: NotebookLM 2026-03-16 指摘 8.1

- [x] ~~**WF-33** (MEDIUM): `resolve_track_id_from_branch` のバリデーション欠如~~ ✅ 修正済み (T003)
  - **対応**: 抽出後に `TrackId::new(slug)` でバリデーション実行。失敗時は `TrackResolutionError::InvalidTrackId` を返却。スペース含み・空文字のテスト追加済み
  - **出典**: NotebookLM 2026-03-16 指摘 9.2

### G-4. エージェント協調

- [ ] **WF-16** (MEDIUM): LLM 間の伝言ゲーム依存 (§G-675)
  - **課題**: Canonical Blocks の verbatim コピーを LLM に委ねる
  - **提案**: 構造化データ (JSON) + システム側テンプレートレンダリング

- [ ] **WF-17** (MEDIUM): 非同期キューと同期コミットの Traceability 喪失 (§G-685)
  - **課題**: `pending-note.md` が単一ファイルで上書き → 以前のタスク Notes 消失
  - **提案**: タスク ID 別ファイル、またはコミット時にバッチマージ

- [ ] **WF-18** (LOW): セキュリティ検証の設定-コード分離違反 (§G-695)
  - **課題**: `EXPECTED_DENY` を Python コードにハードコード
  - **提案**: 設定ファイル駆動のデータ駆動型バリデーション

- [ ] **WF-22** (MEDIUM): マルチエージェント間の会話履歴非共有 (§G-781)
  - **課題**: エージェント間の引き継ぎがファイルベースのみ → 暗黙の意図が喪失
  - **提案**: セッション間コンテキスト共有メカニズム（JSON ペイロード）

- [x] ~~**WF-34** (MEDIUM): Claude/Codex capability 配置の最適化~~ ✅ Phase 1 完了 (planner-claude-migration-2026-04-07)
  - **Phase 1 対応**: default profile の `planner` を Claude (Opus, `--bare -p`) に移行。config + doc 変更のみ
  - **Phase 2 残**: hexagonal 統一 resolver + domain 型定義（下記 WF-34-phase2 参照）

- [ ] **WF-34-phase2** (MEDIUM): Planner hexagonal architecture + 統一 config resolver
  - **課題**: provider/model 解決が Rust (`agent_profiles.rs`), Python (`_agent_profiles.py`), raw JSON (`pr_review.rs`) の 3 箇所に分散しており、解決ルールが不整合になるリスクがある
  - **提案**: domain 層に `AgentProfiles` / `Capability` / `ProviderName` 型を定義し、usecase に `Planner` port trait、infrastructure に `CodexPlanner` / `ClaudePlanner` adapter を配置。`sotp plan auto` で config ベース auto-dispatch
  - **設計資料**: `knowledge/research/2026-04-07-1040-planner-claude-migration-design.md`（Canonical Blocks 含む）
  - **出典**: Codex planner design review (2026-04-06)
  - **実装**: `agent-profiles.json` に新 profile（例: `claude-planner`）を追加し A/B 比較で効果測定
  - **トレードオフ**: Claude 集中でコンテキスト切替コスト削減・レイテンシ改善が見込める一方、多様な視点が減る

- [x] ~~**WF-35** (HIGH): FORBIDDEN_ALLOW から読み取り専用コマンドを解禁 + git 読み取りコマンド allow 追加~~ ✅ 修正済み
  - **対応**: `head`/`tail`/`wc` を FORBIDDEN_ALLOW から削除し `permissions.allow` + `EXPECTED_OTHER_ALLOW` に移行。git 読み取りコマンド（`git status`, `git diff`, `git log`, `git branch --list`, `git rev-parse`, `git show`, `git ls-files`, `git notes show/list`）も allow 済み。`sort`/`uniq` は `sort -o` の書き込みリスクにより除外を維持

- [ ] **WF-36** (HIGH): Review Escalation Threshold の機構化 → [詳細](./refactoring-plan-2026-03-19.md) §4
- [x] ~~**WF-43** (CRITICAL): `record-round` → `check-approved` の code_hash 自己参照循環~~ ✅ 修正済み (PR #38)
- [ ] **WF-62** (~~MEDIUM~~ → LOW, legacy model — production 未使用): `ReviewState::record_round` が Approved→Fast findings_remain で降格しない → [詳細](./refactoring-plan-2026-03-19.md) §4 (旧 WF-40)
- [ ] **WF-41** (LOW): `review_from_document` が偽の Fast ラウンドを合成 → [詳細](./refactoring-plan-2026-03-19.md) §4
- [ ] **WF-38** (LOW): frontmatter パーサーの duplicate key 未検出 → [詳細](./refactoring-plan-2026-03-19.md) §3
- [ ] **WF-39** (MEDIUM): `/track:catchup` の責務分割 — bootstrap と briefing の分離 → [詳細](./refactoring-plan-2026-03-19.md) §4
- [ ] **WF-37** (MEDIUM): `argv_has_rm` のランチャー後走査が過剰（false positive） → [詳細](./refactoring-plan-2026-03-19.md) §3
- [x] ~~**WF-54** (MEDIUM): `track-commit-message` の review guard が planning artifacts 初回コミットをブロックする~~ ✅ done (PR #46, ci-guardrails-phase15-2026-03-20)
  - **対応**: (B) を採用。`/track:plan` が metadata.json 作成時に review state `{status: "not_started", groups: {}}` を含める。`check-approved` が `NotStarted && groups.is_empty()` を許可（初回状態のみ、降格後は不可）

- [ ] **WF-65** (HIGH): 実装フェーズでの設計判断エスカレーションフロー未定義
  - **課題**: 実装者が spec/ADR に記載のない設計判断を独断で追加し、レビューで初めて発覚する（例: `FinalRequiresFastPassed` 制約）。未承認の制約がドメイン層に入り品質が低下
  - **提案**: 実装中に仕様外の設計判断が必要になった場合、実装を一時停止して設計フェーズに戻り spec.md/ADR を更新するエスカレーションフローを `/track:implement` と `track/workflow.md` に定義する。判断基準: (1) 新しい enum variant / error type の追加、(2) 新しい状態遷移制約の追加、(3) 新しい port/trait の追加 — これらは実装者の裁量ではなく設計書への反映が必要
  - **根拠**: review-port-separation track で `FinalRequiresFastPassed` が ADR/spec に記載なく実装に入り、後から除去が必要になった事例

- [ ] **WF-56** (MEDIUM): planning-only コミットガードが ADR を拒否する
  - **課題**: `track-commit-message` の planning-only allowlist が `knowledge/adr/` を許可しない。track artifacts と ADR を同時にコミットできず、ユーザーが手動 `git commit` に頼る必要がある
  - **提案**: planning-only allowlist に `knowledge/adr/` と `knowledge/strategy/` を追加する。ADR は planning artifacts の一部であり、track と一緒にコミットされるべき
  - **根拠**: autorecord-stabilization-2026-03-26 plan branch でコミット不能が発生（2026-03-27）

- [ ] **WF-57** (LOW): `.lock` ファイル残骸が `verify-latest-track` を阻害する
  - **課題**: 完了済みトラックの `metadata.json.lock`（fs4 ロック）が cleanup されずに残留。ディレクトリの存在だけで `verify-latest-track` がトラックを検出し、`metadata.json` 不在エラーとなる
  - **提案**: (1) `/track:done` や `/track:archive` で `.lock` ファイルを cleanup する。(2) `verify-latest-track` が `.lock` のみのディレクトリをスキップする
  - **根拠**: `review-json-separation-2026-03-25` で発生（2026-03-27）

- [ ] **WF-58** (MEDIUM): `!` prefix コマンドが Claude Code ツールセッションの git state に反映されない
  - **課題**: ユーザーが `! git restore --staged` や `! git rm --cached` を実行しても、Claude Code の Bash ツールから見える git index が更新されない。unstage 操作が hook でブロックされる場合の回避策として `!` を使うが、効果が確認できない
  - **提案**: `cargo make` に unstage ラッパー（例: `cargo make unstage <path>`）を追加する
  - **根拠**: review.json の unstage で発生（2026-03-27）

- [x] ~~**WF-59** (HIGH): review hash の index 全体依存による auto-record 不安定性~~ ✅ done (`autorecord-stabilization-2026-03-26` で review-scope manifest hash を実装、後続トラックで運用)
  - **対応**: ADR-2026-03-26-0000 に基づき review-scope manifest hash に移行。`track/review-scope.json` による scope-aware 分類、worktree 直接読み取り、volatile field 正規化、`rvw1:sha256:<hex>` 形式を実装。`review-json-per-group-review-2026-03-29` + `autorecord-reviewjson-wiring-2026-03-30` で per-group scope hash として運用

- [x] ~~**WF-66** (MEDIUM): `track-pr-push` のタスク完了ガードが中間 push をブロックする~~ ✅ done (`pr-review-flow-fix-2026-03-31`)
  - **対応**: タスク完了ガードを push/review_cycle から wait_and_merge_with (merge 前) に移動。中間 push と PR review が未完了タスクでもブロックされなくなった

- [ ] **WF-60** (HIGH): 設計⇆実装の自動遷移 — reviewer finding が spec スコープ外を指摘した場合に、自動で planner に設計相談を escalation し、ADR/spec 更新後に実装に戻るフロー。autorecord-stabilization トラックでは spec 外の修正が大量に蓄積し事後的に spec を更新する事態になった。`/track:review` スキル内に scope guard + auto-escalation to planner を組み込み、(1) finding が spec の in_scope/out_of_scope に該当するか自動判定、(2) scope 外 → planner に設計相談を自動起動、(3) planner が spec 更新 or 別トラック化を判断、(4) spec 更新後に実装に復帰。手動介入なしに設計と実装のスコープ整合性を維持する仕組み

---

## H. 戦略的提案

> H-1 (Rust 化パラダイムシフト) と H-2 (Harness v2 構想) は STRAT-03/09 で実現済み。
> Phase 配置と実行順は [`knowledge/strategy/TODO-PLAN.md`](TODO-PLAN.md) Phase 6 + 依存関係を参照。

### H-3. 方針を明示した戦略 TODO

- [x] ~~**STRAT-02** (HIGH): ファイルロックを Rust デーモンへ集約~~ **スコープ除外 (2026-03-19)**
  - **理由**: ファイルロックシステムが休眠状態（`SOTP_LOCK_ENABLED=1` 未設定）のまま 21 トラックが完了。worktree 分離 (SPEC-04) で並行実行の排他制御は物理的に解決されるため YAGNI 判断。必要になった時点で再評価

- [x] ~~**STRAT-03** (HIGH): Python 依存からの脱却~~ ✅ 全 Phase 完了 (2026-03-16)
  - ~~Phase 1: セキュリティ境界の Rust 完全移行~~ ✅ (`python-hook-launcher-removal-2026-03-15`)
  - ~~Phase 2: Track state machine の Rust 化~~ ✅ (`statemachine-rust-2026-03-15`)
  - ~~Phase 3: Git workflow wrapper の Rust 化~~ ✅ (`py-workflow-cleanup-2026-03-16`)
  - ~~Phase 4: PR review orchestration の Rust 化~~ ✅ (`pr-review-rust-2026-03-16`)
  - ~~Phase 5: verify script 群の Rust 化~~ ✅ (`verify-scripts-rust-2026-03-16`)
  - ~~Phase 6: 残留 Python の optional utility 化~~ ✅ (`python-optional-2026-03-16`)
  - **マイルストーン**: M1 ✅, M2 ✅, M3 ✅, M4 ✅

- [x] ~~**STRAT-04** (MEDIUM): `registry.md` を Git 管理対象外にし完全生成ビューへ移行~~ ✅ done (PR #46, ci-guardrails-phase15-2026-03-20)
  - **対応**: `.gitignore` に `track/registry.md` を追加。`cargo make track-sync-views` で生成のみ。CI は `verify-track-registry` を metadata.json ベースに移行予定

- [ ] **STRAT-05** (HIGH): SSoT (`metadata.json`) と Git 履歴/コミット状態の整合戦略を定義する
  - **背景**: `metadata.json` は論理状態の SSoT だが、実体コードは Git 履歴/working tree に存在する。`done` 済み未コミット、`git revert/reset`、競合解消後の巻き戻りで state drift が起こりうる。
  - **方針**: 「タスク状態」と「Git 保存状態」の関係を型/状態遷移として定義し、必要なら `VerifiedDone` や `CommittedDone` のような明示状態を導入する。

- [x] ~~**STRAT-06** (MEDIUM): generated view の Git 管理方針を `registry.md` だけでなく `plan.md` まで統一する~~ ✅ 方針確定 (ci-guardrails-phase15-2026-03-20)
  - **決定**: registry.md は gitignore 化（共有リソースで diff ノイズ・マージ競合が多い）。plan.md は git tracked のまま維持（トラック専属で PR diff が有用）+ `sotp verify view-freshness` で SSoT との一貫性を CI 保証。用途の違いに基づく意図的な二重方針。

- [ ] **STRAT-07** (HIGH): `worktree`・Docker Compose・CI の実行モデルを統一する
  - **背景**: 並列作業では `git worktree` を使いたい一方、現在の compose/CI 周辺はリポジトリ直下の `.git` ディレクトリ前提が残り、worktree と構造的に噛み合わない。
  - **方針**: 「本リポジトリ直下」「worktree」「CI container」のどこでも同じコマンド体系が動く実行モデルへ寄せる。

- [ ] **STRAT-08** (MEDIUM): 外部非同期システム連携の state 永続化原則を定義する
  - **背景**: 現在は `/track:pr-review` のような外部 API 連携が同期ポーリング主体で、中断時の再開情報を保持しない。
  - **方針**: GitHub PR review などの外部非同期処理は、必ず run-id / trigger-id / status / timestamps を永続化し、resume/reconcile 可能にする。

- [x] **STRAT-09** (MEDIUM): shell wrapper / `cargo make` 依存の縮退 ✅ 完了
  - **背景**: `Makefile.toml` の `script_runner = "@shell"` と `$CARGO_MAKE_TASK_ARGS` 展開が多く、責務分散・quoting 脆弱性・追跡困難性を生んでいる。
  - **方針**: 安全性や状態管理に関わる wrapper は shell 文字列組み立てをやめ、Rust CLI の引数パースへ集約する。
  - **結果**: `sotp make` サブコマンド (26タスク) を新設。28タスクを `command + args` 形式に移行。T011-T012 (-exec daemon) は ROI が低いためスキップ。PR #30 でマージ済み。

- [ ] **STRAT-10** (MEDIUM): トラック archive 後の Git ブランチ寿命管理を定義する
  - **背景**: `track/archive/<id>` への物理移動後も `track/<id>` ブランチは残り続け、ファイル寿命と Git ref 寿命が分離している。
  - **方針**: archive, merge, branch cleanup の責務境界を定義し、少なくとも「残す」「削除候補として表示」「手動 cleanup を必須化」のどれかをシステムとして明示する。

- [ ] **STRAT-11** (MEDIUM): 多言語プロジェクト対応 — ハーネスの言語非依存化設計
  - **詳細設計書**: [`tmp/multi-language-design-2026-03-18.md`](./multi-language-design-2026-03-18.md)
  - **背景**: ハーネスの核は言語非依存だが、CI ゲート・ルール・レイヤーチェックが Rust にハードコード。他言語プロジェクトでの利用が困難
  - **方針**: 言語固有部分をプラグイン的に差し替え可能な設計へ移行（`harness.toml` + `.claude/rules/lang/` + 言語別 CI タスク）
  - **実装ロードマップ**: Phase A（設定ファイル）→ B（ルール分離）→ C（CI ディスパッチ）→ D（テンプレートジェネレータ）
  - **フレームワーク別相性**: Laravel ◎, React/Next.js ◎, FSD+Next.js ◎◎ — 詳細は設計書 §7-8 参照
  - **関連**: SPEC-08, GAP-08

### H-4. 実行順

> 実行順は [`knowledge/strategy/TODO-PLAN.md`](TODO-PLAN.md) Phase 4/6 に統合済み。

---

## I. 仕様フィードバック・権限委譲 (SPEC)

> **出典**: NotebookLM 音声分析 2026-03-18（3本）。分析レポート: `tmp/ma4/2026-03-18/analysis-report*.md`
> Phase 配置は [`knowledge/strategy/TODO-PLAN.md`](TODO-PLAN.md) Phase 2〜4 を参照

- [ ] **SPEC-01** (HIGH): Runtime Spec Evolution — 実装失敗→信号機自動降格ループ → Phase 2
- [ ] **SPEC-02** (HIGH): 信号機ベース権限委譲ルーター — 人間/AI 境界の自動決定 → Phase 2
- [ ] **SPEC-03** (MEDIUM): 信号機 🟡→🔵 自動昇格を CI 客観証拠に限定 → Phase 3
- [ ] **SPEC-04** (MEDIUM): エフェメラル worktree 分離 — `/track:full-cycle` の物理的隔離 → Phase 4
- [x] ~~**SPEC-05** (HIGH): Domain States 信号機 Stage 2 — per-state signal + 遷移関数検証 + spec.json `domain_state_signals`~~
  - **完了**: syn AST 2-pass スキャン, transitions_to 検証, red==0 gate, Stage 1 前提条件チェック
  - **完了日**: 2026-03-23 (PR #58)
- [ ] **SPEC-06** (HIGH): リカバリー3層タクソノミー (Continuation/Rollback/Clean Restart) → Phase 2
- [ ] **SPEC-07** (MEDIUM): Phase 0 不変条件宣言 (隔離・リセット・垂直スライス・HitL) → Phase 0
- [ ] **SPEC-08** (MEDIUM): 垂直スライス原則 — 各 Phase 項目に検証コマンドを同時デプロイ
- [ ] **SPEC-09** (LOW): Human-in-the-Loop マニフェストを TODO-PLAN 冒頭に追加

---

## J. Gemini 構造的 Gap 分析 (GAP)

> **分析レポート**: `knowledge/research/gemini-gap-analysis-2026-03-18.md`
> Phase 配置は [`knowledge/strategy/TODO-PLAN.md`](TODO-PLAN.md) Phase 4-5 を参照
> リファクタリング関連 (GAP-03/04) は [`tmp/refactoring-plan-2026-03-19.md`](./refactoring-plan-2026-03-19.md) §5 を参照

- [x] ~~**GAP-01** (HIGH): `DocumentMeta` タイムスタンプの型化 (`String` → `chrono::DateTime<Utc>`)~~ ✅ done (PR #42)
- [ ] **GAP-02** (MEDIUM): PR State Machine をドメイン層に導入 → Phase 5
- [x] ~~**GAP-03** (LOW): `ReviewVerdict` / `ReviewPayloadVerdict` の重複~~ ✅ DM-01 で統合済み (PR #42)
- [ ] **GAP-04** (MEDIUM): `StatusOverride` の表現力不足 → [詳細](./refactoring-plan-2026-03-19.md) §5
- [x] ~~**GAP-05** (HIGH): `is_test_file` のパス正規化欠如~~ ✅ done (PR #39)
- [x] ~~**GAP-06** (LOW): `#![forbid(unsafe_code)]` 設定~~ ✅ done (PR #39)
- [ ] **GAP-07** (MEDIUM): `AgentId` 衝突防止 (UUID v7 or PID+timestamp) → Phase 4
- [ ] **GAP-08** (MEDIUM): Hook バッチ化 (`sotp hook dispatch-all`) → Phase 4
- [ ] **GAP-09** (LOW): `verify-latest-track` をコンテナ実行に統一 → Phase 5
- [ ] **GAP-10** (MEDIUM): spec attribution 検証を Scope/Constraints まで拡張 → Phase 5
- [ ] **GAP-11** (MEDIUM): `tracing` 導入 (`eprintln!` → 構造化ログ) → Phase 5

---

## 優先順位・実行計画

> Tier 分類と Phase 配置は [`knowledge/strategy/TODO-PLAN.md`](TODO-PLAN.md) に統合済み。
> リファクタリング関連の導入順序は [`tmp/refactoring-plan-2026-03-19.md`](./refactoring-plan-2026-03-19.md) §10 を参照。
> 解決済み項目の詳細は `tmp/TODO-archived-2026-03-16.md` を参照。

---

## 追加 TODO（track 外）

> ワークフロー改善系。関連する既存 TODO がある場合は ID を併記。

- [ ] **SKILL.md と commands の重複解消** (MEDIUM) — `.claude/skills/track-plan/SKILL.md` と `.claude/commands/track/plan.md` に実行手順が重複しており、片方を更新するともう片方が古くなる。SKILL.md はスキルルーター用 description + Phase 概要のみに縮退し、実行仕様は commands に委譲する。他の track:* スキル/コマンドペアも同様に整理
- [ ] **`/track:hotfix` コマンド** (MEDIUM) — ドキュメントのみ・1 ファイル修正などの軽微な変更にレビューサイクルをスキップして直接コミット可能にする。対象ファイルパターン（`*.md`, `LICENSE` 等）で許可範囲を制限。現状は README 1 行変更でもトラック作成 → レビュー → PR → マージが必要でオーバーヘッドが大きい
- [ ] **LICENSE ファイル追加** (LOW) — `LICENSE-MIT` + `LICENSE-APACHE` を作成し、全 `Cargo.toml` の `license` フィールドに `MIT OR Apache-2.0` を設定
- [ ] **TDD 状態マシンの強制** (HIGH) — `metadata.json` の task に `tdd_phase: red|green|refactor` を追加し、`sotp tdd advance` で CI 証拠付きの遷移を強制。Red→Green は CI fail 証拠、Green→Refactor は CI pass 証拠が必要。commit guard は `tdd_required` タスクの `tdd_phase` 完了を検査。→ [詳細](./refactoring-plan-2026-03-19.md) §0-7。関連: HARNESS-03, WORKFLOW-04
- [ ] **Yes/No 承認ダイアログの最小化** — `permissions.allow` に wrapper を事前登録。WF-35 (FORBIDDEN_ALLOW 緩和) で一部対応済み
- [ ] **/track:review の reviewer provider 移譲強制** — hook で外部 subprocess 呼び出しを検証。関連: CLAUDE-BP-02, 10-guardrails.md
- [ ] **track-local-review 出力改善** — verdict JSON を `tmp/reviews/<track-id>/round-<N>.json` に自動保存。関連: RVW-07
- [ ] **レビュー結果の蓄積** — round 別 JSON 蓄積 + `track-review-history` タスク。関連: RVW-06
- [x] ~~**Full model reviewer の --full-auto 必須化**~~ ✅ 解決済み (PR #29)
- [ ] **PR body の自動更新** — `track-pr-push` 時に `gh pr edit --body-file` で body を再生成
- [x] ~~**registry.md のコミット時自動再生成**~~ ✅ 不要化 — registry.md は gitignore 化 (STRAT-04, PR #46)。コミット対象外のため自動再生成は不要
- [ ] **reviewer subagent の Bash timeout 10分引き上げ** — `/track:review` スキルまたは wrapper で設定
- [ ] **track:activate の clean-worktree チェック再評価** (MEDIUM) — `persist_activation_commit()` は `git commit --only` で指定ファイルのみコミットするため、dirty worktree でも activation commit に無関係ファイルは混入しない。`activation_requires_clean_worktree` + `allowed_activation_dirty_paths` の allowlist 機構が本当に必要か検討。不要なら削除して activate.rs を簡素化。関連: ERR-09b（activate.rs モジュール分割）
- [ ] **Review escalation enforcement の機構化** (HIGH) — `record-round` に `--model-tier fast|full` フラグを追加し、domain 層で「全グループが fast zero_findings → full zero_findings の 2 段階を経たか」を追跡。`check-approved` が full model 確認なしのグループを拒否。現状はプロンプト依存で fast model pass のみでコミットできるすり抜けが発生した (2026-03-24 発見)。関連: WF-36, RVW-06
- [ ] **track-local-review のモデル自動解決** (MEDIUM) — `review.md` にモデル解決フォールバックルールを散文で記載するのは DRY 違反・CI 検証不能・ドリフト必至。`cargo make track-local-review --track-id <id> --group <scope> --briefing-file <path>` だけで CLI が `agent-profiles.json` を読み `fast_model` / `default_model` / `--round-type` を自動解決すべき。関連: WF-34-phase2
- [ ] **`designer` capability + `/track:design` コマンド** (HIGH) — track `reverse-signal-integration-2026-04-08` に統合済み (T05-T07)
- [ ] **`sotp review scope-files --group <name>` コマンド** (MEDIUM) — 指定グループに属する変更ファイルリストを出力する。`partition()` ロジックは既に `bin/sotp` 内にあるのでサブコマンド追加で実現可能。review-fix-lead エージェントへの scope allowlist 受け渡しを自動化する。関連: track-local-review のモデル自動解決

---

## CLI エラーハンドリング改善（未完了分）

> 詳細は [`tmp/refactoring-plan-2026-03-19.md`](./refactoring-plan-2026-03-19.md) §7 を参照

- [ ] **ERR-09b** (MEDIUM): `track/activate.rs` (1890行) のモジュール分割 → `branch.rs` / `preflight.rs` / `resume.rs`（旧 `commands/activate.rs` → `commands/track/activate.rs` に移動済み）
- [ ] **ERR-11** (LOW): `tracing` クレート導入（前提: `tech-stack.md` 更新）
- [ ] **ERR-13** (LOW): コンテナ内 `-local` タスクの `cargo run` → `bin/sotp` 置換
- [ ] **ERR-14** (LOW): `_agent_profiles.py` に `fast_model` バリデーション追加
- [ ] **ERR-15** (MEDIUM): `bin/sotp` の staleness 検出
- [ ] **ERR-16** (LOW): `test_make_wrappers.py` のテストカバレッジ拡大

---

## HUMAN_MEMO.md からの追加項目（2026-03-15 整理）

> コード品質・責務分離の詳細は [`tmp/refactoring-plan-2026-03-19.md`](./refactoring-plan-2026-03-19.md) §6 を参照

### 並行処理・ファイル競合

- [ ] **MEMO-01** (MEDIUM): 並列レビュー時のブリーフィングファイル同時書き込み → ランID/セッションID で分離

### アーキテクチャ・コード品質

- [ ] **MEMO-02** (MEDIUM): `DESIGN.md` 肥大化 → トラック別分割、インデックス化
- [ ] **MEMO-03** (MEDIUM): CLI→domain 直接参照禁止 → `architecture-rules.json` / `deny.toml` にルール追加
- [ ] **MEMO-04** (LOW): `validator`/`strum`/`derive_more` による簡約（計画着手時に調査）
- [ ] **MEMO-05** (LOW): `.claude/rules/` の完全英語化
- [ ] **MEMO-06** (LOW): 肥大化リスクファイル分割（`error.rs`, `codec.rs`, `fs_store.rs`, `track.rs`）

---

## NotebookLM レビュー追記（2026-03-13-001 のトリアージ）

> 詳細は `tmp/TODO-archived-2026-03-16.md` の「既存 TODO で管理済み」「不採用」セクションを参照

## NotebookLM レビュー追記（2026-03-16 のトリアージ）

> 詳細は `tmp/NotebookLM-review-2026-03-16-triage.md` を参照

新規 TODO 化した 13 件:
- `SEC-12`: External Guides チェックサム/staleness 検知欠如
- `SEC-13`: conch-parser ベンダリング保守方針
- `SSoT-09`: TrackDocumentV2 の未知フィールド消失
- `SSoT-10`: collect_track_branch_claims の単一破損ファイル全停止
- `INF-13`: atomic_write のパーミッション未転写
- `INF-14`: ホスト Rust ツールチェーン依存
- `WF-26`: find_open_pr_with の First-Match Bias
- `WF-27`: parse_dirty_worktree_paths のクォート未処理
- `WF-28`: git notes add -f ハードコード
- `WF-29`: commit → note の partial failure
- `WF-30`: PR body findings パーサーの限定バレット
- `WF-31`: track:activate の部分失敗 main 汚染
- `WF-32`: TrackId/TaskId の最大長制限なし
- `WF-33`: resolve_track_id_from_branch のバリデーション欠如

既存 TODO で管理済み（10件）、不採用（21件）、事実誤認（4件）。

---

## NotebookLM 提案・音声レビュー

> 詳細は以下を参照:
> - `STRAT-02` 設計入力: `tmp/NotebookLM-suggestion-oneshot-2026-03-13-001.md`
> - アーキテクチャ提案 (4件): `tmp/ma4/2026-03-16/` 配下トリアージファイル
> - 音声レビュー (2026-03-16): `tmp/ma4/2026-03-16/`
> - Phase 配置: [`knowledge/strategy/TODO-PLAN.md`](TODO-PLAN.md) Phase 7 将来構想

**STRAT-02 設計入力** (NotebookLM oneshot 2026-03-13):
Lease/LeaseId モデル、daemon/client 分離、UDS 通信、接続断自動 reap

**アーキテクチャ提案** (NotebookLM 2026-03-16 指摘11):
- AST認識型 3-Way Merge → STRAT-02 延長（ロック改善が先）
- LSP (rust-analyzer) 直結コンテキスト → PoC: `sotp lsp find-references`
- CoW エフェメラルサンドボックス → STRAT-07/SPEC-04 と合流
- 自律的調停プロトコル (Rebuttal + Arbitration) → `/track:review` 拡張

**音声レビュー** (2026-03-16): `/track:auto` 設計入力 3 件 + 既存 TODO 管理済み 3 件。独立 TODO 追加不要。

---

## 外部フレームワーク取り込み候補（HARNESS/Tsumiki/CC-SDD/WORKFLOW/CLAUDE-BP）

> 詳細は [`tmp/adoption-candidates-2026-03-17.md`](./adoption-candidates-2026-03-17.md) を参照
> Phase 配置は [`knowledge/strategy/TODO-PLAN.md`](TODO-PLAN.md) を参照

**Harness Engineering Best Practices 2026**:
- [ ] **HARNESS-01** (MEDIUM): PostToolUse 構造化フィードバック統一 → Phase 5
- [ ] **HARNESS-02** (MEDIUM): Linter 設定ファイル保護 hook
- [ ] **HARNESS-03** (HIGH): Stop hook テスト通過ゲート → Phase 3
- [ ] **HARNESS-04** (LOW): WHY/FIX/EXAMPLE エラーメッセージ
- [ ] **HARNESS-05** (MEDIUM): ADR 導入 → Phase 6
- [ ] **HARNESS-06** (LOW): CLAUDE.md スリム化
- [ ] **HARNESS-07** (LOW): Ghost File / Comment Bloat 検出
- [ ] **HARNESS-08** (MEDIUM): ブートストラップルーチン標準化 → Phase 5
- [ ] **HARNESS-09** (LOW): フィードバックループ速度階層の明文化

**Tsumiki フレームワーク**:
- [x] ~~**TSUMIKI-01** (HIGH): 信号機評価 🔵🟡🔴~~
  - **完了**: Stage 1 (spec signals, PR #55) + Stage 2 (domain state signals, PR #58) + spec.json SSoT (PR #57)
  - **完了日**: 2026-03-23
- [x] ~~**TSUMIKI-02** (MEDIUM): ソース帰属~~ ✅ Phase 1 完了
- [x] ~~**TSUMIKI-03** (MEDIUM): 差分ヒアリング~~ ✅ done (`diff-hearing-2026-03-27`)
- [ ] **TSUMIKI-04** (MEDIUM): TDD 完了時の要件網羅率 → Phase 3
- [x] ~~**TSUMIKI-05** (MEDIUM): 構造化ヒアリング UX — AskUserQuestion + multiSelect による選択肢型質問~~ ✅ done (`hearing-ux-improvement-2026-04-01`)
  - **対応**: SKILL.md Phase 1 に構造化質問ステップを追加。🟡/🔴/❌ 項目を選択肢型質問に変換
- [x] ~~**TSUMIKI-06** (LOW): ヒアリング作業規模選定 — Full/Focused/Quick モード~~ ✅ done (`hearing-ux-improvement-2026-04-01`)
  - **対応**: Step 0 でモード選択を追加。Focused: researcher/planner スキップ、Quick: Blue サマリー表示のみ
- [x] ~~**TSUMIKI-07** (LOW): ヒアリング記録 — spec.json に hearing_history 追加~~ ✅ done (`hearing-ux-improvement-2026-04-01`)
  - **対応**: domain 層に HearingMode/HearingRecord 型追加。spec.json に hearing_history フィールド追加。append-only 設計、content_hash から除外
- [ ] **TSUMIKI-08** (MEDIUM): シグナル伝播 — spec.json の信号を metadata.json タスクに worst-case 伝播。🔴依存タスクの implementing 遷移をブロック → Phase 3

**CC-SDD フレームワーク**:
- [x] **CC-SDD-01** (HIGH): 要件-タスク双方向トレーサビリティ — ✅ done (PR #60, track: req-task-traceability-2026-03-24)
- [x] **CC-SDD-02** (MEDIUM): 明示的承認ゲート — ✅ done (PR #62, track: spec-approval-gate-2026-03-24)
- [ ] **CC-SDD-03** (LOW): EARS 記法
- [ ] **CC-SDD-04** (MEDIUM): Steering 自動生成 → Phase 6
- [ ] **CC-SDD-05** (MEDIUM): 実装検証コマンド → Phase 3

**Review Infrastructure 強化**:
- [x] **RVW-10** (HIGH): レビュー verdict 改竄防止 — ✅ done (PR #63, track: review-verdict-autorecord-2026-03-25)
- [x] **RVW-11** (MEDIUM): レビュー diff スコープ強制 — ✅ done (PR #63, track: review-verdict-autorecord-2026-03-25)
- [ ] **RVW-12** (MEDIUM): 既存コード品質問題の修正 — guard/policy.rs バイパス (P0)、spec_frontmatter.rs trim_end (P1)、git_cli.rs branch_claims skip (P1)
- [ ] **RVW-13** (HIGH): `--auto-record` end-to-end 実戦テスト — `/track:review` で `--auto-record` フラグを使った並列レビューの実運用確認。review.md に記載済みだが未使用
- [ ] **RVW-14** (MEDIUM): path normalization 改善 — 絶対パス→repo-relative 変換（infra で repo root strip 後に usecase に渡す）、renamed file 旧パスの DiffScope 追加（`git diff --diff-filter=R --name-status`）、`looks_decorated()` の精度向上
- [ ] **RVW-15** (MEDIUM): GitDiffScopeProvider テスト強化 — merge-base/staged/unstaged/untracked/rename/delete の契約テスト（tempdir git fixture）
- [ ] **RVW-16** (LOW): escalation block (exit 3) 統合テスト — 実 escalation state を構築して auto-record の exit 3 パスを検証
- [ ] **RVW-17** (MEDIUM): Agent hook empty stdin 根本対策 — `codex-reviewer` agent の `tools:` 制限検証。効かない場合は別の構造的対策（Agent 専用 hook bypass / envelope 生成）を検討
- [ ] **RVW-18** (LOW): `codex-reviewer` agent `tools:` frontmatter 制限の動作検証 — `Bash(cargo make track-local-review:*)` が実際にツール制限として機能するか確認
- [ ] **RVW-19** (LOW): record-round リトライジッターを `rand` クレートに置換 — 現在は `DefaultHasher` + pid + nanosecond で代用。`rand` はデファクトスタンダードなので依存追加して `thread_rng().gen_range(50..250)` に簡約

**Coding Agent Workflow 2026**:
- [ ] **WORKFLOW-01** (MEDIUM): FIC 閾値管理
- [ ] **WORKFLOW-02** (LOW): Best-of-N 並列戦略
- [ ] **WORKFLOW-03** (MEDIUM): ドキュメント鮮度追跡
- [ ] **WORKFLOW-04** (MEDIUM): マイクロタスク分解
- [ ] **WORKFLOW-05** (LOW): Dual-Agent 自動化パターン
- [ ] **WORKFLOW-06** (LOW): CLAUDE.md/rules サイズ検証
- [ ] **WORKFLOW-07** (LOW): 理解負債の軽減策
- [ ] **WORKFLOW-08** (LOW): Spec-as-Source 成熟度モデル

**Claude Code Best Practices (公式)**:
- [ ] **CLAUDE-BP-01** (MEDIUM): PreCompact 圧縮保存指示強化
- [ ] **CLAUDE-BP-02** (MEDIUM): Writer/Reviewer 分離 → [詳細](./refactoring-plan-2026-03-19.md) §8
- [ ] **CLAUDE-BP-03** (LOW): Custom status line
- [ ] **CLAUDE-BP-04** (LOW): Fan-out バッチパターン
- [ ] **CLAUDE-BP-05** (MEDIUM): 2 回修正失敗 → /clear ルール化

---

## L. レビューサイクル品質改善 (RVW)

> 詳細は [`tmp/refactoring-plan-2026-03-19.md`](./refactoring-plan-2026-03-19.md) §2 を参照

- [ ] **RVW-01** (HIGH): 共通 Frontmatter パーサー抽出 → `frontmatter.rs`
- [ ] **RVW-02** (HIGH): conch-parser AST 直接走査による hand-rolled shell 解析の廃止
- [ ] **RVW-03** (MEDIUM): typed deserialization convention + canonical_modules serde 移行
- [ ] **RVW-04** (LOW): `syn` crate による `is_inside_test_module` 置き換え + standalone テストファイル除外
- [ ] **RVW-05** (LOW): `skip_command_launchers` の per-launcher フラグモデリング（RVW-02 で根本解決）
- [ ] **RVW-06** (HIGH): metadata.json にレビュー状態統合 + エスカレーション順序強制 + コミットガード
- [ ] **RVW-07** (HIGH): Codex verdict 抽出の stderr フォールバック + セッションログ保存
- [ ] **RVW-08** (HIGH): diff-scope filter (ScopeFilteredPayload) の削除 — レビューアの findings を勝手にフィルタして捨てるのは不適切。全 findings を漏れなく record-round に渡すべき。対象: `usecase/src/review_workflow/scope.rs` の `ScopeFilteredPayload` および `codex_local.rs` での適用箇所
- [ ] **RVW-09** (~~MEDIUM~~ → LOW, legacy model — production 未使用): review invalidation のスコープ限定 — code_hash が review.json トップレベルに1つだけ存在し、hash mismatch 時に全グループがリセットされる。影響を受けたグループのみ invalidation すべき。対象: `domain/src/review/state.rs` の `invalidate()` メソッド
- [ ] **RVW-20** (HIGH): ACCEPTED finding の仕組み化 + dispute adjudication — orchestrator が briefing に ACCEPTED リストを自由に手書きして reviewer findings を握り潰すパターンを防止。(1) `sotp review accept-finding` CLI コマンドで accepted findings を metadata.json に SSoT 化、(2) `track-local-review` wrapper が briefing 生成時にストアから ACCEPTED を自動構築（手書き ACCEPTED は無視）、(3) briefing に `ACCEPTED` / `DO NOT re-report` を含む場合に hook で reject。`resolve-escalation` と同様に evidence + 理由を要求する。(4) reviewer と実装者の見解が対立する場合の adjudication フロー — `sotp review dispute` で dispute を記録、evidence（git show, ADR ref, テスト結果）を添付、planner capability または user が裁定、binding decision を metadata.json に保存して reviewer briefing に自動注入。NotebookLM 提案: レビュアーと被レビュー側の対立を調停する裁判所的仕組み
- [x] ~~**RVW-21** (MEDIUM): per-group 独立レビュー進行~~ ✅ done (`review-json-per-group-review-2026-03-29`)
  - **対応**: `ReviewCycle` / `ReviewGroupRound` ドメインモデルを導入。per-group 独立の fast/final 進行を実現。global round 同期を廃止し、latest-success per-group 判定に移行
- [x] ~~**RVW-22** (MEDIUM): diff_base の review state 永続化~~ ✅ 修正済み (`ReviewState.base_ref` 永続化 + `check_approved` で persisted value を必須化)
- [x] ~~**RVW-23** (HIGH): `is_planning_only_path` と `review-scope.json` の SSoT 統合~~ ✅ 修正済み (`detect_planning_only` が `ReviewScopePolicy` を `track/review-scope.json` から load して planning-only/review-operational 分類を config-driven に実施。ハードコード allowlist は削除済み)
- [ ] **RVW-24** (~~HIGH~~ → LOW, legacy model — production 未使用): `update_status_after_record` の降格ロジック見直し — final round の findings_remain で `Approved` → `FastPassed` に戻し、fast round の findings_remain で `FastPassed`/`Approved` → `NotStarted` に戻す降格が過剰。1 件の finding 修正で全グループ × fast + final のフルサイクルやり直しが発生し、autorecord-stabilization トラックでは 20+ ラウンドの原因になった。review-scope hash が code freshness を保証しているため、status 降格は redundant。RVW-21 (per-group 独立進行) と組み合わせて降格ロジック自体の廃止を検討。導入: `42a667f` (2026-03-20, Phase C/D)。対象: `domain/src/review/state.rs` の `update_status_after_record`
- [ ] **RVW-25** (HIGH): domain 値オブジェクト徹底 — `CodeHash::Computed(String)` の inner を `ReviewHash` newtype に置換し、rvw1 format を型レベルで保証。`computed_unchecked` も `ReviewHash::new_unchecked` に統一。同様に他の `String` wrapper（Timestamp, TrackId 等）で validation が constructor 以外に散逸していないか監査。`sotp verify domain-purity` に「newtype inner が String のまま」のチェックを追加して CI で検出。対象: `domain/src/review/types.rs`
- [ ] **RVW-26** (MEDIUM): fast model recurrent false positive 対策 — gpt-5.4-mini が `scope.rs` の bare filename / mid-path traversal を 4 回連続で誤読。briefing にテスト名を明記しても再発。対策案: (1) fast model で同一 finding が N 回連続したら自動 skip して final に escalate、(2) finding の hash を記録し、ソース検証済み false positive を `metadata.json` に保存して briefing に自動注入（RVW-20 と統合）
- [ ] **RVW-27** (LOW): codec `computed_unchecked` のロード時 format warning — 壊れた `rvw1:sha256:` hash がサイレントにロードされる。意図的設計（legacy 互換）だが、tracing::warn で検出可能にすべきか検討
- [ ] **RVW-28** (LOW): `check_approved` の single-process 前提の明文化 — コメント追加済みだが、将来 daemon 化した場合の TOCTOU 対策が未設計。daemon 化時に advisory lock または optimistic concurrency control を導入する計画を記録
- [x] ~~**RVW-29** (CRITICAL): Codex CLI `--full-auto` が `--sandbox read-only` を上書き~~ ✅ 解決: planner/reviewer 両方の wrapper から `--full-auto` を削除。`--sandbox read-only` のみで gpt-5.4 + `--output-schema` が安定動作することを 10/10 テストで確認（2026-03-28）。原因: `--full-auto` は Codex CLI の仕様で `--sandbox workspace-write` を強制するエイリアスであり、後続の `--sandbox read-only` は無視される。exec モードではデフォルトで `approval: never`（自動承認）のため `--full-auto` は不要。
- [ ] **RVW-30** (HIGH): track-commit-message の add-all 自動実行 or check-approved の worktree/index 差分検出 — RecordRoundProtocolImpl が metadata.json をワークツリーに書くだけで git index を更新しないため、staging 漏れでコミット内容と承認状態が乖離する可能性がある。現状は /track:commit のプロンプト制約で保証しているが、コード上のガードがない。daemon 化や直接 CLI 利用が始まる前に仕組み化すべき
- [x] ~~**RVW-31** (HIGH): review state を review.json に分離 + 内部 checksum による tamper detection~~ ✅ done (`review-json-per-group-review-2026-03-29` + `autorecord-reviewjson-wiring-2026-03-30`)
  - **対応**: review state を metadata.json から review.json に完全分離。ReviewCycle モデルで per-group 管理。RecordRoundProtocolImpl が FsReviewJsonStore 経由で review.json に書き込み、check-approved が review.json から読み取り。policy_hash + partition 変更検出で staleness 検証
- [x] ~~**RVW-33** (MEDIUM): review.json cycle の frozen partition scope 接続~~ ✅ 解決: autorecord-reviewjson-wiring T005/T006 で per-group scope hash を実装。`group_scope_hash` が worktree ファイル内容を直接読み SHA-256 manifest hash を構築（git 非依存）。record-round (write) と check_approved (read) の両方が per-group scope hash を使用。policy_hash/partition_changed 検証は cycle.rs の `check_cycle_approved` / `check_cycle_staleness_any` で実装済み。T007-T009 で露呈した後続の運用問題は frozen partition scope そのものの未実装ではなく、別フォローアップ（例: WF-66）として継続管理
- [x] ~~**RVW-34** (MEDIUM): StoredFinding lossy conversion~~ ✅ done (`review-finding-fidelity-2026-04-02`)
  - **対応**: `RecordRoundProtocol::execute` に `findings: Vec<StoredFinding>` パラメータを追加。lossy 変換コードを削除し、元の message/severity/file/line を保持。usecase 層に `review_findings_to_stored()` 変換関数を追加
- [x] ~~**RVW-37** (CRITICAL): review.md グループ名 `infra` vs `infrastructure` 不一致~~ ✅ 修正済み (`rv2-docs-skill-update-2026-04-06`)
  - **対応**: `.claude/commands/track/review.md` のグループ名を `infrastructure` に統一。`track/review-scope.json` の定義と一致
- [x] ~~**RVW-38** (HIGH): `FindingDocument` / `StoredFinding` に `category` フィールドなし~~ ✅ done (`review-finding-fidelity-2026-04-02`)
  - **対応**: `StoredFinding` に `category: Option<String>` フィールドと `with_category()` ビルダーを追加。v2 `Finding` 型にも `category` フィールドあり
- [ ] **RVW-39** (HIGH): timeout/ProcessFailed 時の review.json stale 放置 — `codex_local.rs` で Timeout/ProcessFailed 時に auto-record がスキップされ review.json が更新されない。次ラウンドでコード変更があると hash mismatch で invalidation cascade。提案: (1) failed/timeout を informational round として review.json に記録、(2) stderr に stale 警告出力。出典: review-process-audit-2026-03-31
- [ ] **RVW-40** (MEDIUM): `RecordRoundProtocolImpl::execute` の巨大関数分割 — 295 行で `#[allow(clippy::too_many_lines)]`。policy resolution + diff 取得が cycle auto-create と scope hash 計算で 2 回重複。policy resolution 結果をキャッシュして共有すべき。出典: review-process-audit-2026-03-31
- [x] ~~**RVW-41** (MEDIUM): `check_approved` の `_writer: &impl TrackWriter` 未使用パラメータ削除~~ ✅ 削除済み（`review-system-v2-2026-04-05` で review usecase をリファクタリング時に解消）
- [ ] **RVW-42** (MEDIUM): corrupted partial output で session log fallback 不発 — `codex_local.rs` の session log fallback が `Missing` 状態でのみ起動し、`Invalid`（部分的に壊れた JSON）では試行されない。`Invalid` でも fallback を試行すべき。出典: review-process-audit-2026-03-31
- [ ] **RVW-43** (LOW): policy hash の `unwrap_or_default` → `expect` — `review_group_policy.rs` line 273。`serde_json::Value` の serialization は実質失敗不可能だが、将来の変更で全ポリシーが同一 hash になる silent bug の footgun。出典: review-process-audit-2026-03-31
- [ ] **RVW-35** (LOW): `normalize_track_file_for_hash` の group_scope_hash 統合 — `group_scope_hash` は生ファイル内容を SHA-256 する。metadata.json が scope に含まれる場合、`updated_at` 等の volatile fields が変更されるだけで hash が変わりうる。`tmp/transition-backup/review_scope.rs` の正規化ロジックを移植して volatile fields を除外すべき
- [ ] **RVW-36** (HIGH): Codex CLI 上限到達時のレビュー fallback — Codex 週間上限に達すると local reviewer が使えず、review.json に zero_findings を記録できないためコミットが不可能になる。claude-heavy profile の Claude reviewer で record-round を実行するパス、または check-approved の一時的バイパス機構が必要
- [x] ~~**RVW-32** (HIGH): same_round_and_zero_findings 制約の緩和~~ ✅ done (`review-json-per-group-review-2026-03-29`)
  - **対応**: global round 同期を廃止。per-group の latest-success 判定に移行し、round 番号不一致による昇格ブロックを解消。review.json 分離 (RVW-31) と統合して実装
- [ ] **RVW-44** (MEDIUM): `sotp review reset-cycle` CLI コマンド — スコープ変更（新グループ追加等）時に review.json のサイクルを安全にリセットする。現状は手動 `rm review.json` のみでバリデーションもログもない。リセットが必要なのはグループ構成が変わった場合のみ。同一グループ内の fix → re-review ではリセット不要。設計: review.json を削除ではなく `review.json.archive-{timestamp}` にリネームしてアーカイブする形にする（履歴保持）
- [ ] **RVW-45** (MEDIUM): harness-policy グループ分類不整合 — `review-scope.json` で `.claude/commands/**` は `harness-policy` に定義されているが、`--group harness-policy` の auto-record が exit 105 で失敗し `--group other` では成功した。partition と auto-record 内部の group 名照合にバグの可能性。発見: 2026-04-02 review-finding-fidelity セッション
- [ ] **RVW-46** (MEDIUM): レビューシステムのメンタルモデル文書 — cycle/group/scope/hash の関係、expected-groups の意味、スコープ変更時のリセット条件が運用文書に不足。初見の AI エージェントには難しい
- [ ] **RVW-47** (MEDIUM): `sotp review status` コマンド — 各グループの fast/final zero_findings 取得状況を一覧表示。現状は review.json 目視か check-approved のバイナリ結果しかなく、どのグループが残っているか分からない
- [ ] **RVW-48** (LOW): `sotp diff-groups` コマンド — 現在の diff を review-scope.json でグループ分類して表示。`--expected-groups` を手動で組み立てる必要がなくなる
- [ ] **RVW-49** (LOW): `sotp review scope-check` コマンド — review.json のサイクルスコープと現在の worktree diff を比較し、スコープ不整合（リセットが必要か）を事前検知
- [ ] **RVW-50** (MEDIUM): partition → record-round → review.json の結合テスト強化 — 個々のコンポーネントは正しくても境界面のバリデーションが手薄。RVW-34 T003 完了後のフォローアップ候補
- [ ] **RVW-51** (MEDIUM): auto-record verdict 反転バグ — Codex が zero_findings を返したのに review.json に findings_remain が記録されるケースを確認。RVW-34（lossy conversion）とは別問題。review flow を fail-closed 側へ誤誘導し approval/commit を不正にブロックしうるため、LOW ではなく MEDIUM 相当。verdict 変換か前ラウンド verdict の再利用、またはセッションログの誤抽出の可能性。発見: 2026-04-02 incremental-review-scope セッション
- [ ] **RVW-52** (MEDIUM): `/track:done` で approved_head 書き込み後の dirty review.json を処理 — 最後のコミット後に persist_approved_head が review.json を変更するため、worktree が dirty になり track-switch-main が失敗する。review.json を破棄するのではなく、approved_head を保持したまま clean に戻せる同期フロー（例: commit/note への取り込み、または別の永続化ポイント）が必要
- [ ] **RVW-53** (LOW): `/track:commit` に APPROVED_HEAD_FAILED 自動リカバリを追加 — track-commit-message の stdout に APPROVED_HEAD_FAILED が出たら即座に `bin/sotp review set-approved-head` を実行する指示をスキル定義に追加
- [ ] **RVW-54** (MEDIUM): CLI 統合テストハーネス構築 — make.rs の persist_approved_head、review/mod.rs の set-approved-head 等、git repo + プロセス実行を伴う CLI パスのテスト基盤が存在しない。infra 層の setup_test_repo パターンを CLI 層にも導入すべき
- [ ] **RVW-55** (MEDIUM): v1 `persist_approved_head` 残骸の削除 — `apps/cli/src/commands/make.rs` L561-566 に v1 review.json の `approved_head` 書き込みコードが残っている（コメント: "kept for backwards compat, T007 cleanup"）。v2 scope-based review.json (`scopes` フィールド) を v1 codec (`schema_version` + `cycles`) で読もうとして毎回 soft fail する。v2 `persist_commit_hash_v2` が正常動作しており v1 パスは完全にデッドコード。`persist_approved_head` 関数本体と呼び出し元、および関連する RVW-52 / RVW-53 の前提を再評価して削除すべき。発見: 2026-04-10 agent-profiles-redesign planning commit

---

## K. ドメインモデリング強化 (DM)

> 詳細は [`tmp/refactoring-plan-2026-03-19.md`](./refactoring-plan-2026-03-19.md) §0 を参照
> Phase 配置: Phase 1.5 (1.5-1〜1.5-3)

- [x] ~~**DM-01** (HIGH): `ReviewRoundResult::verdict: String` → `Verdict` enum（+ GAP-03 統合）~~ ✅ done (PR #42)
- [x] ~~**DM-02** (HIGH): `PrReviewResult::state: String` → `GhReviewState` enum~~ ✅ done (PR #42)
- [x] ~~**DM-03** (MEDIUM): `PrReviewFinding::severity: String` → `Severity` enum~~ ✅ done (PR #42)
- [x] ~~**DM-04** (MEDIUM): `ReviewRoundResult::timestamp: String` → `chrono::DateTime<Utc>`~~ ✅ done (PR #42)
- [ ] **DM-05** (MEDIUM): 散在する `chrono::Utc::now()` 直書きを `infrastructure::timestamp_now()` に統一 — `review/mod.rs`, `review_store.rs` (archive), `spec.rs`, `signals.rs` 等に残存。`infrastructure::timestamp_now()` を追加し `fs_store.rs` と `review_store.rs` (record_round) は移行済み（tddd-baseline-2026-04-11 T006）。CLI 層の残存箇所は後続トラックで実施

---

## J. CLI コマンド層の肥大化・ロジック流出 (CLI)

> 詳細は [`tmp/refactoring-plan-2026-03-19.md`](./refactoring-plan-2026-03-19.md) §1 を参照

- [ ] **CLI-01** (HIGH): `pr.rs` (1964行) の review polling/parsing を usecase 層に移動 → 目標 ~500行
- [x] ~~**CLI-02** (HIGH): `review.rs` (~2300行) の record-round/check-approved/resolve-escalation を usecase 層に移動 → 目標 ~700行~~ ✅ review-usecase-extraction-2026-03-20 + cli-review-module-split-2026-03-22 で完了。4ファイル分割、port traits を usecase に配置、hexagonal architecture convention 追加
- [ ] **CLI-03** (LOW): CLI-01/02 完了後のコーディング原則適合チェック

---

## K. WF-36 Follow-up (Review Escalation Threshold)

> **出典**: review-escalation-threshold-2026-03-19 の Codex reviewer findings (gpt-5.4)
> **追加日**: 2026-03-19

- [ ] **WF-44** (LOW): codec の phase/streak 不整合検出 — `phase=clear` だが `concern_streaks` に threshold 以上の streak がある矛盾状態を decode 時に検出・修復する
- [x] ~~**WF-45** (MEDIUM): `render_review_payload()` で `category: null` を明示出力~~ ✅ DM-01 (Verdict enum 化, PR #42) で自然消滅
- [ ] **WF-46** (LOW): codec の重複正規化後衝突検出の完遂 — collision 検出ロジック自体は done-hash-backfill Phase D で実装済み (codec.rs L392-397) だが、duplicate normalization 後衝突ケースのユニットテスト backfill が未了。回帰テスト追加まで close しない
- [ ] **WF-47** (MEDIUM): `findings_to_concerns()` を `record-round` CLI に自動配線 — 現在は `--concerns` の手動指定に依存。reviewer verdict JSON から concerns を自動抽出して渡すフローを構築する
- [ ] **WF-48** (LOW): `ReviewEscalationState::with_fields` の threshold/block バリデーション — threshold=0 や空 concerns の Blocked を domain 層で拒否する（codec 層では検証済みだが domain API 自体は無防備）
- [ ] **WF-49** (LOW): `update_escalation_after_record` の streak リセット方式 — 現在は absent concern を `retain` で削除。削除と「0 にリセット」は機能的に同等だが、`consecutive_rounds: 0` を保持して「過去に出現したが現在は連続していない」ことを表現可能にする
- [ ] **WF-50** (LOW): `file_path_to_concern` の中間ディレクトリ保持 — `apps/cli/src/commands/review.rs` → `cli.commands.review` のように中間ディレクトリを含む。設計判断として粒度が細かい方が正確だが、同一ファイルの異なるパス表現で slug が分散するリスクあり
- [x] ~~**WF-51** (LOW): `ReviewRoundResult::new`/`new_with_concerns` のバリデーション~~ ✅ DM-01 (Verdict enum 化, PR #42) で自然消滅
- [ ] **WF-52** (MEDIUM): CLI review コマンドの統合テスト — `record-round --concerns`, exit-code-3 escalation, `resolve-escalation` のパース・エラーメッセージを CLI レベルでテスト
- [ ] **WF-53** (LOW): `file_path_to_concern` の absolute path 誤マッチ — `/opt/libs/repo/apps/cli/...` のようにチェックアウト外の `libs/` にマッチするケース。rfind で最後の `libs/`/`apps/` を使うか、workspace root 相対パスに正規化してから処理すべき
- [ ] **WF-55** (HIGH): metadata.json SSoT 一貫化 — 全 track `.md` ファイルを metadata.json からの read-only view に統一。view は git tracked のまま維持し、`sotp verify view-freshness` で metadata.json ↔ view の一貫性を CI で保証する
  - **設計方針**: B案（構造化フィールド + 散文は `Vec<String>` で格納）。plan.md の `summary` / `sections[].description` で実証済みのパターンを全ファイルに適用
  - **view の git 管理方針**: tracked のまま維持（untrack しない）。理由: PR diff で view の変更が見える、CI artifact 生成の運用コスト不要、レビュー体験を損なわない。手動編集は `sotp verify view-freshness` で CI 検出・拒否
  - **Phase 1**: `sotp verify view-freshness` + view-freshness CI ゲート
    - `sotp verify view-freshness` を実装 — metadata.json から view を再生成し、既存 view と差分があれば CI fail
    - plan.md + registry.md で先行導入
    - `/track:plan` スキル定義に spec JSON テンプレートを組み込み（AI は穴埋めするだけ、編集コスト対策）
  - **Phase 2**: verification.md を metadata.json に統合 + view 化
    - Scope Verified セクション: plan.md のタスクチェックボックスと重複 → 削除
    - Manual Verification Steps: `verification.steps[]` フィールド（`{ id, description, auto: bool }`）に構造化
    - Result / verified_at: metadata.json のトップレベルフィールド
    - view-freshness CI ゲートを verification.md にも拡張
    - `schema_version: 4` で新フォーマットを区別。codec の `decode` に後方互換パス（`verification` フィールドが無い場合は verification.md を別途読む）を残し、既存トラックはマイグレーション不要
  - **Phase 3**: spec.md を metadata.json に統合 + view 化
    - `spec.goal: string`, `spec.scope: [{ phase, title, items[] }]`, `spec.out_of_scope: string[]`, `spec.constraints: string[]`
    - `spec.acceptance_criteria: [{ id, description, phase, task_refs[] }]` — 要件トレーサビリティの構造的保証
    - `spec.domain_states: [{ id, name, signal, description }]` — 信号機 (🔵🟡🔴) の機械検証基盤（`sotp verify spec-signals` で CI ゲート可能）
    - 散文は `description: Vec<String>` で格納（Markdown 段落・箇条書きをそのまま保持）
    - Mermaid 図は spec.md の責務外（plan.md / DESIGN.md の Canonical Blocks）
    - `sotp verify metadata-schema` で spec/plan 境界を強制（spec に tasks/sections が入っていたら reject、plan に goal/acceptance_criteria が入っていたら reject）。domain 層の型定義が境界を強制するため、codec でコンパイルエラー = 物理的に混同不可能
    - spec/plan 内容重複防止ルールを `project-docs/conventions/` に convention として文書化（spec.scope = 「何を」要件レベル、plan.sections = 「どうやって」実装レベル、spec.acceptance_criteria.task_refs が両者をリンク）
  - **既存トラック互換**: 新規トラックのみ新フォーマット適用。完了/アーカイブ済みトラックはマイグレーション不要。`schema_version` で新旧を区別し、codec 内部で後方互換を吸収
  - **ビジョンとの整合**: 探索的精緻化ループが SSoT 上で閉じる。信号機の `signal` フィールドが Phase 2 (TODO-PLAN 2-1 TSUMIKI-01) の基盤になる
  - **実装タイミング**: CLI-02 完了後に metadata.json codec を触るタイミングで Phase 2-3 を実装するのが効率的

---

## L. Review System v2 Follow-up

> **出典**: review-system-v2-2026-04-05 セッション 3 のレビューサイクルで特定
> **追加日**: 2026-04-05

- [x] ~~**RV2-01** (HIGH): `is_planning_only_path` のハードコーディング廃止~~ ✅ planning_only 概念を完全廃止（Session 4 で削除）。v2 は全ファイルを review-scope.json で分類
- [x] ~~**RV2-02** (MEDIUM): サブエージェントによる修正+レビューの自律ループ~~ ✅ done (`rv2-docs-skill-update-2026-04-06` T006)
  - **対応**: review-fix-lead エージェント定義を新規作成。1 scope 所有で fix→verify→re-review→zero_findings まで自律。cross-scope は blocked_cross_scope で fail-closed
- [ ] **RV2-03** (LOW): `ReviewState::Running` の追加 — 並列レビュー時に「レビュー実行中」を区別し、重複起動防止と進捗可視化を可能にする。`ReviewState::Running { started_at: Timestamp }` を enum variant として追加
- [x] ~~**RV2-04** (HIGH): `/track:review` SKILL の v2 対応~~ ✅ done (`rv2-docs-skill-update-2026-04-06` T002)
  - **対応**: review.md の v1 group-based 手順を v2 scope-based に更新。provider-support サブセクション集約、fail-closed 契約追加、NotStarted bypass 仕様追加

> **出典**: review-system-v2-2026-04-05 セッション 5 のレビューサイクルで特定
> **追加日**: 2026-04-06

### 未修復の穴（Accepted Deviations として残存）

- [ ] **RV2-05** (HIGH): プロセスグループ kill 不可 — `codex_reviewer.rs` の `terminate_reviewer_child` は `child.kill()` のみ。`killpg` は `#[forbid(unsafe_code)]` で使えない。timeout 後に孫プロセスが残留し、繰り返すとホストリソース消費。CLI 層での process group kill wrapper を実装する
- [ ] **RV2-06** (HIGH): v2 escalation 完全切断 — `codex_local.rs` の `write_verdict` は `record_round` / concern 追跡 / metadata.json escalation state を一切更新しない。同じ finding が無限ループ可能。v2 terms で escalation を再設計する
- [ ] **RV2-07** (MEDIUM): v1 domain コード残存 — `track/codec.rs` が v1 review 型（`ReviewCycle`, `CycleGroupState`, `ReviewState` v1, escalation）に依存し削除不可。track codec のリファクタリングが必要
- [ ] **RV2-08** (MEDIUM): v2 CLI パスのテスト不在 — `execute_codex_local` → `ReviewCycle` → `write_verdict` のフルパスが未テスト。サブコンポーネントは独立テスト済みだが integration test がない
- [ ] **RV2-09** (LOW): `main` ハードコード — `compose_v2.rs` の `resolve_diff_base` が `.commit_hash` 不在時に `git rev-parse main` にフォールバック。default branch が `main` でないリポジトリで動作しない

- [ ] **RV2-16** (HIGH): 計画レビュー専用コマンド — ADR `knowledge/adr/2026-04-09-2047-planning-review-phase-separation.md` で設計済み（agent-profiles 再設計は track `agent-profiles-redesign-2026-04-10` で完了済み）
  - **根本原因**: 計画レビュー時に `metadata.json` / `spec.json` (SoT) と `plan.md` / `spec.md` / `verification.md` (rendered view) の両方をレビューアが読むため、同じ情報の表現齟齬を延々と指摘し続ける無限ループに陥る (v2-escalation-redesign-2026-04-09 の planning review で実測: 30+ ラウンドでも収束せず)
  - **解決策**: (1) 新コマンド `sotp review plan` / `sotp commit plan` を追加し既存 review システムと完全独立 (2) 必要度分類 (Necessity: must/advisory/info) 付き verdict で `must_count == 0` ゲート (3) `track/planning-artifacts.json` に allowlist 集約 (4) `plan-review.json` を新設 (既存 review.json v2 schema 踏襲 + necessity 追加)
  - **prerequisite**: ~~agent-profiles.json 再設計~~ ✅ 完了 (track `agent-profiles-redesign-2026-04-10`、ADR `2026-04-09-2235` Accepted)
  - **追加日**: 2026-04-09

- [ ] **RV2-17** (MEDIUM): Python hook 全廃止 — `.claude/hooks/*.py` を全削除し、fail-closed 系は Rust `sotp hook dispatch` に統一、advisory 系は各 track command の skill / コマンドドキュメントに吸収する
  - **対象**: `check-codex-before-write.py`, `check-codex-after-plan.py`, `post-implementation-review.py`, `post-test-analysis.py`, `suggest-gemini-research.py`, `lint-on-save.py`, `python-lint-on-save.py`, `log-cli-tools.py`, `_agent_profiles.py`, `_shared.py`, および関連テスト
  - **既に Rust 化済み**: `skill-compliance`, `block-direct-git-ops`, `block-test-file-deletion` (`sotp hook dispatch` 経由)
  - **スコープ**: (a) 各 Python hook が提示していた advisory message を track command skill に移植、(b) .claude/settings.json から Python hook エントリを削除、(c) `.claude/hooks/` ディレクトリを削除、(d) CI の `hooks-selftest` タスクを削除または Rust hook test に置き換え
  - **実装順序**: 本 RV2-17 が **Phase 1** で、agent-profiles redesign (ADR `2026-04-09-2235`) と RV2-16 (ADR `2026-04-09-2047`) の **prerequisite**。Python hook を先に削除することで、後続 ADR の Python 側参照更新が不要になり、スコープが縮小する
  - **追加日**: 2026-04-09

### 運用文書の穴

- [x] ~~**RV2-10** (MEDIUM): pr-review.md — Codex Cloud 同一コミット再レビュー不可の注記~~ ✅ done → **修正** (`bridge01-export-schema-2026-04-06`)
  - **確認**: 実測で同一 HEAD でも `@codex review` 再ポストで新規レビューされることを確認 (PR #81, 2026-04-06)。pr-review.md を「再レビュー可」に修正済み
- [x] ~~**RV2-11** (MEDIUM): pr-review.md — 手動ポーリング禁止の明記~~ ✅ done (`rv2-docs-skill-update-2026-04-06` T003)
  - **確認**: pr-review.md に "No manual polling" セクション追加済み
- [x] ~~**RV2-12** (MEDIUM): review.md — fail-closed 契約のチャネル単位明記~~ ✅ done (`rv2-docs-skill-update-2026-04-06` T002)
  - **確認**: review.md Step 2d に Channel-scoped fail-closed contract テーブル追加済み
- [x] ~~**RV2-13** (MEDIUM): review.md / track/workflow.md — NotStarted bypass 仕様記載~~ ✅ done (`rv2-docs-skill-update-2026-04-06` T002/T004)
  - **確認**: review.md Step 4 に NotStarted bypass 条件を記載。track/workflow.md に v1 `record-round` 参照は残存なし
- [x] ~~**RV2-14** (LOW): knowledge/conventions/ — create_dir_all ガード無効化パターン convention 化~~ ✅ done (`rv2-docs-skill-update-2026-04-06` T005)
  - **確認**: `knowledge/conventions/filesystem-persistence-guard.md` に convention 追加済み
- [x] ~~**RV2-15** (LOW): track/workflow.md — v2 運用手順更新 + v1 残存監査~~ ✅ done (`rv2-docs-skill-update-2026-04-06` T004)
  - **確認**: track/workflow.md に v1 `record-round` 参照なし (grep 0 件)

- [ ] **RV2-18** (MEDIUM): `sotp review codex-local` の verdict/findings stdout 出力改善 — 現在は Codex CLI の stdout 末尾に verdict JSON が埋もれており、review-fix-lead agent が `tail` で抽出している。`sotp review codex-local` が auto-record 完了後に findings のサマリーを整形して stdout に出力すれば、agent も orchestrator も exit code + stdout だけで判断完結する。review.json の Read が不要になり、レビューループの手間が減る
  - **追加日**: 2026-04-10
