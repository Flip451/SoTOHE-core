<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# CLI-02: review.rs usecase extraction + domain/usecase/infrastructure module split

CLI-02: apps/cli/src/commands/review.rs (2317行) から domain/infrastructure オーケストレーションロジックを抽出し、domain 厚く / usecase 薄く / CLI 薄くの原則で全層のモジュール分割を実施。全ファイル 700行以下を目標。

## Phase 1: Infrastructure 移動

T001: PrivateIndex (~319行) + resolve_real_index_path (~18行) を infrastructure::git_cli::private_index.rs に移動
git_cli.rs をモジュールディレクトリ化（git_cli/mod.rs + private_index.rs）

- [ ] PrivateIndex + resolve_real_index_path を infrastructure::git_cli::private_index に移動（git_cli モジュールディレクトリ化）

## Phase 2: Domain モジュール分割 + 純粋ロジック集約

T002: domain::review.rs (2375行) → review/{mod,error,concern,escalation,types,state}.rs に分割
純粋ロジック追加: classify_review_verdict, resolve_full_auto, ModelProfile, file_path_to_concern, extract_verdict_from_content
デッドコード削除: ReviewGroupState::from_legacy_final_only（呼び出し箇所ゼロ、done-hash-backfill Phase D 残件）

- [ ] domain::review.rs をモジュールディレクトリ化（review/{mod,error,concern,escalation,types,state}.rs）+ 純粋ロジック追加（classify_review_verdict, resolve_full_auto, ModelProfile, file_path_to_concern, extract_verdict_from_content）+ from_legacy_final_only 削除（デッドコード）

## Phase 3: UseCase モジュール分割 + 薄い UseCase 構築

T003: usecase::review_workflow.rs (936行) → review_workflow/{mod,verdict,concern,usecases}.rs に分割
UseCase 新設: RecordRoundUseCase, ResolveEscalationUseCase, CheckCommitReadyUseCase (薄い Load→domain→Save)
agent-profiles.json の I/O（AgentProfiles/ProviderConfig serde 型 + ファイル読み込み）を infrastructure に分離
extract_verdict_from_session_log を usecase に移動（file read + domain::extract_verdict_from_content の薄いオーケストレーター）

- [ ] usecase::review_workflow.rs をモジュールディレクトリ化（review_workflow/{mod,verdict,concern,usecases}.rs）+ 薄い UseCase 構築 + agent-profiles.json I/O を infrastructure に分離（AgentProfiles/ProviderConfig serde 型）+ extract_verdict_from_session_log を usecase に移動（file read + domain::extract_verdict_from_content 呼び出しの薄いオーケストレーター）

## Phase 4: CLI 圧縮

T004: run_record_round (~242行) → RecordRoundUseCase 呼び出し (~30行) に圧縮。CLI の codex-local は usecase::extract_verdict_from_session_log を呼び出す（domain/infra 直接参照なし）
T005: run_resolve_escalation (~117行) → ResolveEscalationUseCase 呼び出し (~20行) に圧縮
T006: run_check_approved (~76行) → CheckCommitReadyUseCase 呼び出し (~15行) に圧縮

- [ ] CLI run_record_round を RecordRoundUseCase 呼び出しに圧縮
- [ ] CLI run_resolve_escalation を ResolveEscalationUseCase 呼び出しに圧縮
- [ ] CLI run_check_approved を CheckCommitReadyUseCase 呼び出しに圧縮

## Phase 5: テスト整合性 + CI

T007: domain/usecase テスト新設 + CLI テスト調整 + cargo make ci 全通し

- [ ] テスト移動・調整（domain/usecase テスト新設 + CLI テスト調整）+ CI 全通し
