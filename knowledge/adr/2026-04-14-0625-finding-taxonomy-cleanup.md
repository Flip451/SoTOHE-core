---
adr_id: 2026-04-14-0625-finding-taxonomy-cleanup
decisions:
  - id: 2026-04-14-0625-finding-taxonomy-cleanup_grandfathered
    status: accepted
    grandfathered: true
---
# Finding 型 Taxonomy クリーンアップ — 同名衝突の解消と hexagonal 分離の維持

## Status

Accepted (track `tddd-04-finding-taxonomy-cleanup-2026-04-14` で実装予定)

関連 ADR:

- Parent: `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` — Phase 1 Completion Amendment §3.B で本 ADR への deferral を宣言し、§3.B Resolution サブセクションから本 ADR を参照している。
- Sibling: `knowledge/adr/2026-04-13-1813-tddd-taxonomy-expansion.md` — `TraitPort` → `SecondaryPort` の cascade rename の前例。本 ADR は同じポリシーを踏襲する。

## Context

SoTOHE-core には現在、レイヤーをまたいで **4 つの distinct な `Finding` family 型** が存在する:

| # | 型 | 配置 | 役割 |
|---|---|---|---|
| 1 | `domain::review_v2::Finding` | `libs/domain/src/review_v2/types.rs:210` | validated な domain newtype。reviewer 出力の finding を表し、constructor で `message` の非空を強制する。field は private。 |
| 2 | `domain::verify::Finding` | `libs/domain/src/verify.rs:28` | `sotp verify` サブコマンドが生成する構造化エラー/警告。`Severity` enum を持つ。 |
| 3 | `usecase::review_workflow::ReviewFinding` | `libs/usecase/src/review_workflow/verdict.rs:102` | reviewer JSON wire format 用の serde DTO (`#[serde(deny_unknown_fields)]`)。 |
| 4 | `usecase::pr_review::PrReviewFinding` | `libs/usecase/src/pr_review.rs:70` | Codex Cloud の PR review 結果パース型。severity が文字列 (`"P0"/"P1"/…`) の別形状。 |

このうち (1) と (2) は domain crate 内で last-segment 名 `Finding` を共有している。`libs/infrastructure/src/code_profile_builder.rs:54` は domain rustdoc JSON を元に last-segment 名をキーとする `HashMap<String, TypeNode>` を構築するため、2 つの型が非決定的に衝突し、片方のエントリが片方を上書きして `warning: same-name type collision for Finding` が `build_type_graph` 実行ごとに stderr へ出力される。

TDDD-01 (`tddd-01-multilayer-2026-04-12`) はこの下流ノイズを抑えるため、`domain-types.json` の `type_definitions` 配列の 4 番目に suppression 用の `"reference"` entry を置いた:

```json
{
  "name": "Finding",
  "description": "Reference entry for the pre-existing same-name type collision between domain::verify::Finding and domain::review_v2::types::Finding. The collision is documented as a known issue; this reference declaration suppresses the baseline_changed_type Red signal caused by non-deterministic HashMap ordering at TypeGraph build time",
  "approved": true,
  "action": "reference",
  "kind": "value_object"
}
```

このエントリは Red シグナルこそ黙らせるが、根本的な衝突は解消していない。`description` にも "documented as a known issue" とある通り、これは恒久的な設計判断ではなく technical debt である。

衝突は具体的に 4 つの悪影響を持つ:

1. **baseline capture のたびに stderr が汚染される**: `sotp track baseline-capture <track-id> --layer domain` の各実行で collision warning が出続ける。
2. **TypeGraph が非決定的**: rustdoc JSON の順序次第で `review_v2::Finding` か `verify::Finding` のどちらかが `TypeGraph` から落ち、TDDD signal evaluator は落ちた方について推論できない。
3. **catalogue 参照が曖昧**: TDDD L1 signals は last-segment 名で `expected_methods` / `kind_tag` を解決するため、catalogue に `Finding` と書かれていても 2 つのうちどちらを指しているか判別できない。
4. **suppression entry 自体が負債**: `domain-types.json` の `"Finding"` reference entry には意味的な価値がなく、HashMap 非決定性由来の `baseline_changed_type` Red シグナルを黙らせるためだけに存在している。

