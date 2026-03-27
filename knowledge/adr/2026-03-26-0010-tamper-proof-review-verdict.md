# レビュー verdict 改ざん防止設計

## ステータス

承認済み（実装は review hash スコープ再設計完了後に開始）

## 背景

Claude Code（オーケストレーター）はレビューワークフローを管理する非信頼エージェントである。
現行設計には以下の改ざんベクトルが存在する：

1. **verdict 注入**: `sotp review record-round --verdict '{"verdict":"zero_findings",...}'` に偽の verdict を渡せる
2. **直接ファイル改ざん**: Write/Edit ツールで `review.json` の `status` を `approved` に書き換えられる
3. **auto-record 迂回**: `codex-local --auto-record`（内部アトミック）を使わず、手動 `record-round` で偽 verdict を記録できる

### 信頼境界

- **非信頼**: Claude Code（引数選択、ファイル書き込み、コマンド実行タイミング）
- **信頼**: Rust CLI `sotp`（guard hook で上書き保護、サブプロセス内で verdict 抽出 → 記録をアトミック実行）

## 決定

4 層の防御で verdict 改ざんを構造的に不可能にする：

### 1. CLI 封鎖

`sotp review record-round` サブコマンドを `ReviewCommand` enum から削除。
verdict 記録は `codex-local --auto-record` 内部（sotp がサブプロセスの実行 → verdict 抽出 → 記録を一体実行）のみ可能にする。

### 2. Hook ガード（多層防御）

`BlockProtectedReviewStateWrite` PreToolUse hook を追加。
`Write` / `Edit` ツールで以下のパスへの書き込みをブロック：
- `track/items/*/review.json`
- `track/items/*/review-artifacts/**`

Bash 経由の書き込みは既存の bash-write-guard（`block-direct-git-ops` hook 内の output redirect 検知 + `permissions.deny` による `cp`/`mv`/`tee` 等のブロック）で防御する。
ただし `bash -c` payload の再帰パース不足（SEC-14）は既知の残留リスクとして TODO に記録済み。
完全な Bash 経路封鎖は SEC-14 の解決後に達成される。

防御の階層:
1. `Write`/`Edit` → `BlockProtectedReviewStateWrite` hook でブロック
2. Bash output redirect (`>`, `>>`, `tee`) → bash-write-guard でブロック
3. Bash `cp`/`mv`/`install` → `permissions.deny` でブロック
4. `bash -c` payload → SEC-14 解決後にブロック（残留リスク）
5. 最終防御: `check-approved --require-provenance` が artifact digest を検証し、改ざんされた review.json を拒否

### 3. Provenance（出自証明）

各レビューラウンドに `VerdictProvenance` を付与：
- `LegacyUnverified`: 旧データ（provenance なし）
- `TrustedSubprocess`: sotp 内部で生成された verdict。セッションログ・final payload・attestation の SHA-256 ダイジェストを保持

アーティファクトは `track/items/<id>/review-artifacts/<invocation-id>/` に永続化。

### 4. Verification（検証ゲート）

`check-approved --require-provenance` フラグを追加。
commit 前にアーティファクトの存在 + ダイジェスト整合性 + verdict 一致を検証。

### 移行パス

- Phase 1: `review.json` schema_version 2 — provenance は optional（旧データは `LegacyUnverified`）
- Phase 2: `record-round` CLI 削除、全フローを `--auto-record` に統一
- Phase 3: `check-approved` で provenance 検証（lenient モード）
- Phase 4: CI で `--require-provenance` を強制

### RecordRoundProtocol の互換性

`execute_attested()` を既存の `execute()` と共存させる additive API として導入。
Phase 2 で `execute()` を削除し `execute_attested()` のみにする。

## Canonical Blocks

```rust
// domain/src/review/provenance.rs

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Sha256Hex(String);

impl Sha256Hex {
    pub fn new(value: impl Into<String>) -> Result<Self, ReviewError> {
        let value = value.into();
        let is_hex = value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit());
        if !is_hex {
            return Err(ReviewError::InvalidProvenance(
                "sha256 digest must be 64 hex chars".to_owned(),
            ));
        }
        Ok(Self(value.to_lowercase()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReviewInvocationId(NonEmptyString);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDigest(Sha256Hex);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadDigest(Sha256Hex);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttestationDigest(Sha256Hex);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectedArtifactRef {
    path: NonEmptyString,
    bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum ReviewerKind {
    CodexLocal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum VerdictSource {
    OutputLastMessage,
    SessionLogFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedSubprocessProvenance {
    invocation_id: ReviewInvocationId,
    reviewer: ReviewerKind,
    captured_at: Timestamp,
    source: VerdictSource,
    session_log: ProtectedArtifactRef,
    session_digest: SessionDigest,
    final_message: ProtectedArtifactRef,
    payload_digest: PayloadDigest,
    attestation: ProtectedArtifactRef,
    attestation_digest: AttestationDigest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerdictProvenance {
    LegacyUnverified,
    TrustedSubprocess(TrustedSubprocessProvenance),
}
```

