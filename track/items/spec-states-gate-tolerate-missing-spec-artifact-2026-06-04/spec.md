<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 41, yellow: 0, red: 0 }
---

# spec-states commit ゲートを spec 成果物未生成の段階でも通す + dry-checker 運用修正

## Goal

- [GO-01] `verify spec-states`（spec パス引数なし、ブランチからトラックを自動解決する経路）が、spec 成果物（spec.json / spec.md のいずれも）が存在しない Phase 0 の段階では評価を skip（no-op + success、SKIP 表示）し、非ゼロ終了しないようにする。これにより commit ゲート（`cargo make ci` 経由の `verify-spec-states-current`）が Phase 0 でも通り、「init 直後に review → commit で ADR を初回 commit する」標準フローが機能する [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1]
- [GO-02] `sotp dry` 一式（`dry write` / `dry check-approved` / `dry fix-local`）が Codex/ChatGPT アカウント環境で通しで成立するよう、エージェントモデル解決・出力スキーマ整合・インデックス成果物の除外・insert と埋め込みの一括化・インデックス永続化と増分維持という運用成立条件を修正する [adr: knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D1, knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D2, knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D3, knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D4, knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D5, knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D6, knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D7]

## Scope

### In Scope
- [IN-01] トラック解決経路（spec パス未指定で、ブランチから対象トラックを解決する経路）において、spec.json / spec.md のいずれも存在しない場合に skip（no-op + success）を返す挙動の実装を対象とする [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [IN-02] spec 成果物が存在するフェーズ（Phase 1 以降）では、従来どおりシグナルを評価する。🔴 はゲートを block し、CI 中間モードでは 🟡 は warning、merge ゲートの strict モードでは 🟡 も block するという既存の使い分けを不変に保つ [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [IN-03] skip 判定の精度担保を対象とする。skip は「spec.json / spec.md のいずれも存在しない」ことを実ファイル存在で厳密に判定した場合にのみ発動し、成果物がある状態での fail-open を作らない [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [IN-04] skip 時のユーザー向け出力（SKIP 表示）を対象とする。skip した旨が観測できる出力を標準出力または標準エラーに表示し、silent no-op にしない [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [IN-05] 新しい skip 分岐のテストカバレッジを対象とする。spec 成果物が存在しない場合に skip を返すことを確認するテストを追加する [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [IN-06] DRY 検出が生成する LanceDB インデックス成果物（既定パスの `.semantic_index/` ディレクトリ、その sidecar ファイル群 `.semantic_index.*`（`.sotp-cache` / `.lock` / `.manifest` 等、D6/D7 が永続インデックスと併せて生成するキャッシュマーカー・ファイルシステムロック・ファイル単位マニフェスト）、および一時インデックス `sotp-dry-index-*/`）を `.gitignore` に追加し、バージョン管理対象から除外する。グロブ `.semantic_index*` でディレクトリと全 sidecar を一括して覆える。再生成可能なローカル成果物であり、commit に混入させない [adr: knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D3] [tasks: T002]
- [IN-07] セマンティックインデックスへの insert を一括化する。コーパス構築は全フラグメントを単一の RecordBatch + 単一の追加操作で投入する。このため意味インデックスのポート（`SemanticIndexPort`、usecase 層）に一括投入オペレーション（`insert_batch`）を追加し、コーパス構築を行う各インタラクタ（DRY 書き込み・承認確認）が `insert_batch` を使う。単件 insert は別の利用者のために残置する [adr: knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D4] [tasks: T002]
- [IN-08] コーパス構築における埋め込みを一括化する。埋め込みポート（`EmbeddingPort`、usecase 層）に一括埋め込みオペレーション（`embed_batch`）を追加し、コーパス構築を行う共有ヘルパーが `embed_batch` を呼ぶ。型カタログは `embed_batch` をこの spec 要素（IN-08）を根拠として宣言することでグラウンディングする。単件 `embed` は他の利用者がいれば残置する [adr: knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D5] [tasks: T002]
- [IN-09] セマンティックインデックスを `--db-path`（既定 `.semantic_index`、D3 で gitignore 済み）に永続化する（D6）。永続インデックスの安全な消去・並行制御のためのキャッシュ安全機構（マーカー / ロック）も維持する（D6 が実装詳細を track 実装へ委譲、D7 がその維持を明記） [adr: knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D6, knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D7] [tasks: T002]
- [IN-10] セマンティックインデックスをファイル単位の内容ハッシュで増分維持する。単一フィンガープリントに代えて、各ソースファイルの内容ハッシュのマニフェスト（`{path → 内容ハッシュ}` + 埋め込みモデル識別子）を永続化する。実行時、現在のワーキングツリーの `{path → 内容ハッシュ}` をマニフェストと差分比較し変化分だけを更新する: 内容ハッシュ変化・新規ファイルは既存フラグメントを `source_path` 単位で削除してから抽出・埋め込み・挿入、消えたファイルは削除のみ、不変ファイルは何もしない（再埋め込みしない）。差分が空の場合は削除も挿入も発生しない。埋め込みモデル識別子が変わった場合は全ファイルを dirty 扱いとして全再構築する。鍵はワーキングツリーの内容ハッシュとし、git コミットハッシュは使わない（未コミット・未追跡も正しく扱うため）。この増分維持には `SemanticIndexPort`（usecase 層）に「`source_path` を指定してそのファイルの全フラグメントを削除する」オペレーションが必要になる。型変更（新 port メソッド等）は本 ADR の D7 を根拠として型カタログに宣言して grounding する（Phase 2 で実施） [adr: knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D7] [tasks: T003]

### Out of Scope
- [OS-01] 明示的 spec パス指定経路（`verify spec-states <path>`）の挙動変更は対象外とする。この経路は従来どおり当該ファイルを検証し、ファイルが存在しなければエラーとする [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1]
- [OS-02] シグナル評価セマンティクス（🔵🟡🔴 の評価ルール）の変更は対象外とする。spec 成果物が存在するフェーズでは評価ロジックをそのまま維持する [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1]
- [OS-03] 他の verify ゲート（`verify-plan-artifact-refs`、`verify-catalogue-spec-refs`、`verify-latest-track`、`check-catalogue-spec-signals`）の挙動変更は対象外とする [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1]
- [OS-04] commit ゲートから `verify-spec-states-current` を丸ごと外す案は対象外とする。ゲート自体を bypass すると spec 成果物が揃った後のフェーズでもシグナル評価が走らず fail-open になる。ADR Rejected Alternative A として記録されている [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1]
- [OS-05] Phase 0 専用の別 commit 経路の新設は対象外とする。兄弟チェックと同じ「入力不在なら skip」という一様なルールで足りるため、フェーズ別の経路を増やすのは不要な複雑化。ADR Rejected Alternative B として記録されている [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1]
- [OS-06] トラック自動解決経路の廃止（spec パスを常に必須引数にする案）は対象外とする。他の兄弟チェックはトラック自動解決 + 入力不在 skip で揃っており、spec-states だけ設計を変えると一貫性が崩れる。ADR Rejected Alternative C として記録されている [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1]
- [OS-07] skip 判定の具体的な実装位置（`build_spec_path_from_track_id` 関数の変更箇所、`dispatch_spec_states_with_resolver` のテスト対象など）の特定は対象外とする。これらは Phase 2 / 3 の関心事であり、本 spec は振る舞い契約のみを記述する [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1]
- [OS-08] DRY 検出の判定基準（類似閾値・判定モデルの良否）の変更は対象外とする。本トラックは運用成立条件（モデル解決・スキーマ整合・成果物 gitignore・insert 一括化・埋め込み一括化・インデックス永続化と増分維持）のみを対象とし、検出精度は不変とする [adr: knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D1]
- [OS-09] 単件 insert（`SemanticIndexPort` の既存の insert オペレーション）の削除は対象外とする。単件 insert は別の利用者が使っているため残置する [adr: knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D4]
- [OS-10] 単件 `embed`（`EmbeddingPort` の既存の embed オペレーション）の削除は対象外とする。単件 `embed` は他の利用者がいれば残置し、削除の要否は本トラックの対象外とする [adr: knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D5]
- [OS-11] フラグメント粒度の差分インデックス更新（変更されたフラグメントを個別に特定して部分的に再埋め込みし無効化する方式）は対象外とする。正確な差分追跡・古いフラグメントの部分無効化が必要で実装が複雑になる。フラグメント単位の差分更新は ADR Rejected Alternative E（D6 の代替として記録）として却下されている。ファイル単位（`source_path` 単位の削除→再挿入）の増分維持（D7, IN-10）で反復コストを解消できれば十分であり、フラグメント粒度の部分無効化は将来の最適化として保留する。なお、ファイル単位の増分維持（`source_path` 単位の削除→再挿入。D7）は IN-10 として in-scope であり、本項が除外するのはフラグメント粒度の部分無効化に限定する [adr: knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D6, knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D7]

## Constraints
- [CN-01] skip は「spec.json / spec.md のいずれも実在しない」ことを厳密に判定した場合にのみ発動する。どちらか一方でも存在すれば通常評価を行い、fail-open を作らない [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [CN-02] skip 挙動は、同じ commit ゲートで既に欠損入力を skip している兄弟チェック（`verify-latest-track`、`verify-plan-artifact-refs`、`verify-catalogue-spec-refs`、`check-catalogue-spec-signals`）の寛容さに揃える。呼び出し経路（gate 経由か直接か）によって挙動が変わらない [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [CN-03] spec 成果物が存在するフェーズ（Phase 1 以降）では、シグナル評価の厳格性を従来どおり維持する。🔴 は引き続きゲートを block し、CI 中間モードと strict モードの使い分けも不変 [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [CN-04] `cargo make ci`（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する状態を維持する [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [CN-05] `sotp dry write` のエージェントモデルは、`--model` を明示指定しない場合、`agent-profiles.json` の `dry-checker` capability から解決する（`AgentProfiles::resolve_execution(capability_name, Final)`）。リテラル `"codex"` を既定値としてアダプタへ渡す従来動作は廃止する。`--model` 明示値は上書きとして尊重する [adr: knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D1] [tasks: T002]
- [CN-06] DRY 判定エージェントの構造化出力スキーマ（`response_format`）は OpenAI strict モード準拠とする。`required` 配列に `properties` の全キーを列挙する。`refactor_proposal` は違反でない場合に値を持たないため型は nullable（`["string", "null"]`）を維持したうえで `required` に含める。スキーマの不変条件（`required` ⊇ `properties` のキー集合）を回帰テストで固定する [adr: knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D2] [tasks: T002]

## Acceptance Criteria
- [ ] [AC-01] `verify spec-states`（spec パス引数なし）が、spec.json / spec.md のいずれも存在しない状態（Phase 0）でゼロ終了し、SKIP を示す出力が観測できる [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [ ] [AC-02] `verify spec-states`（spec パス引数なし）が、spec.json / spec.md が存在する状態（Phase 1 以降）では従来どおりシグナルを評価し、🔴 の場合に非ゼロ終了する（skip による fail-open が発生していない） [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [ ] [AC-03] `verify spec-states <path>`（明示的 spec パス指定経路）の挙動が変わらない。spec パスが存在しない場合にエラーとなる従来動作を維持する [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [ ] [AC-04] spec 成果物が存在しない場合に skip を返すことを確認するテストが追加されており、`cargo make test` でパスする [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [ ] [AC-05] `cargo make ci` が pass する（fmt-check + clippy + nextest + deny + check-layers + verify-* の全ステップ） [adr: knowledge/adr/2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact.md#D1] [tasks: T001]
- [ ] [AC-06] `sotp dry write` が `--model` を省略した場合に `agent-profiles.json` の `dry-checker` capability からモデルを解決して実行し、リテラル `"codex"` を既定値とする従来動作で発生していた 400 エラーが再現しない [adr: knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D1] [tasks: T002]
- [ ] [AC-07] DRY 判定エージェントの構造化出力スキーマについて、`required` に `properties` の全キー（`refactor_proposal` を含む）が列挙されており、`refactor_proposal` が nullable required として扱われることを、外部 API 呼び出しに依存しないローカルの strict-schema 不変条件テストで確認できる [adr: knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D2] [tasks: T002]
- [ ] [AC-08] `.semantic_index/` とその sidecar 群（`.semantic_index.*`: `.sotp-cache` / `.lock` / `.manifest`）および `sotp-dry-index-*/` が `.gitignore` に登録されており（グロブ `.semantic_index*` で一括して覆われている）、`cargo make add-all`（または同等の一括ステージング操作）でこれらのパスが commit に含まれない [adr: knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D3] [tasks: T002]
- [ ] [AC-09] インデックス構築（`sotp dry write` のコーパスビルドパス）が単一の一括 insert を発行する。ワークスペース規模（約 400 ソースファイル相当）のコーパスを対象とした DRY 実行が、フラグメント単位のトランザクション storm を起こさずに完了する [adr: knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D4] [tasks: T002]
- [ ] [AC-10] コーパス構築（`sotp dry write` のコーパスビルドパス）が、全フラグメントに対して単一の一括埋め込み呼び出し（`embed_batch`）を発行する。フラグメント単位の CPU 推論ループが主たる遅延要因でなくなり、ワークスペース規模（約 400 ソースファイル相当）のコーパスビルドが実用的な時間で完了する [adr: knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D5] [tasks: T002]
- [ ] [AC-11] 不変コーパスに対する 2 回目の DRY 実行（例: `dry write` 直後の `dry check-approved`、または同一コード上の連続実行）が、埋め込みフェーズをスキップしてインデックスを再利用することを確認できる（ログ出力またはテストで再埋め込みが走っていないことが観測可能） [adr: knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D6] [tasks: T002]
- [ ] [AC-12] ファイル単位マニフェスト（`{path → 内容ハッシュ}` + 埋め込みモデル識別子）が永続化されており、実行時に差分判定に使われることを確認できる。ソースファイルを変更・追加した場合、変更されたファイルのフラグメントのみが再埋め込み・再挿入され、不変ファイルのフラグメントは再埋め込みされない（ログ出力またはテストで不変ファイルが再埋め込みされないことが観測可能）。削除されたファイルのフラグメントはインデックスから除去されており、stale フラグメントが残らない。埋め込みモデル識別子が変わった場合は全ファイルが dirty 扱いとなり全再構築が走る [adr: knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D7] [tasks: T003]

## Related Conventions (Required Reading)
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- .claude/rules/04-coding-principles.md#No Panics in Library Code

## Signal Summary

### Stage 1: Spec Signals
🔵 41  🟡 0  🔴 0