TDDD-02 (`tddd-02-usecase-wiring-2026-04-14`) はこの問題への対応を明示的に deferral した (ADR 2026-04-11-0002 Phase 1 Completion Amendment §3.B)。理由は 3 点:

- 自然な rename 先である `ReviewFinding` は既に `usecase::review_workflow::ReviewFinding` (DTO) が使っている。
- `domain::review_v2::Finding` は `Verdict::FindingsRemain(NonEmptyFindings)` / `FindingError` などに深く組み込まれており、rename は `review_v2` モジュール全体と infrastructure consumer 群に cascade する。
- 適切な修正は taxonomy レベルであって単一ファイルの rename ではない。tddd-02 の merge gate 作業と並行して実施すると destabilize する恐れがある。

deferral はフォローアップトラックとして明示的に **`tddd-04 finding-taxonomy-cleanup`** を指定した。本 ADR はその follow-up のための設計判断を記録する。

## Decision

### D1: Option B — full rename、統合は行わない

4 つの `Finding` 型は **いずれも別個の Rust 型として維持する**。衝突している domain 側 2 型のみ、last-segment 名が unique になるよう rename する。usecase 層の `ReviewFinding` (serde DTO) と `PrReviewFinding` (Codex Cloud 結果) は既に last-segment 名が distinct なので変更しない。

