# verification: usecase-purity-ci (INF-15)

## Scope Verified

- spec.md の acceptance criteria 1-4 をカバー

## Manual Verification Steps

1. `bin/sotp verify usecase-purity` が warning ゼロで pass すること
2. テスト入力で禁止パターン（std::fs::, chrono::Utc::now, println!）が検出されること
3. `cargo make ci` が全て pass すること
4. `#[cfg(test)]` ブロック内の禁止パターンは無視されること

## Result / Open Issues

- (未実施)

## Verified At

- (未実施)
