---
adr_id: 2026-05-01-0702-free-function-l2-source-form-evaluator
decisions:
  - id: D1
    user_decision_ref: "chat_segment:free-function-l2-evaluator-fix:2026-05-01"
    candidate_selection: "from:[A-catalogue-stripped,B-lazy-normalization,C-evaluator-source-form] chose:C-evaluator-source-form"
    status: proposed
---
# Free Function L2 Evaluator: returns 比較を source form に統一する

## Context

TDDD 型カタログ (`<layer>-types.json`) の `FreeFunction` エントリは `expected_returns` フィールドに戻り値の型を `Vec<String>` として保持する。カタログ著者は Rust ソースコードを読みながら宣言を記述するため、`"Result<(), TrackResolutionError>"` や `"Option<String>"` のような **source form** で書くのが自然な表記であり、実際に HEAD の `usecase-types.json` および `infrastructure-types.json` 内の自由関数エントリはすべて source form で宣言されている。

一方、L2 forward check を担う `evaluate_free_function` は `FunctionNode::returns()` を参照して比較を行う。`FunctionNode` は `code_profile_builder.rs` で `fi.return_type_names()` を使って構築される。`return_type_names()` は内部で `collect_type_names` を呼び出し、`Result<T, E>` と `Option<T>` を剥いで内側の型名のみを取り出す **stripped form** (`["TrackResolutionError"]` / `["String"]` など) を返す。

この結果、カタログ著者が source form で宣言すると L2 forward check が常に partial match に落ち、`signal_for_forward_miss` が Yellow を返す **構造的 false positive** が発生する。

一方、`MethodDeclaration` (型/トレイトのメソッド用) はすでに `fi.returns()` (source form 単体 String) を使って構築されており、free function との非対称が存在する。

`fi.return_type_names()` は typestate 遷移の検出 (`extract_typestate_names`) で引き続き利用されているため、getter 自体を削除することはできない。

`cli-via-usecase-only-2026-04-30` トラックにおいて、usecase 4 件・infrastructure 37 件の free function で本 false positive が観測された。ユーザーが「`Result<(), Err>` のような source 形式で宣言するのが正しいのでは？」と指摘し、orchestrator がバグと分析・合意した。

## Decision

### D1

`FunctionNode::returns` を populate する際に `fi.return_type_names()` (stripped form) ではなく `fi.returns()` (source form 単体 String) を使用し、`FunctionNode` の `returns` フィールドを `Vec<String>` から `String` へ変更する、または `fi.returns()` を `vec![]` にラップして source form 単体を `Vec<String>` の唯一の要素として格納する形に変更する。

これにより、カタログ宣言の source form と L2 evaluator が比較する `FunctionNode::returns` の表現が一致し、構造的 false positive が解消される。

具体的には `code_profile_builder.rs` の `FunctionNode` 構築箇所で `fi.return_type_names().to_vec()` を `vec![fi.returns().to_string()]` に置き換え、カタログ側の `expected_returns` と同じ表現体系に揃える。

`fi.return_type_names()` の呼び出しは typestate 遷移検出 (`extract_typestate_names`) において引き続き保持する。変更対象は free function の L2 forward check パスのみ。

## Rejected Alternatives

### A — カタログ側を stripped form に統一する

カタログ著者が `["TrackResolutionError"]` のように stripped form で宣言するよう規約を変更する。

却下理由: カタログ著者の直感に逆らう。Rust ソースを読んで自然に書いた宣言が間違いになるため、新規エントリを書くたびに変換ルールを意識する必要がある。誤った form で宣言した場合の CI フィードバックが遅い (L2 check failure は Yellow、コンパイルエラーではない)。HEAD の既存エントリ (40+ 件) がすべて source form であるため、一括変換のコストも高い。

### B — 評価時の双方向 lazy 正規化 (比較時にカタログ宣言を剥く)

L2 evaluator の比較ロジックで、カタログの `expected_returns` を読んだ時点で `collect_type_names` 相当の処理を実行し stripped form に変換してから `FunctionNode::returns` と比較する。

却下理由: 評価ロジックが複雑化し、カタログ宣言の意味論が評価コードに埋め込まれる。`collect_type_names` の剥き方 (`Result`/`Option` のみ透過) はあくまで typestate 遷移検出用の関心事であり、forward check の比較基準として採用することで二つの異なる用途が混在する。将来 `collect_type_names` の挙動を typestate 用途に合わせて変更した場合、forward check の比較語義が静かに変化するリスクがある。


## Consequences

- 良い点: カタログの source form 宣言が L2 forward check で正しく評価される。構造的 false positive (Yellow) が解消される。`MethodDeclaration` と `FunctionNode` の表現体系が揃い、L2 evaluator の比較ロジックが対称になる。
- 注意点: `FunctionNode::returns()` の戻り値の語義が変わるため、その getter に依存する既存コードを調査して影響範囲を確認する必要がある。`FunctionNode` を直接参照する箇所が stripped form を前提としている場合はあわせて修正が必要。
- 中立: `fi.return_type_names()` は typestate 遷移検出のために引き続き存在する。名前と用途の乖離は生じないが、「returns との非対称な getter が 2 つある」状態は維持される。コメントで用途を明記することで混乱を防ぐ。

## Reassess When

- rustdoc の `FunctionInfo::returns()` が返す型表現の形式が変わった場合 (module path の短縮方針など)
- `collect_type_names` の stripped form が typestate 遷移検出以外の新しい用途に使われるようになった場合
- `FunctionNode` の `returns` フィールドを複数の型名 (stripped) で保持する必要がある新機能が追加された場合

## Related

- [TDDD-01: Multilayer Extension](2026-04-11-0002-tddd-multilayer-extension.md) — free function を型カタログの検証対象に含めた元の判断
- [TDDD-05: Secondary Adapter variant の追加](2026-04-15-1636-tddd-05-secondary-adapter.md) — L2 evaluator の forward/reverse check 構造の詳細
