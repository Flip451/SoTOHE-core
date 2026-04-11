<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# TDDD-03: Type action declarations — add/modify/reference/delete

ADR 2026-04-11-0003 (TDDD-03) の実装。domain-types.json の各エントリに optional な action フィールド (add/modify/reference/delete) を追加し、型操作の意図を明示的に記録する。
TDDD-02 の「既存型の削除と TDDD の併用不可」制約を解消する。action: delete で意図的な削除を宣言でき、kind migration (struct→trait) は同名 delete+add ペアで表現する。
action と baseline の矛盾を警告 (contradiction) として検出し、delete + baseline 不在はエラーとして阻止する。

## Domain 層: TypeAction enum + DomainTypeEntry 拡張

TypeAction enum (Add/Modify/Reference/Delete, Default=Add) を catalogue.rs に定義する。
DomainTypeEntry に action フィールドを追加し、new() シグネチャを変更する。
全 call site (テスト含む) を修正する。

- [x] Domain: TypeAction enum (Add/Modify/Reference/Delete) 定義 + DomainTypeEntry に action フィールド追加 + new() シグネチャ変更 + 全 call site 修正 bf44c19

## Domain 層: Delete forward check 反転

evaluate_delete ヘルパー関数を追加。TraitPort は graph.get_trait()、他は graph.get_type() で判定。
evaluate_single で TypeAction::Delete を kind dispatch 前に分岐。不在→Blue、存在→Yellow。

- [x] Domain: evaluate_delete ヘルパー追加 + evaluate_single で Delete を kind dispatch 前に分岐 (TraitPort は get_trait、他は get_type で不在=Blue/存在=Yellow) bf44c19

## Domain 層: ConsistencyReport 拡張 + contradiction 検出

ActionContradiction struct + ActionContradictionKind enum を定義する。
ConsistencyReport に contradictions (Vec<ActionContradiction>) と delete_errors (Vec<String>) を追加。
check_consistency に action-baseline 矛盾検出ロジックと delete baseline 検証を実装する。
check_consistency の TDDD-03 関連コメントを更新する。

- [x] Domain: ActionContradiction / ActionContradictionKind 型 + ConsistencyReport に contradictions と delete_errors フィールド追加 bf44c19
- [x] Domain: check_consistency に contradiction 検出 (add+baseline=warn, modify+no-baseline=warn, reference+no-baseline=warn, reference+not-blue=warn) + delete baseline 検証 (delete+no-baseline=error) + TDDD-03 コメント更新 bf44c19

## Domain 層: exports 更新

lib.rs の pub use に TypeAction, ActionContradiction, ActionContradictionKind を追加する。

- [x] Domain: lib.rs exports 更新 — TypeAction, ActionContradiction, ActionContradictionKind を pub use に追加 bf44c19

## Infrastructure 層: Codec + Render

TypeActionDto enum (serde rename_all snake_case) を定義。action フィールドを DTO に追加 (default=add, skip_serializing_if)。
duplicate name 検証を緩和: 同名エントリは delete+add ペア (2件) のみ許可、3件以上は常にエラー。
domain_types_render に Action 列を追加 (Add='—', 他=action名)。

- [x] Infrastructure: Codec DTO 拡張 — TypeActionDto enum + action フィールド (default=add, skip_serializing_if=add) + duplicate name 検証緩和 (同名は delete+add ペアのみ許可、3件以上は常にエラー) 4fbde52
- [x] Infrastructure: domain_types_render に Action 列追加 (Add='—', 他=action名) 4fbde52

## CLI 層: verify + signals

verify.rs: contradictions → Finding::warning, delete_errors → Finding::error, JSON 出力に新フィールド追加。
signals.rs: contradictions → [WARN] 出力, delete_errors → CliError。

- [x] CLI: verify.rs に contradictions → Finding::warning + delete_errors → Finding::error + print_consistency_report_json に新フィールド追加 4fbde52
- [x] CLI: signals.rs に contradictions → [WARN] 出力 + delete_errors → CliError 失敗 4fbde52

## /track:design コマンド更新

Step 2 に action 選択ガイダンスを追加する。
Step 3 の JSON スキーマ例に action フィールドを追加する。

- [x] Command: /track:design の Step 2 に action 選択ガイダンス追加 + Step 3 の JSON スキーマ例に action フィールド追加 4fbde52
