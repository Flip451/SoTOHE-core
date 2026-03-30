<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# review.json 分離 + group 独立レビュー + per-group hash

review state を metadata.json から review.json に分離し、group 独立進行モデルへ移行する。
track/review-scope.json は policy source として残し、mandatory other + frozen partition + per-group hash で stale 判定を行う。
check-approved は latest-success per-group 判定へ置き換え、group 間 round 一致制約を廃止する。

## Review State Model

review.json schema と domain/usecase の review state model を定義する。
cycle には base_ref / policy_hash / frozen group scopes を保持し、group round は append-only とする。
stale reason ADT と latest-success 判定の入力モデルを固める。

- [x] Domain: ReviewCycle / ReviewGroupRound / ReviewStalenessReason の state model を定義
- [x] Infrastructure: review.json codec と永続化ポートを追加し、metadata.json から review state を切り離す

## Policy Freeze & Partition

track/review-scope.json を policy source として使い、named groups と mandatory other を含む partition を導出する。
review cycle 開始時に partition と policy_hash を固定し、cycle 中の drift は stale 扱いにする。
scope は path list として cycle に保存する。

- [x] Usecase/Infrastructure: track/review-scope.json の policy schema を groups 対応に拡張し、optional per-track review-groups.json override との 2 層 resolution で policy_hash と group partition を導出する
- [x] Usecase: mandatory other と cycle freeze を実装し、scope drift を stale 扱いにする。base policy 変更は policy_changed、per-track groups override 変更は partition_changed として区別する

## Append-Only Recording

record-round を review.json append-only 更新へ置き換える。
group ごとの fast/final 履歴を保持し、zero_findings 済み group は他 group の round 進行に巻き込まない。
per-group hash を cycle 開始時に frozen した group scope/base_ref/policy_hash で計算・保存する。

- [x] Usecase/Infrastructure: record-round を review.json append-only 更新 + per-group hash 記録へ置換

## Approval Guard Rewrite

check-approved を各 group の latest successful fast/final round ベースに作り直す。
global round synchronization を廃止し、self scope の hash mismatch のみで stale と判定する。
final は全 group 必須とする。

- [x] Usecase: check-approved を latest-success per-group fast/final 判定へ再設計し、global round sync を廃止

## CLI, Cleanup & Verification

CLI status 表示・stale reason 表示・テスト群を review.json 前提へ更新する。
metadata.json から review state を除去し、関連 codec / fixture / docs を整理する。
cargo make ci で回帰確認する。

- [x] CLI: review status / stale reason / final requirement 表示を review.json ベースへ更新
- [x] Verification: codec/usecase/CLI 回帰テスト追加 + cargo make ci で全通確認
