---
adr_id: "2026-08-21-0055-type-identity-fully-qualified-paths"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session_01Yak2qzt8aCZQ1dkPbuLpSd:2026-08-21 Phase 0 boundary approval of the converged text (D1 full-path identity, catalogue declarations, scope)"
    candidate_selection: "from:[full-path-identity,duplicate-rename-hotfix,duplicate-name-lint] chose:full-path-identity"
    status: proposed
---
# 型シグナル評価の型識別を完全修飾パスで行う

## Context

型シグナル評価器は、catalogue に宣言した型契約と、rustdoc が出力する実コードの JSON を突き合わせる。そのため、両者で同じ型・トレイトを同じ識別キーで表せなければならない。本 ADR でいう完全修飾パスは、モジュールパスと名前を合わせた識別子である。

現在の短名依存は、パスを取得できないときだけの退避経路に限られない。型・トレイトの identity map、名前から id への対応、型参照の解決、impl の self 型とトレイト、generic 引数の構造比較、表示用 contract-map の node 解決が、モジュールを含まない名前を識別に使っている。`crate::a::Input` と `crate::b::Input` のような別の型は、短名ではともに `Input` となるため、対応表では一方だけが残り、参照先を失った項目が `DanglingId` になる。

catalogue も現在は短名を top-level key にしており、同じ名前の型を 1 つの catalogue に併記できない。各 entry の `module_path` はあっても、現在の codec と local resolver はそれを識別に用いていない。したがって、この問題は評価器の一つの fallback だけを直して解消できるものではない。

compiler-internal trait は既に突合前に既存の allowlist で除外され、auto trait の合成 impl も synthetic として除外される。この allowlist は短名で照合するためのものではない。また、`paths` を得られない impl trait path の短名 fallback は存在するものの、catalogue 側は external・local とも `paths` entry を作るため、実入力でこの経路が使われる証拠は確認できていない。

同名による衝突は、generic 引数でも既知の偽陽性として記録されている。一方、今回の module collision については、保存済みの観測ログから再現条件を確定できていない。したがって、catalogue の内容によらず決定論的に再現するとまでは主張しない。

## Decision

### D1: 識別キーを完全修飾パス（モジュールパス + 名前）にする

型・トレイトを識別する下記の経路では、rustdoc の `paths` を権威とする完全修飾パスをキーに用いる。短名は表示や入力の表記として残り得ても、型・トレイトの識別を決める権威にはしない。

catalogue における型・トレイトの宣言は短名を既定とし、完全修飾パスを任意に併記できるようにする。評価器はこの宣言を完全修飾パスへ解決してから識別に用いる。短名の宣言が複数候補に一致し、完全修飾パスの併記もなく文脈から一意に決められない場合は、候補の完全修飾パスを列挙して fail-closed とする。利用者は完全修飾パスの併記によって曖昧さを解消できる。同一 crate 内の同名型・同名トレイトを 1 つの catalogue で同時に表現できなければならないが、その宣言形式の具体設計はこの ADR では固定しない。

完全修飾パスを解決できない場合は、解決できなかった対象を診断メッセージで示して fail-closed とする。短い名前だけのキーへ暗黙に戻してはならない。raw rustdoc の `paths` はすでに baseline に保存されているため、この決定は rustdoc の wire format を変更するものではない。

#### Scope

ここでいう TypeRef は、catalogue 内で別の型またはトレイトを参照する記法である。in-crate 参照は同じ crate 内の対象を参照する TypeRef をいう。

この決定で変更する面は次のとおりである。

