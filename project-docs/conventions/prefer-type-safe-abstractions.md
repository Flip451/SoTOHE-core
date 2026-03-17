# Prefer Type-Safe Abstractions Convention

## Rule

バグパターンが発見されたとき、lint ルールや convention doc で「やってはいけない」を追加するのではなく、そのバグクラスを型システムや標準ライブラリで根本的に排除する方法を優先すること。

## Rationale

- **Lint ルールは破られる**: convention doc やメモリに記録されたルールは忘れられる。CI ルールも例外追加で形骸化する。
- **型は破れない**: コンパイラが強制する制約は全開発者と全 AI エージェントに等しく適用される。
- **AI エージェントの傾向**: AI は「手書きコード + ルールで防止」に走りがちだが、「標準ライブラリで問題クラスを排除」が正しい選択肢であることが多い。

## Decision Flow

バグパターン発見時:

1. **標準ライブラリ / 既存 crate で排除可能か?**
   - `serde` typed deserialization → `serde_json::Value` 手動走査の排除
   - `syn` AST パース → 行ベースヒューリスティックの排除
   - `conch-parser` → hand-rolled shell tokenizer の排除
   - **可能なら採用** (最優先)

2. **型で表現可能か?**
   - Newtype pattern で不正値を構築不能にする
   - `enum` で状態遷移を限定する
   - **可能なら採用**

3. **上記で対応不可能な場合のみ**:
   - CI lint (`architecture-rules.json`, clippy)
   - Convention doc
   - メモリ / behavioral rule (最後の手段)

## Examples

| バグパターン | Bad (ルールで防止) | Good (型で排除) |
|---|---|---|
| JSON の不正データ | `filter_map` 禁止ルール | `#[derive(Deserialize)]` |
| テストモジュール誤判定 | 行ベース heuristic 改善 | `syn` crate で AST パース |
| Shell コマンド解析漏れ | hand-rolled parser 修正 | `conch-parser` AST 走査 |
| 不正な Email 値 | バリデーション関数 | `Email` newtype |
