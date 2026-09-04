---
adr_id: "2026-08-25-0804-post-fq-identity-regression-repair"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:claude-session-017jQbkBNQHUjkxck4R84QHp:2026-08-25 Phase 0 boundary final approval of the converged text (D1 add-type resolution set, D2 root-alias canonicalization, D3 module_path omission rule; route set closed as call sites)"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:claude-session-017jQbkBNQHUjkxck4R84QHp:2026-08-25 Phase 0 boundary final approval of the converged text (D1 add-type resolution set, D2 root-alias canonicalization, D3 module_path omission rule; route set closed as call sites)"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:claude-session-017jQbkBNQHUjkxck4R84QHp:2026-08-25 Phase 0 boundary final approval of the converged text (D1 add-type resolution set, D2 root-alias canonicalization, D3 module_path omission rule; route set closed as call sites)"
    status: proposed
---
# 完全修飾識別の 2 リグレッション(実装前 add 型・bin root 別名)を修復する

## Context

完全修飾パス識別の導入後、2 系統のリグレッションが確認された。評価器は評価のたびに、baseline と current の rustdoc `paths` 表を合成して「解決集合」を作り、catalogue の参照をこれに照合する。

**(1) 実装前の add 型が encode できない。** 解決集合は rustdoc 由来のパスだけから作られ、集合に無い参照は即エラーになる。先に宣言し後で実装する流れでは、add 型は宣言時点で rustdoc に無いのが正常なのに、新しい型を宣言する作業がすべて止まる。テストは catalogue の宣言を解決集合に足しており、本番と異なる集合を使っていたため検出できなかった。解決集合の実体は Id を key とする rustdoc summary の表で、パスの所属判定と Id の逆引きの両方に使われる。そのため add 型を受け入れるには、catalogue の宣言から合成した summary も表に入れる必要がある。

add 型の扱いは既に経路ごとにばらばらである。type-signal identity index の経路は黙認し、deletion の経路は生の名前にフォールバックし、Phase 1 の定義パス authority は経路固有のフォールバックを持つ一方、codec の経路だけが fail-closed で停止する。

**(2) bin ターゲット crate の型エントリが解決できない。** `apps/cli` は package 名 `cli`、bin 名 `sotp` なので、rustdoc は `sotp::…`、catalogue は `cli::…` を使う。読み替え（別名）の仕組みは関数と import の 2 箇所にしか適用されていない。

現在の `module_path` の省略時既定値は crate root である。しかし、これは catalogue 作成者の負担を減らすために field を optional にした本来の意図と矛盾し、省略しても実際には配置を選ばせている。

いずれも短名識別の時代は「名前しか比較しない」ことで偶然吸収されていた潜在的不整合であり、完全修飾化が顕在化させた。元 ADR とそれ以前の記録には、これらを既知制限として扱ったものは無い。

## Decision

### D1: catalogue が宣言する add 型を解決集合に加える

解決集合の構築は 1 箇所で行い、rustdoc 由来のパスに加えて catalogue が add 宣言した型を、宣言から合成した summary として入れる。解決集合を参照するすべての経路はこの 1 箇所の結果を使い、経路ごとの add 特例（黙認・生の名前へのフォールバック・経路固有のフォールバック）は廃止する。ここでいう経路とは解決集合を参照する呼び出し箇所であり、コード上で列挙できる閉集合である（現時点では codec、type-signal identity index、deletion 処理、Phase 1 の定義パス authority の 4 箇所）。

合成 summary の module は D3 に従う（`module_path` があれば crate + module_path + name、未指定かつ未実装なら crate + name で module 未確定。crate root としては扱わない）。

回帰テスト: add 型同士の相互参照、modify 型から add 型への参照、宣言順序への非依存。rustdoc にも catalogue にも無い参照が fail-closed で拒否される挙動は不変。

### D2: root 別名の適用を解決集合の正準化に一元化する

`cli`↔`sotp` の読み替えは、解決集合を参照して型の名前を正式な形に整える段階で 1 回だけ行い、すべての解決経路がそこを通る。関数 identity と catalog import に散っている個別の読み替えは廃止する。経路の集合は D1 と同じく呼び出し箇所として閉じている（現時点では D1 の 4 箇所に関数 identity と catalog import を加えた 6 箇所）。回帰テスト: bin ターゲット crate の型エントリ(add / modify / reference)と関数パスの双方が解決されること。

### D3: module_path 省略時の解決規則

`module_path` の省略は crate root を意味せず、配置未指定を意味する（配置は解決規則により rustdoc から得る）。明示した `module_path` または修飾 key は、従来どおり完全一致で照合する。

同名比較は catalogue 自身の crate 内で namespace を分け、型は型、trait は trait と照合する。`add` は baseline に同名があれば fail-closed とする。baseline に無ければ、current に同名が 1 つなら実装済みとしてその path を identity とし、複数なら曖昧として fail-closed として `module_path` または修飾 key を要求し、無ければ未実装として crate と名前を identity としモジュールは未確定とする。`modify` / `delete` / `reference` は baseline に同名が 1 つならそこで解決して current と突合し、複数または無ければ fail-closed とする。

## Rejected Alternatives

- **root 型の解決経路にだけ別名適用を追加する**: 症状は消えるが、別名の知識が 3 箇所目になり、次の解決経路の追加で同じ漏れが再発する。単一通過点の原則に反する。
- **codec テストのフィクスチャ補完(カタログ由来パスの注入)を仕様と見なす**: テストが本番と異なる解決集合を組み立てて欠陥を隠すため、拒否する。catalogue 由来の合成 summary は D1 の解決集合から本番側へ供給し、テストも同じ構築を経由する。
- **catalogue 側で rustdoc root 名(`sotp::…`)を書かせる**: 宣言者に rustdoc の内部事情を要求し、crate 名で書くという catalogue の自然な語彙を壊す。

## Consequences

- 良: 宣言先行の流れが回復し、bin ターゲット crate の型評価も全経路で回復する。別名適用漏れと、経路別の add 型特例の欠陥クラスが構造的に消える。
- 良: catalogue 作成者は配置を選ばずに `module_path` を省略でき、配置は rustdoc に同名が 1 つあるときに得られる。

## Reassess When

- 1 crate に複数 bin ターゲットが定義され、root 別名が 1:1 でなくなったとき。

## Related

- `knowledge/adr/2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` D7 — 同 ADR D7 の entry struct 定義に付随する「module_path は省略可、空 = crate root」という既定を、D3 が省略時の解決規則として refine する。