- catalogue の `types` / `traits` の top-level key。短名だけでは同名を併存できないため、短名既定・任意の完全修飾パス併記・曖昧時の fail-closed に対応する。
- `trait_impls` の `trait_ref` と `for_type`、`inherent_impls` の `type_name`、および in-crate TypeRef 全般。後者には `fields[].ty`、`params[].ty`、`returns`、`bounds`、`where_predicates`、`supertrait_bounds`、`type_alias.target`、variant payload、role payload を含める。
- 評価器の型・トレイト identity map、名前から id への対応、型参照の解決、impl の self 型とトレイト、generic 引数の構造比較、型シグナル出力の owner 結合、contract-map の node / edge 解決。
- catalogue-lint の `ReferencedRoleConstraint`、`FieldElementUniqueAcrossEntries`、`NoExternalReferenceInMethods`。いずれも現在は module を照合せず、同名を同一視し得る。
- test-obligation derive の trait role index、task-contract と pre-review gate の `CatalogueEntryKey`、および `sotp catalog add` / `cite` と `import` が書き込む entry key。これらは短名を前提にしているため、完全修飾パスへの解決と同名併存に追従させる。

次の面は変更しない。`functions` の key と cross-crate 参照の catalogue 表記はすでに完全修飾パスであり、`sotp catalog import --type` の rustdoc 側照合も crate・module・name の完全一致で短名 fallback を持たない。baseline-graph renderer は rustdoc の `paths` を基準にし、`catalog check` と catalogue-spec refs は section と entry key の hash を検査して TypeRef を解決しない。raw rustdoc baseline の wire format も変更しない。

renderer の表示ラベルと通常の診断メッセージは表示のみなので短名のまま残す。ただし曖昧な短名で fail-closed するときは候補の完全修飾パスを示す。catalogue-lint の `KindLayerConstraint`、test-obligation の anchor、`dup-check` と `find-similar`、spec 側からの catalogue 逆参照も、型名または TypeRef を解決しないため対象外とする。

test-obligation の trait role index は現在 `(crate_name, TraitName)` で module を持たず、同じ crate 名の catalogue が複数あれば first-match を取る。この状況が実運用で到達するかは未確認であり、ここでは不確実性として記録する。impl identity にある外部 workspace crate の同名 trait を区別する部分的な完全修飾パス化は、上記の評価器 identity 面に含め、この ADR とは別の決定を設けない。

## Rejected Alternatives

- **重複側の改名（応急）**: 封鎖は解けるが、合法なコード形状が評価器の都合で禁止されたままになり、次の重複で再発する。
- **同一 crate 内の重複型名を lint で禁止する**: 現在の catalogue が短名だけの宣言では同名を表せないという既存の制約を明示する方法ではある。しかし、完全修飾パスを識別の基準にし、必要時にそれを併記できる選択では、合法な同名型・同名トレイトを catalogue の受理対象から除外せずに表現できるため、この制約を恒久的な方針にはしない。
- **短名 identity の維持 + 衝突検出時のみエラー**: 短名への縮退を保ったままでは、異なる対象を同じ key にしてからエラーにするだけである。すべての identity 面を完全修飾パスへ移し、衝突そのものを表現可能にする。

## Consequences

- 良: 同名の型・トレイトが複数あっても識別が一意になり、catalogue でもその区別を表現できる。短名を既定の宣言として保ちつつ、曖昧な場合だけ完全修飾パスを併記できる。generic 引数で観測された短名の偽陽性衝突も同時に解消される。
- 負: catalogue が短名既定と任意の完全修飾パス併記を受理し、それを完全修飾パスへ解決できるように、codec、評価器の identity 面、型シグナル出力の結合、contract-map、catalogue-lint、test-obligation、task-contract と pre-review gate、書き込み側 command、既存のテストと fixture を連動して改訂する必要がある。単一の fallback 修正では完結しないため、複数の独立した変更単位に分けて段階的に実施する。
- 中立: 問題のきっかけになった重複型名は、合法な Rust としてそのまま残してよい。本修正後は識別の問題にならない。

## Reassess When

- rustdoc JSON の `paths` の意味論（完全パスの供給保証）が変わったとき。
- catalogue または rustdoc が、完全修飾パスを型・トレイトの安定した識別子として提供できなくなったとき。
