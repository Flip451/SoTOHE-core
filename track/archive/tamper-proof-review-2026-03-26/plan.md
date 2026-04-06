<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# レビュー verdict 改ざん防止 — provenance + write-guard + CLI 封鎖

レビュー verdict の改ざん防止を 4 層で実現する:
1. Domain: VerdictProvenance ADT で各ラウンドの出自を型レベルで保証
2. CLI 封鎖: record-round サブコマンド削除、codex-local --auto-record のみが verdict 記録可能
3. Hook ガード: review.json + review-artifacts/ への Write/Edit をブロック
4. Verification: check-approved --require-provenance で SHA-256 ダイジェスト整合性検証

## Domain Provenance Model

VerdictProvenance (LegacyUnverified | TrustedSubprocess) を ReviewRoundResult に追加。
SHA-256 ダイジェスト用 newtype 群、ReviewInvocationId、ProtectedArtifactRef を定義。
既存 review.json データは LegacyUnverified としてデコード。

- [-] Domain: VerdictProvenance ADT + Sha256Hex + ReviewInvocationId + ProtectedArtifactRef + SessionDigest/PayloadDigest/AttestationDigest newtypes
- [-] Domain: ReviewRoundResult に provenance フィールド追加 + コンストラクタ統一
- [-] Domain: ReviewError に provenance 関連バリアント追加 (InvalidProvenance, MissingEvidence, EvidenceDigestMismatch 等)

## Infrastructure Schema & Persistence

review.json schema_version 2 — ラウンドドキュメントに provenance フィールド追加。
review-artifacts/<invocation-id>/ にセッションログ、final-message、attestation を永続化。
ReviewAttestationDocument serde 型を定義。

- [-] Infrastructure: review.json schema_version 2 — ReviewRoundDocument に Optional<provenance> + ReviewRoundProvenanceDocument + ReviewArtifactRefDocument serde 型
- [-] Infrastructure: ReviewAttestationDocument serde 型 + review-artifacts/ ディレクトリ管理 (persist/load)

## Attested Recording Pipeline

AttestedReviewRound 型で verdict + provenance + evidence バイトを一体化。
RecordRoundProtocol::execute_attested() で review.json + artifacts をアトミック永続化。
codex-local --auto-record 内で provenance 生成（hash + attestation）を統合。

- [-] Usecase: AttestedReviewRound 型 + RecordRoundProtocol::execute_attested() ポート + record_attested_round() usecase 関数
- [-] Infrastructure: RecordRoundProtocolImpl::execute_attested() — アーティファクト永続化 + review.json + git add をアトミック実行
- [-] CLI: codex-local --auto-record に provenance 生成統合 — session log hash + payload hash + attestation 生成 → AttestedReviewRound 構築

## CLI Sealing & Verification

record-round サブコマンドを削除し、手動 verdict 注入を構造的に不可能にする。
ReviewEvidenceVerifier ポートで check-approved に provenance 検証ゲートを追加。
FsReviewEvidenceVerifier でファイル読込 + SHA-256 ダイジェスト照合を実装。

- [-] CLI: RecordRound サブコマンド削除 — ReviewCommand enum から除去
- [-] Usecase: ReviewEvidenceVerifier ポート + check_approved に require_provenance ゲート追加
- [-] Infrastructure: FsReviewEvidenceVerifier — アーティファクトファイル読込 + SHA-256 検証 + attestation 整合性チェック

## Write Guard & CI Enforcement

BlockProtectedReviewStateWrite hook で review.json + review-artifacts/ への直接書込みをブロック。
settings.json に Write|Edit matcher hook 追加。
CI で --require-provenance を soft enforcement として導入。

- [-] Domain/Hook: BlockProtectedReviewStateWrite hook handler — Write/Edit で review.json + review-artifacts/ をブロック
- [-] CLI/Settings: settings.json に Write|Edit matcher hook 追加 + verify-orchestra で hook 存在を検証
- [-] CI: check-approved に --require-provenance フラグ追加 + cargo make ci での soft enforcement
- [-] CI: --require-provenance を cargo make ci のデフォルトに昇格（hard enforcement — LegacyUnverified ラウンドを拒否）
