# Spec: Security Control Tests

## Goal

他の5トラックで導入するセキュリティ強化措置の CI 回帰テストを整備し、ガードレールのサイレントな劣化を防止する。

## Scope

### In scope

- コンテナ内 .git read-only マウントの検証テスト
- コンテナ内機密ディレクトリ不可視の検証テスト
- フック fail-closed 挙動の selftest
- filelock ベースの並行書き込み検出テスト
- `cargo make ci` パイプラインへの統合

### Out of scope

- セキュリティ強化の実装自体（他の5トラックの責務）
- ペネトレーションテスト

## Constraints

- このトラックは他の5トラック完了後に実装する
- テストは `scripts/test_verify_scripts.py` に追加、または独立テストファイルとして配置
- CI テストはコンテナの起動を伴うため、実行時間に注意

## Acceptance Criteria

1. `.git` ro マウントテストが CI で実行される
2. 機密ディレクトリ不可視テストが CI で実行される
3. fail-closed フック挙動テストが CI で実行される
4. 並行ロックテストが CI で実行される
5. `cargo make ci` が全テストを含めて通過

## Resolves

- 他の5トラックの回帰テスト要件を一括カバー