```rust
// usecase — attested recording

pub struct AttestedReviewRound {
    pub round_type: RoundType,
    pub group_name: ReviewGroupName,
    pub expected_groups: Vec<ReviewGroupName>,
    pub verdict: Verdict,
    pub concerns: Vec<ReviewConcern>,
    pub timestamp: Timestamp,
    pub provenance: VerdictProvenance,
    pub final_message_json: Vec<u8>,
    pub session_log_bytes: Vec<u8>,
    pub attestation_json: Vec<u8>,
}

pub trait RecordRoundProtocol {
    /// Existing entrypoint — retained for backward compatibility during migration.
    fn execute(
        &self,
        track_id: &TrackId,
        round_type: RoundType,
        group_name: ReviewGroupName,
        verdict: Verdict,
        concerns: Vec<ReviewConcern>,
        expected_groups: Vec<ReviewGroupName>,
        timestamp: Timestamp,
    ) -> Result<(), RecordRoundProtocolError>;

    /// Attested entrypoint — becomes sole entrypoint after migration Phase 2.
    fn execute_attested(
        &self,
        track_id: &TrackId,
        round: AttestedReviewRound,
    ) -> Result<(), RecordRoundProtocolError>;
}
```

```rust
// usecase — evidence verification

pub trait ReviewEvidenceVerifier {
    fn verify_round(
        &self,
        track_id: &TrackId,
        group: &ReviewGroupName,
        round_type: RoundType,
        round: &ReviewRoundResult,
    ) -> Result<ReviewEvidenceStatus, String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewEvidenceStatus {
    Verified,
    LegacyUnverified,
    MissingArtifact { path: String },
    DigestMismatch { path: String, expected: String, actual: String },
    VerdictMismatch,
}
```

```rust
// infrastructure — serde documents

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReviewRoundProvenanceDocument {
    LegacyUnverified,
    TrustedSubprocess {
        invocation_id: String,
        reviewer: String,
        captured_at: String,
        source: String,
        session_log: ReviewArtifactRefDocument,
        final_message: ReviewArtifactRefDocument,
        attestation: ReviewArtifactRefDocument,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReviewAttestationDocument {
    pub schema_version: u32,
    pub invocation_id: String,
    pub reviewer: String,
    pub model: String,
    pub track_id: String,
    pub round_type: String,
    pub group: String,
    pub expected_groups: Vec<String>,
    pub captured_at: String,
    pub source: String,
    pub review_hash: String,
    pub final_payload_sha256: String,
    pub session_log_sha256: String,
}
```

```rust
// domain/src/hook/types.rs — new hook variant

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookName {
    BlockDirectGitOps,
    BlockTestFileDeletion,
    BlockProtectedReviewStateWrite,
}
```

```text
// artifact layout

track/items/<id>/
  review.json
  review-artifacts/
    <invocation-id>/
      final-message.json
      session.log
      attestation.json
```

```rust
// CLI — sealed (RecordRound removed)

#[derive(Debug, clap::Subcommand)]
pub enum ReviewCommand {
    CodexLocal(CodexLocalArgs),
    CheckApproved(CheckApprovedArgs),
    ResolveEscalation(ResolveEscalationArgs),
    Status(StatusArgs),
}
```

## 影響

- `ReviewRoundResult` に `provenance: VerdictProvenance` フィールドが追加される
- `ReviewError` に provenance 関連バリアント（`InvalidProvenance`, `MissingEvidence`, `EvidenceDigestMismatch` 等）が追加される
- `review.json` schema_version が 2 に上がり、ラウンドに optional provenance ドキュメントが追加される
- `check-approved` の呼び出し元は `--require-provenance` フラグを選択的に有効化できる
- 旧データは `LegacyUnverified` としてデコードされ、`--require-provenance` が false の場合は許可される

## 前提条件

- ADR-2026-03-26-0000（review hash スコープ再設計）の完了が必須。auto-record の安定性がこの設計の前提

## 追加決定: record-round 削除の検証

Codex planner (gpt-5.4) による検証結果:

- **完全削除が正しい**: sotp がサブプロセス実行 → verdict 抽出 → 永続化をアトミック実行する信頼モデルにおいて、呼び出し元提供の verdict を受け付ける公開コマンドは主要な注入ベクトルを再開する
- **escape hatch は不要**: 「sealed manual verdict」は安全でないか、内部 auto-record パスの名前変更にすぎない
- **デバッグ/修復用途**: verdict 偽装ではなく状態修復コマンド（invalidate / reset / purge）で対応。これらは破損したレビューアーティファクトの修復であり、verdict の捏造ではない
- **トレードオフ**: 修復コマンドが実装されるまでインシデント復旧の利便性は低下するが、許容範囲

## 出典

Codex planner (gpt-5.4) により設計、2026-03-26。
Codex planner (gpt-5.4) により record-round 削除の検証完了、2026-03-27。
