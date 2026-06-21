# Observations — existing-dry-violation-cleanup-2026-06-19

## T013: post-remediation DRY census 検証 (AC-07)

remediation 完了後、本 track が対象とする in-scope クラスタの重複が解消したことを
ソース直接検証で確認した。`bin/sotp dry check-approved` は全コミットで APPROVED。

**重要 — gate の評価範囲**: `sotp dry` は **コミット diff の changed fragment** のみを判定する
(whole-codebase 全走査ではない)。後述の deferred test-helper コピーは本 track で一切 touch していない
ため diff に現れず、judge 対象にならない。よって gate は APPROVED である。これは矛盾ではなく、
「consolidation のために当該コピーを touch すると changed fragment となり gate が violation 判定する
(T008 で実証し revert 済み) → だから保留した」という後述の説明と整合する。両者は「diff に現れるか
否か」で区別される。

### 解消確認 (in-scope クラスタ)

| クラスタ | 解消前 | 解消後 | 検証 |
|---|---|---|---|
| **D1** track-ID slug 検証 | usecase 3 + CLI 3 の独立再実装 (`previous_was_hyphen` ループ) | domain `is_valid_track_id`/`TrackId::try_new` 単一正典 + 委譲のみ | `previous_was_hyphen` は `libs/domain/src/ids.rs` のみに存在 |
| **D2** 空/空白禁止不変条件 | domain 8+ 箇所の inline `trim().is_empty()` | `NonEmptyString::try_new` + `validate_trimmed_non_empty` 正典へ委譲 | domain constructor の inline guard = 0、正典 helper 存在 |
| **D3** Codex subprocess / SHA-256 hex | codex_reviewer / codex_dry_checker の重複 helper、inline hex | `codex_common` 単一 helper + `corpus::sha256_hex` 委譲 | spawn_codex/drain_pipe/tee は codex_common 単一、production inline hex = 0 |
| **D4-constants** | POLL_INTERVAL 5 コピー、"tmp/reviewer-runtime" infra 4 箇所 | 層境界ごと単一 const (POLL_INTERVAL 1 infra + 1 cli、REVIEW_RUNTIME_DIR infra 単一) | const 定義数が層ごと 1 |

### 保持 (意図的、未解消違反に数えない)

- **D3 retain (ADR D3)**: exclusive-lock 取得パターン、4-source git-diff union — port 固有の
  parallel structure として保持。
- **D4 cross-layer coincidental constants (AC-05)**: apps/cli と infrastructure に跨る同値の
  POLL_INTERVAL / "tmp/reviewer-runtime" は layer 境界 (cli ↛ infrastructure) に起因する
  偶然の一致定数として保持。
- **apps/cli planner の codex subprocess helpers**: plan/codex_local.rs の spawn_codex 等は
  apps/cli 層 (↛ infrastructure) の並行実装で D3 スコープ外。tee_stderr_to_file のみ
  cli_composition 経由で infra 正典へ委譲済み (T006)。

### 保留 (OS-06 / ADR D4 note — T008-T010 deferred)

D4 test-helper サブクラスタ (CwdGuard/CurrentDirGuard、init_git_repo、usecase stub bindings) は
本 track から保留。census 上はこれらの境界ごと/crate 跨ぎコピーが残存する (期待通り):
- CwdGuard/CurrentDirGuard: 5 コピー (infra 2, cli 2, cli-composition 1)
- init_git_repo / init_git_repo_on_track_branch: 9 定義
- stub_binding / StubLayerBindings / NoLayersBindings: 8 定義

**保留理由 (機械検証不可の観測)**: DRY gate (embedding 類似度 + Codex judge) は diff の changed
fragment を判定し、fragment 抽出は fn/impl のみを対象とする。ADR D4 が意図的に保持する
「テストコンパイル境界ごと 1 定義」を実際に consolidation しようとすると、境界をまたぐ
byte-identical な RAII guard (struct + `impl Drop`) や helper fn が changed fragment となり、
gate はこれを violation 判定する (T008 で `CurrentDirGuard` の cross-crate ペアが score 0.997 で
flag され実証、その後 revert)。しかし gate には architecture-rules.json / ADR コンテキストを judge に
注入する手段も、accept-list で受容済みと記録する手段も無い (pair の skip は config_fingerprint 一致時のみ、
judge が violation を返すと通す術がない)。ゲートを弱める・禁止された cross-crate dev-visible test API を
作る・gating を回避する、のいずれも避けるため test-helper consolidation を保留した。一方、コピーを
touch しなければ diff に現れず gate は APPROVED のまま (現状)。また const/data 重複は fragment 抽出
対象外 (fn/impl のみ抽出) のため、D4-constants (T011/T012) は consolidation しても gate に阻まれず完遂できた。

→ Reassess: gate が層認識型 judge コンテキスト or accept-list を獲得した時点で再着手 (ADR D4 Reassess When)。

### コミット (12)

baseline (ADR + census evidence) → plan (Phase 1-3) → T001-T007 (D1/D2/D3) →
D4 test-helper 保留 (scope 縮小) → T011/T012 (D4-constants)。全コミットで
`cargo make ci` green + DRY APPROVED + 全 review scope zero_findings。