却下された代替案 2 つについては [Rejected Alternatives](#rejected-alternatives) を参照。

### D2: 新しい名前

| 旧シンボル | 新シンボル | 採用理由 |
|---|---|---|
| `domain::review_v2::Finding` | `domain::review_v2::ReviewerFinding` | この型は `review_v2` に属し、`usecase::review_workflow::ReviewFinding` に対応する domain 側の validated カウンターパートである。`Reviewer` プレフィックスは `CodexReviewer` adapter と自然にペアを組み、`ReviewFinding` (wire DTO) ↔ `ReviewerFinding` (validated domain) という明示的な対応関係を確立する。 |
| `domain::verify::Finding` | `domain::verify::VerifyFinding` | この型は `sotp verify` サブコマンドが生成し、`Severity` を持つ。`Verify` プレフィックスはモジュールパスを反映しており、`VerifyOutcome` から要素型を追うときに読み手が自然に期待する名前である。 |

いずれの新名も workspace 全体で **一意であることを確認済み** (全 `.rs` ファイルの grep で既存の `ReviewerFinding` / `VerifyFinding` 参照がゼロ)。本 track 着地前に他クレートが同名を採用した場合は再評価が必要。

候補として検討し却下した名前:

- `CodexFinding` — domain 型を `CodexReviewer` という特定 adapter に縛ってしまう。domain 型は reviewer プロバイダ非依存であるべき。
- `ReviewRemark` / `CritiqueItem` — 既存コードの語彙に馴染まない。全体として `Finding` を使っているので整合性を欠く。
- `Diagnostic` — コンパイラ/linter 用語と衝突し曖昧。
- `VerificationIssue` — 冗長。`Verify` プレフィックスで十分。
- `CheckResult` — pass/fail の意味合いが強く、個別 finding の意味に合わない。

### D3: Cascade rename 一覧

`domain::review_v2::Finding` と一緒に rename するシンボル:

| 旧 | 新 | Layer |
|---|---|---|
| `Finding` (struct) | `ReviewerFinding` | domain/review_v2 |
| `NonEmptyFindings` | `NonEmptyReviewerFindings` | domain/review_v2 |
| `FindingError` | `ReviewerFindingError` | domain/review_v2 |
| `FindingError::EmptyMessage` | `ReviewerFindingError::EmptyMessage` | domain/review_v2 |
| `mod.rs` の `FindingError` re-export | `ReviewerFindingError` | domain/review_v2 |
| `mod.rs` の `Finding` re-export | `ReviewerFinding` | domain/review_v2 |
| `mod.rs` の `NonEmptyFindings` re-export | `NonEmptyReviewerFindings` | domain/review_v2 |
| test helper `fn finding(msg)` / `fn finding_full()` の戻り値型 | `ReviewerFinding` | domain/review_v2/tests |
| test helper 内の `Finding::new(…)` 呼び出し | `ReviewerFinding::new(…)` | domain/review_v2/tests |
| tests 内の `use super::error::{…, FindingError, …}` | `{…, ReviewerFindingError, …}` | domain/review_v2/tests |
| tests 内の `Err(FindingError::EmptyMessage)` アサーション | `Err(ReviewerFindingError::EmptyMessage)` | domain/review_v2/tests |
| `Verdict::findings_remain(Vec<Finding>)` シグネチャ | `Vec<ReviewerFinding>` | domain/review_v2 |
| `FastVerdict::findings_remain(Vec<Finding>)` シグネチャ | `Vec<ReviewerFinding>` | domain/review_v2 |
| `NonEmptyFindings::{new, as_slice, into_vec}` シグネチャ | `NonEmptyReviewerFindings::{…}` | domain/review_v2 |
| `convert_findings_to_domain` 戻り値型 | `Vec<ReviewerFinding>` | infrastructure/review_v2 |
| `convert_findings_to_domain` 内の `Finding::new(…)` | `ReviewerFinding::new(…)` | infrastructure/review_v2 |
| `persistence/review_store.rs` の `findings: &[Finding]` | `&[ReviewerFinding]` | infrastructure/review_v2/persistence |
| `persistence/review_store.rs` の `Vec<Finding>` / `Finding::new(…)` | `Vec<ReviewerFinding>` / `ReviewerFinding::new(…)` | infrastructure/review_v2/persistence |
| `persistence/tests.rs` の `fn sample_finding() -> Finding` | `fn sample_finding() -> ReviewerFinding` | infrastructure/review_v2/persistence |
| `libs/usecase/src/review_v2/tests.rs` の `use domain::review_v2::{..., Finding, ...}` | `ReviewerFinding` | usecase/review_v2 |
| `libs/usecase/src/review_v2/tests.rs` の `Finding::new(…)` | `ReviewerFinding::new(…)` | usecase/review_v2 |
| `finding_to_review_finding(f: &domain::review_v2::Finding)` の引数型 | `&domain::review_v2::ReviewerFinding` | apps/cli |
| `types.rs` / `error.rs` / `codex_reviewer.rs` / `codex_local.rs` で `# Errors` / `# Returns` セクションが `Finding` に言及している doc comment | `ReviewerFinding` | 全レイヤー (load-bearing な doc のみ) |

`domain::verify::Finding` と一緒に rename するシンボル:

| 旧 | 新 | Layer |
|---|---|---|
| `Finding` (struct) | `VerifyFinding` | domain/verify |
| `Finding::new / error / warning` (constructor) | `VerifyFinding::{new, error, warning}` | domain/verify |
| `#[cfg(test)] mod tests` 内の `Finding::error(…)` / `Finding::warning(…)` 呼び出し (line 146, 155, 162-165, 179 の 9 箇所) | `VerifyFinding::error(…)` / `VerifyFinding::warning(…)` | domain/verify (test module) |
| `Finding::severity / message` (accessor) | `VerifyFinding::{severity, message}` | domain/verify |
| `impl fmt::Display for Finding` | `impl fmt::Display for VerifyFinding` | domain/verify |
| `VerifyOutcome { findings: Vec<Finding> }` field 型 | `Vec<VerifyFinding>` | domain/verify |
| `VerifyOutcome::pass()` / `is_ok()` / `has_errors()` / `error_count()` | シグネチャは変わらない (body の `self.findings` が暗黙的に新型になる)。取りこぼしを防ぐため網羅表に含める。 | domain/verify |
| `VerifyOutcome::from_findings(Vec<Finding>)` / `findings() -> &[Finding]` / `add(Finding)` / `merge` | 全て `VerifyFinding` に更新 (明示的なシグネチャ変更あり) | domain/verify |
| domain consumer 側の `use crate::verify::{Finding, …}` | `VerifyFinding` | domain/{tddd/consistency.rs, spec.rs} |
| usecase consumer 側の `use domain::verify::{Finding, …}` | `VerifyFinding` | usecase/{merge_gate.rs, task_completion.rs} |
| 18 個の infra verify ファイルの `use domain::verify::{Finding, …}` | `VerifyFinding` | `libs/infrastructure/src/verify/*.rs` (18 files) |
| CLI 内の `domain::verify::Finding::error(…)` 呼び出し | `VerifyFinding::error(…)` | `apps/cli/src/commands/verify.rs` |
| `verify.rs:26` の doc comment `/// A single verification finding.` | `VerifyFinding` を参照するよう更新 | domain/verify |
| `source-attribution.md` の prose `Finding::warning` 言及 | `VerifyFinding::warning` | knowledge/conventions |

### D4: 意図的に rename しないシンボル

- **`VerdictError::EmptyFindings`** — variant 名は「findings コレクションが空である」という意味を正確に記述している。`EmptyReviewerFindings` への rename は過剰設計。`VerdictError` enum 自体も変更しない。
- **`Verdict` / `FastVerdict` / `VerifyOutcome` / `Severity`** — 型名は変えない。内部の要素型のみ差し替わる。
- **`REVIEW_OUTPUT_SCHEMA_JSON`** — JSON schema は `$defs/finding` を内部識別子として定義している。`serde_json` は Rust 型名ではなく struct のフィールド名でシリアライズするため、wire format は Rust 型の rename に対して不変。schema 更新は不要。
- **`usecase::review_workflow::ReviewFinding`** — 既に distinct な名前。DTO はそのまま維持。
- **`usecase::pr_review::PrReviewFinding`** — 既に distinct。そのまま維持。
- **`domain::auto_phase::FindingSeverity`** — 無関係な enum (`P1`/`P2`/`P3`)。ここでの `Finding` は複合形容詞であり型参照ではない。`struct Finding` / `enum Finding` / `use.*Finding` のような targeted rename でこのシンボルを壊さないこと。
- **過去の track 成果物** (`track/items/tddd-02-*/` 配下など) — 遡及的な rename は行わない。過去の ADR や設計スニペット (過去 ADR の code block 含む) は当時のコードそのままの記録として残す。本 ADR は rename を forward-only とする。
- **例外**: `track/items/tddd-01-multilayer-2026-04-12/domain-types.json` は **live catalogue source** であり、`architecture-rules.json` から参照されている。これは固まった履歴成果物ではなく、TDDD tooling がシグナル評価のたびに読む active なデータファイルである。T006 はこのファイルを明示的に更新対象に含める。track ドキュメント (spec.md / plan.md / verification.md) とは扱いが異なる点に注意 — 後者は固定化された履歴である。

### D5: 後方互換性なし (前例踏襲)

tddd-01 / tddd-02 の前例および 2026-04-13 のユーザーガイダンスに従い、`pub use Finding = ReviewerFinding;` のような alias も deprecated shim も migration path も提供しない。旧名は即時削除する。`domain-types.json` catalogue は T006 で再承認される — 単一の `"Finding"` reference entry を削除し、新名の 3 つの `declare` entry に差し替える。

### D6: domain purity を維持する

`domain::review_v2::ReviewerFinding` に `Serialize` / `Deserialize` derive は **追加しない**。`libs/domain` crate 自体は既に `serde` 依存を持つ (`catalogue.rs` / `schema.rs` が使用) が、`ReviewerFinding` 型本体は serialization free のままとする。これにより DTO / domain の分離が保たれ、hexagonal な分離も維持される:

- `usecase::review_workflow::ReviewFinding` = serde DTO (wire format)
- `domain::review_v2::ReviewerFinding` = validated domain newtype (non-empty message 不変条件 + private field + constructor が `Result` を返す)
- `infrastructure::review_v2::codex_reviewer::convert_findings_to_domain` = DTO → domain の variant conversion。`filter_map` で empty-message の finding を silent に捨てる (load-bearing な不変条件境界)
- `apps/cli/src/commands/review/codex_local::finding_to_review_finding` = domain → DTO の variant conversion (JSON 出力用)

non-empty message 不変条件は **load-bearing** である。これは untrusted な reviewer 出力と domain-trusted な値の間のフィルター境界であり、この境界を溶かすような統合案 (Rejected Alternatives 参照) は受け入れない。

### D7: `domain-types.json` catalogue の更新

`track/items/tddd-01-multilayer-2026-04-12/domain-types.json` の suppression entry (`type_definitions` 配列の 4 番目、`"Finding"` reference entry) は **削除** する。代わりに 3 つの新しい `declare` entry を追加する:

```json
{
  "name": "ReviewerFinding",
  "description": "Domain-validated reviewer finding. Invariant: message is non-empty. Counterpart to usecase::review_workflow::ReviewFinding (serde DTO).",
  "approved": true,
  "action": "declare",
  "kind": "value_object"
},
{
  "name": "NonEmptyReviewerFindings",
  "description": "Non-empty collection of ReviewerFinding values. Used as the inner payload of Verdict::FindingsRemain and FastVerdict::FindingsRemain.",
  "approved": true,
  "action": "declare",
  "kind": "value_object"
},
{
  "name": "VerifyFinding",
  "description": "Structured error or warning produced by sotp verify subcommands. Has a Severity (Info/Warning/Error) and a message string.",
  "approved": true,
  "action": "declare",
  "kind": "value_object"
}
```

catalogue 更新後、`sotp track type-signals tddd-04-finding-taxonomy-cleanup-2026-04-14 --layer domain` を再実行すること (注: `type-signals` は track ID を `--layer` の前に positional 引数として取る)。期待される結果: `yellow=0 red=0`、`blue` カウントが少なくとも 2 増加 (3 つの新 `declare` エントリが 1 つの reference エントリを置き換える。旧 reference エントリも blue としてカウントされていたので、ネットで +3 − 1 = +2 blue 以上)。

### D8: タスクの順序と commit のグルーピング

実装は 7 タスク (T001-T007) に分割し、**4 つの atomic commit** にグルーピングする。各 commit は 500 行未満で、workspace を compile-clean な状態に保つこと。D5 (no-alias rule) により cascade グループは atomic に着地する必要がある — domain の rename 単独では依存 crate が compile できなくなる (旧名が既に存在せず、互換 alias も提供しないため)。

**Commit グルーピング:**

- **Commit 1 (~220 行)**: T001 + T002 + T003 — `verify::Finding` → `VerifyFinding` の full cascade
  - T001: `libs/domain` (verify.rs の struct/impl/Display/tests、加えて domain consumer である tddd/consistency.rs と spec.rs — spec.rs:517-521, 1638, 1652 および consistency.rs:356-361 の `# Rules` セクション doc comment 内の `Finding::error` / `Finding::warning` 言及も更新対象)
  - T002: `libs/usecase` (merge_gate.rs, task_completion.rs)
  - T003: `libs/infrastructure/src/verify/` 配下 18 ファイル (spec_states.rs:32-36, 186 の doc comment 言及含む)、`apps/cli/src/commands/verify.rs`、および `knowledge/conventions/source-attribution.md` の line 29 prose
- **Commit 2 (~135 行)**: T004 + T005 — `review_v2::Finding` → `ReviewerFinding` の full cascade
  - T004: `libs/domain/src/review_v2/` (types.rs, error.rs, mod.rs, tests.rs)
  - T005: `libs/infrastructure/src/review_v2/codex_reviewer.rs`、`libs/infrastructure/src/review_v2/persistence/review_store.rs`、`libs/infrastructure/src/review_v2/persistence/tests.rs`、`libs/usecase/src/review_v2/tests.rs`、および `apps/cli/src/commands/review/codex_local.rs`
- **Commit 3 (~30 行)**: T006 — `domain-types.json` catalogue 更新 (reference entry 削除、3 つの declare entry 追加、signals 再生成)
- **Commit 4 (0-10 行)**: T007 — `cargo make ci` の full CI gate + 残留 `verify::Finding` / `review_v2::Finding` / `struct Finding` の明示的な grep (期待値ゼロ)。`sotp track baseline-capture tddd-04-finding-taxonomy-cleanup-2026-04-14 --layer domain --force` の出力から `same-name type collision for Finding` stderr warning が消えていることを確認する

**順序制約:**

- verify 側の cascade (Commit 1) と review 側の cascade (Commit 2) は独立しており、どちらを先に着地させてもよい。
- Commit 3 (T006、catalogue 更新) は Commit 1 と Commit 2 の両方が着地した **後** に実行する必要がある。`sotp track type-signals` が `VerifyFinding` と `ReviewerFinding` を Blue として評価するには、再生成された rustdoc JSON に両名が含まれていなければならない。
- Commit 4 (T007) は最終ゲート。

**なぜ 1 task = 1 commit ではないのか?** D5 の no-alias rule により、T001 単独では `cargo build -p usecase` が失敗する (usecase は依然として `Finding` を domain から import するが domain は既に `VerifyFinding` しか export していない)。同様に T004 単独では `cargo build -p infrastructure` が失敗する。タスク番号 T001-T007 はドキュメント・進捗管理・task レベルの記述粒度を保つために維持するが、commit の粒度は 4 つの atomic group となる。

## Rejected Alternatives

### A1: Option A — (1) と (3) を単一の型に統合する

`domain::review_v2::Finding` に `Serialize` / `Deserialize` derive を追加し、`usecase::review_workflow::ReviewFinding` を削除する。衝突を解消するため (2) はリネームする。

却下理由:

- `domain::review_v2::Finding` (新名 `ReviewerFinding`) に `Serialize` / `Deserialize` を追加すると、domain 型が serializable になり DTO / domain 境界が溶ける。注: `libs/domain` は既に `serde` 依存を持つ (`catalogue.rs` / `schema.rs` で使用) ため、新規の crate 依存は入らない。しかし serde の利用範囲が validated newtype レイヤーにまで拡張される — これは `knowledge/conventions/hexagonal-architecture.md` が示す hexagonal intent (validated domain type は wire format 型になってはならない) に反する。
- DTO / domain 境界が崩れる。現状 `Finding::new()` + `FindingError::EmptyMessage` + `convert_findings_to_domain` の `filter_map` が強制している non-empty message 不変条件が、(a) 完全に失われるか、(b) 表現の面倒な domain-level な serde validation hook を必要とする。この不変条件は load-bearing であり、現在 empty-message な reviewer 出力は silent に捨てられていて、上流コードはこの挙動に依存している。
- 今後「この domain 型にも serde を追加したい」という要望に抵抗しづらい前例を作る。

### A2: Option C — (1) を削除して DTO を直接使用する

`domain::review_v2::Finding` を削除し、`convert_findings_to_domain` と `apps/cli/src/commands/review/codex_local::finding_to_review_finding` が `usecase::review_workflow::ReviewFinding` を直接扱うようにする。衝突解消のため (2) は `VerifyFinding` にリネームする。

却下理由:

- non-empty message 不変条件が失われる。`ReviewFinding.message: String` は任意の文字列であり、非空を強制する仕組みがない。下流コード (`Verdict::FindingsRemain`) が DTO の値を直接保持することになり、domain モデルが弱体化する。
- `Verdict::FindingsRemain(NonEmptyFindings)` は (a) DTO を包むか (awkward — domain aggregate が infrastructure type を包むのは unnatural)、(b) 構造的な非空保証を失って消滅するか、のいずれかを強いられる。どちらも review state machine の設計意図を損なう。
- 「簡素化した」と見せかけて実際は認知負荷が増える。呼び出し側は「この `ReviewFinding` は reviewer JSON 由来なので message が空でないことを信用してはいけない」と毎回覚えておく必要がある。現行の validated-newtype パターンはこの懸念を境界で済ませている。

### A3: 衝突する側だけリネームして命名を不揃いにする

`domain::review_v2::Finding` → `ReviewerFinding` だけリネームし、`domain::verify::Finding` は触らない。catalogue には `verify::Finding` 用の別の suppression entry を追加する。

却下理由:

- 衝突は両者が last-segment 名を共有していることが原因。片方だけをリネームしても衝突は解消するが、catalogue に不要な非対称性 (`ReviewerFinding` は declared、`Finding` は `verify::Finding` 経由で暗黙参照) が残る。
- TDDD の last-segment 名ポリシーは全型名が workspace 全体で一意になることを好む。片方のみの rename は問題の先送りであり、将来のメンテナが結局もう片方も rename せざるを得なくなる。
- 一貫性: 片方の domain `Finding` を可読性のためにリネームするなら、もう片方も対称的にリネームすべき。

## Consequences

### Positive

- **既知の suppression entry を消せる**。`domain-types.json` の `"Finding"` reference entry の `description` は現状「documented as a known issue」と書かれている — 本 rename でこの負債を返済する。
- **`code_profile_builder.rs:54` の stderr ノイズが消える**。baseline capture のたびに出ていた `warning: same-name type collision for Finding` がなくなる。
- **hexagonal 純粋性を維持できる** (Option B vs Option A)。DTO / domain の分割がそのまま残る。
- **non-empty message 不変条件を維持できる** (Option B vs Option C)。validated newtype 境界が untrusted な reviewer 出力をフィルタする役割を継続する。
- **コードベースに可読なペアリングを導入できる**: `usecase::review_workflow::ReviewFinding` (DTO) ↔ `domain::review_v2::ReviewerFinding` (validated)。旧名 `Finding` ではこの対称性が見えにくかった。
- **TDDD catalogue に 3 つの新エントリを明示できる** (`ReviewerFinding` / `NonEmptyReviewerFindings` / `VerifyFinding`)。これまで suppression されていた型が正式にカバーされる。
- **将来の TDDD 解析が両方の `Finding` を独立に見られる**。これまでは rustdoc 実行ごとに TypeGraph に片方しか出てこなかった。

### Negative

- **Cascade rename の cost**: domain / usecase / infrastructure / CLI 層をまたいで 40〜60 ファイルに触れる (うち 18 が infra/verify ファイルで各々数箇所の call site を持つ)。mechanical だが非自明。推定差分は **4 atomic commit** で ~395 行 (grouping 詳細は D8 参照)。4-commit grouping は D5 (no-alias rule) により load-bearing: 各 commit が self-contained かつ compile-clean でなければならないため、7 タスクが 4 commit にマップされる — Commit 1: T001+T002+T003 (verify::Finding cascade, ~220 行)、Commit 2: T004+T005 (review_v2::Finding cascade, ~135 行)、Commit 3: T006 (~30 行)、Commit 4: T007 (0-10 行)。
- **過去 ADR / 設計ドキュメントとの drift**: `2026-04-12-1200-strict-spec-signal-gate-v2.md` は 33 箇所の `Finding::error` / `Finding::warning` pseudo-code を含み (2026-04-14 実測)、`2026-04-04-1456-review-system-v2-redesign.md` は 8 箇所の `Finding` struct / `Vec<Finding>` を Rust code block に含む (2026-04-14 実測)。これらは当時のコードを記述した歴史的設計記録であり、そのまま残す。過去 ADR を現行コードと照らして参照する読者は、名前が変わっていることを認識しておく必要がある。
- **`sotp verify arch-docs` のスコープ**: 2026-04-14 に merge された tddd-02 の CI 実績により、historical ADR に旧名が残っている状態でも `arch-docs` は ADR code block 内の Rust 型参照を lint しないことが確認済み。本 track は `cargo make ci` が historical ADR 編集なしで pass することを前提とする。将来 `arch-docs` が code block も lint するようになり historical reference が引っかかった場合、対処は follow-up track (`historical-adr-lint-resolution`) に委ねる — 本 track (`tddd-04`) のスコープは Finding rename に限定し、T007 でも historical ADR は編集しない。
- **Track スコープの catalogue 保存**: `track/items/tddd-01-multilayer-2026-04-12/domain-types.json` は live catalogue source (`architecture-rules.json` から参照される) であり、完了済み track の成果物を follow-up track で更新するのは通常のパターンではない。現状の per-track catalogue 配置では回避不可。将来 domain 型を変える track も同じパターンに直面する。

### Neutral

- **`VerdictError::EmptyFindings` を維持**: variant 名は「findings collection が空である」という意味を正確に記述しており、`EmptyReviewerFindings` への rename は過剰設計。これは受容したトレードオフ。
- **4 つの Finding 型は依然存在**: Option B は 4 型を全て distinct に保つ。これは intentional — それぞれが別の役割を持つ。問題は *衝突* であって *多重性* ではない。
- **JSON wire format は変わらない**: reviewer JSON の `$defs/finding` キーは schema 識別子であり Rust 型名ではない。wire format の変更なし、reviewer prompt の更新なし、下流 tooling の更新なし。

## Reassess When

- **Adoption プロジェクトが DTO / domain 統合を要望した場合**: SoTOHE を採用するプロジェクトが「単一の `Finding` 型 + serde にして non-empty 不変条件の喪失も受け入れる」と明言した場合は Option A を再検討する。
- **`review_v2` の再設計**: 将来的に review system を再設計して `domain::review_v2::Finding` が完全に廃止される (例: severity ごとの `ReviewerRemark` enum に置き換え) 場合、本 rename は自明に revert できる。
- **3 つ目の衝突が発生した場合**: 将来の track が 5 つ目の `Finding` family 型を導入した場合、本 ADR の命名ルーブリックを ad-hoc ではなく一貫して適用すること。
- **TDDD catalogue が per-track 配置から移動した場合**: `domain-types.json` が `track/items/tddd-01-*/` の外に置かれるようになれば、catalogue 更新タスクは完了済み track 成果物の編集ではなく canonical ファイルの単純編集となる。

## Related

- **ADR `2026-04-11-0002-tddd-multilayer-extension.md`** (Phase 1 Completion Amendment §3.B): 本 track を指名した deferral 通知。
- **ADR `2026-04-13-1813-tddd-taxonomy-expansion.md`**: `TraitPort` → `SecondaryPort` cascade rename の前例。同じ「後方互換性なし」ポリシー。
- **`knowledge/conventions/hexagonal-architecture.md`**: Option A を却下する根拠の domain purity ルール。
- **`.claude/rules/04-coding-principles.md`**: 「Make Illegal States Unrepresentable」 — non-empty message 不変条件はこの原則の具体的な適用であり、Option C はこれを弱める。
- **`libs/infrastructure/src/code_profile_builder.rs` (~line 54)**: collision warning を emit しているポイント。
- **`track/items/tddd-01-multilayer-2026-04-12/domain-types.json` (`"Finding"` reference entry、`type_definitions` の 4 番目)**: 本 track で削除する suppression entry。
- **`knowledge/research/2026-04-14-0625-planner-tddd-04-finding-taxonomy.md`**: Claude Opus の full planner 出力 (Canonical Blocks、data-flow 図、rename 表を含む 7 セクション)。
- **`tmp/handoff/tddd-04-finding-taxonomy-handoff-2026-04-14.md`**: tddd-02 セッション発の original handoff で問題をフレーミングしたもの。
