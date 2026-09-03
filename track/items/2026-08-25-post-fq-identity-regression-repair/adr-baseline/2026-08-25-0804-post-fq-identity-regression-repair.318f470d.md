---
adr_id: "2026-08-25-0804-post-fq-identity-regression-repair"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:claude-session-01KjrLiixdHPZAezqFdVwGSS:2026-08-25 pr251-regression adjudication(lane D 提案採用)"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:claude-session-01KjrLiixdHPZAezqFdVwGSS:2026-08-25 pr251-regression adjudication"
    status: proposed
---
# 完全修飾識別の 2 リグレッション(実装前 add 型・bin root 別名)を修復する

## Context

完全修飾パス識別の導入後、2 系統のリグレッションが確認された。

**(1) 実装前の add 型が encode できない。** identity universe の実体は Id を key とする rustdoc summary map であり、canonical path の所属判定だけでなく Id の逆引きにも使われる。したがって canonical identity のパスを universe に含めるだけでは add 型を受け入れられず、catalogue が宣言する add 型に対応する合成 summary も供給する必要がある。しかし宣言が先・実装が後という本テンプレートの根幹の流れでは、add 型は宣言時点で rustdoc に存在しないのが正常であり、新しい型を宣言する全 track が type-design で停止する。codec テストは catalogue 由来のパスをフィクスチャに補っていたため検出しなかった。

add 型の扱いは既に経路ごとに不整合である。type-signal identity index の経路は未解決の add 型を黙認し、deletion の経路は raw key にフォールバックし、Phase 1 の定義パス authority は codec 出力を経路固有の fallback として持つ一方、codec の経路だけが fail-closed で停止する。これは (2) と同じ、経路別の特例による欠陥である。

**(2) bin ターゲット crate の型エントリが解決できない。**bin ターゲットの rustdoc は crate 名ではなく bin 名を root に用いる（catalogue の crate は `cli`、rustdoc root は `sotp`）。既存の別名機構（`rustdoc_root_name`）は関数 identity と catalog import には適用されているが、型エントリの解決経路には適用されず、add に限らず modify / reference でも fail する。

いずれも短名識別の時代は「名前しか比較しない」ことで偶然吸収されていた潜在的不整合であり、完全修飾化が顕在化させた。元 ADR と当該 track の observations に既知制限としての記録は無い。

## Decision

### D1: catalogue が宣言する add 型を identity universe に加える

identity universe の構築を 1 箇所に一元化し、rustdoc 由来の summary と、catalogue が add として宣言する型の catalogue 由来の合成 summary を供給する。これにより canonical path の所属判定と Id の逆引きの双方が add 型を認識する。codec、type-signal identity index、deletion 処理、Phase 1 の定義パス authority はすべてこの単一の構築結果を利用し、未解決 add 型の黙認、raw key へのフォールバック、経路固有の fallback といった add 型の特例を廃止する。

回帰テストとして、add 型同士の相互参照（field の型が別の add 型を指すこと）、modify 型から add 型への参照、宣言順序への非依存を固定する。rustdoc にも catalogue にも無い参照が fail-closed で拒否される挙動は不変。

### D2: root 別名の適用を identity 解決の正準化に一元化する

`rustdoc_root_name` による catalogue crate 名 ↔ rustdoc root 名の別名解決を、identity の正準化（canonical identity の構築）の一段として実装し、root 型・関数パス・その他の解決経路がすべて同じ別名適用を通るようにする。経路ごとの個別適用（関数 identity と catalog import のみの現状の形）を廃し、別名の知識を持つ場所を 1 箇所にする。回帰テストとして「bin ターゲット crate の型エントリ(add / modify / reference)と関数パスの双方が解決される」ケースを固定し、すべての解決経路が同じ正準化の段階を通ることで、別名の知識がコード上でも 1 箇所に保たれることを検証する。

## Rejected Alternatives

- **root 型の解決経路にだけ別名適用を追加する**: 症状は消えるが、別名の知識が 3 箇所目になり、次の解決経路の追加で同じ漏れが再発する。単一通過点の原則に反する。
- **codec テストのフィクスチャ補完(カタログ由来パスの注入)を仕様と見なす**: 拒否するのは、テストが本番と異なる universe を組み立てて欠陥を隠す構図である。catalogue 由来の合成 summary を単一の universe 構築から本番側へ供給することは D1 の正規手段であり、テストもその同じ構築を経由する。
- **catalogue 側で rustdoc root 名(`sotp::…`)を書かせる**: 宣言者に rustdoc の内部事情を要求し、crate 名で書くという catalogue の自然な語彙を壊す。

## Consequences

- 良: 宣言先行の流れが全 track で回復し、bin ターゲット crate の型評価も全経路で回復する。別名適用漏れと、経路別の add 型特例の欠陥クラスが構造的に消える。
- 中立: 影響を受けた track は develop 取り込みと `bin/sotp` 再ビルドの後、type-design から再入場する（各 track の再開手順は当該 track の briefing 側に記録済み）。

## Reassess When

- 1 crate に複数 bin ターゲットが定義され、root 別名が 1:1 でなくなったとき。
