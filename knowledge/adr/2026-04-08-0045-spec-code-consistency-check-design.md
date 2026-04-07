# 3-12 spec ↔ code 整合性チェック — TypeGraph + 既知課題の解決

## Status

Proposed

## Context

`spec-domain-types-v2-2026-04-07` track で domain-types.json の分離と DomainTypeKind 5 カテゴリの型定義・評価ロジックが完成した。しかし実装過程で以下の課題が発見され、PR review で繰り返し指摘された。

### 前提: 完了済みの基盤

| 基盤 | 場所 | 内容 |
|------|------|------|
| DomainTypeKind | `libs/domain/src/domain_types.rs` | Typestate/Enum/ValueObject/ErrorType/TraitPort の 5 カテゴリ |
| TypestateTransitions | 同上 | Terminal / To(targets) — 空 Vec の暗黙的 terminal を排除 |
| CodeProfile | `libs/domain/src/schema.rs` | HashMap<String, CodeType> + HashMap<String, CodeTrait> — pre-indexed 評価インターフェース |
| build_code_profile | `libs/infrastructure/src/code_profile_builder.rs` | SchemaExport → CodeProfile 変換 |
| evaluate_domain_type_signals | `libs/domain/src/domain_types.rs` | CodeProfile ベースの Blue/Red 2 値評価 |
| FunctionInfo.return_type_names | `libs/domain/src/schema.rs` | rustdoc JSON から再帰的に抽出した戻り値型名 |
| FunctionInfo.has_self_receiver | 同上 | associated function を遷移候補から除外 |
| domain-types.json codec | `libs/infrastructure/src/domain_types_codec.rs` | serde tagged enum (kind フィールド) |
| domain-types.md renderer | `libs/infrastructure/src/domain_types_render.rs` | kind ごとの Details 列 + (name,kind) ペアで signal 照合 |
| verify spec-states | `libs/infrastructure/src/verify/spec_states.rs` | Stage 1 prerequisite + Blue-only gate + name+kind coverage |
| domain-type-signals CLI | `apps/cli/src/commands/track/domain_state_signals.rs` | RustdocSchemaExporter → CodeProfile → evaluate |

### 課題 1: collect_type_names が全 generic args を展開する

`libs/infrastructure/src/schema_export.rs` の `collect_type_names()` は `rustdoc_types::Type` を再帰的に走査し、全ての `ResolvedPath` の generic arguments を展開する。そのため `Vec<Published>` や `HashMap<K, Published>` の `Published` も `return_type_names` に含まれてしまう。

**影響**: typestate 遷移検出で `fn get_history(&self) -> Vec<Published>` が `Draft → Published` 遷移として誤検出される可能性。

**対策**: `collect_type_names` を `Result<T, E>` と `Option<T>` のみに制限する。他の generic wrapper (Vec, HashMap, Box, Arc 等) は展開しない。

### 課題 2: 同名型の衝突

`build_code_profile` は型名の最後のセグメント (bare name) でインデックスする。異なるモジュールに同名の pub 型がある場合、後から処理された型が上書きし、遷移データが混ざる。

**影響**: `domain::review::Error` と `domain::guard::Error` が衝突。

**対策**: CodeProfile のキーを module path 付きにするか、TypeGraph で full path を保持する。

### 課題 3: CodeProfile → TypeGraph への発展

CodeProfile は「フラットなインデックス」であり、型間の関係をグラフとして表現していない。以下の機能が未実装:

- **到達可能性**: `Draft` から `Published` → `Archived` の遷移経路が spec 上で宣言されているが、コード上で実現可能かの検証
- **孤立ノード検出**: どの typestate からも到達不能な typestate の検出
- **サイクル検出**: typestate 間の意図しない循環の検出
- **逆方向チェック** (3-12 本体): コードに存在するが spec に未宣言の型の検出

### 課題 4: spec ↔ code 双方向整合性チェック CLI

3-12 の本体機能。`sotp verify spec-code-consistency` コマンドを新設し:
- 前方向: domain-types.json の各エントリがコードに存在するか (既存の evaluate で対応済み)
- 逆方向: コードの pub 型が domain-types.json に宣言されているか (未実装)
- 構造: variant 名、メソッド名の完全一致 (既存の evaluate で対応済み)

## Decision

### 1. collect_type_names を Result/Option のみに制限

`libs/infrastructure/src/schema_export.rs` の `collect_type_names` を修正:

```rust
fn collect_type_names(ty: &rustdoc_types::Type, out: &mut Vec<String>) {
    match ty {
        rustdoc_types::Type::ResolvedPath(p) => {
            let name = last_path_segment(&p.path);
            match name.as_str() {
                "Result" | "Option" => {
                    // Unwrap first generic argument only
                    if let Some(args) = &p.args {
                        if let rustdoc_types::GenericArgs::AngleBracketed(ab) = args.as_ref() {
                            if let Some(rustdoc_types::GenericArg::Type(inner)) = ab.args.first() {
                                collect_type_names(inner, out);
                            }
                        }
                    }
                }
                _ => {
                    // Non-wrapper type — add to results, do NOT recurse into generics
                    out.push(name);
                }
            }
        }
        rustdoc_types::Type::BorrowedRef { type_: inner, .. } => {
            collect_type_names(inner, out);
        }
        _ => {}
    }
}
```

これにより `Vec<Published>` → `["Vec"]` (Published は展開されない), `Result<Published, Error>` → `["Published"]` (Result は unwrap される) となる。

### 2. CodeProfile を TypeGraph にリネーム・拡張

```rust
pub struct TypeGraph {
    types: HashMap<String, TypeNode>,
    traits: HashMap<String, TraitNode>,
}

pub struct TypeNode {
    kind: TypeKind,
    members: Vec<String>,
    /// Outgoing typestate transitions (self-method return types filtered to typestate types)
    outgoing: HashSet<String>,
    /// Module path for disambiguation (e.g., "domain::review")
    module_path: Option<String>,
}

pub struct TraitNode {
    method_names: Vec<String>,
}
```

`outgoing` は `method_return_types` のうち typestate 型のみに絞り込んだもの。`build_code_profile` → `build_type_graph` に改名し、typestate フィルタを graph 構築時に適用する (現在は評価時にフィルタ)。

### 3. module_path による同名型の区別

`SchemaExport` の `TypeInfo` に `module_path: Option<String>` を追加。rustdoc JSON には各 item の path 情報があるので、`build_schema_export` で抽出可能。

`TypeGraph` のキーは引き続き bare name だが、`TypeNode.module_path` で衝突を検出し warning を出す。domain crate 内で同名 pub 型がある場合は domain-types.json で `module_path` を明示指定できるように `DomainTypeEntry` を拡張する (将来)。

### 4. 逆方向チェック CLI

`sotp verify spec-code-consistency --track-id <id> --crate <name>`:
1. domain-types.json を読み込む
2. SchemaExport → TypeGraph を構築
3. 前方向: evaluate_domain_type_signals (既存)
4. **逆方向**: TypeGraph の全ノードのうち、domain-types.json に宣言がないものを報告
5. 結果を `ConsistencyReport` として出力

`ConsistencyReport`:
```rust
pub struct ConsistencyReport {
    forward_signals: Vec<DomainTypeSignal>,  // 既存
    undeclared_types: Vec<String>,           // 逆方向: code にあるが spec にない
    undeclared_traits: Vec<String>,          // 逆方向: trait
}
```

### 5. approved フィールドと Yellow の将来

ADR `2026-04-07-0045-domain-types-separation.md` の Reassess When に記載済み:
逆方向チェックで未宣言の型が見つかった場合、AI が自動で `approved: false` のエントリを domain-types.json に追加するフローが実装された時点で、Yellow = 「AI 自動追加・人間未承認」として再導入を検討する。

## Critical Bug: Legacy spec.md Domain States Regression

### 問題

`track-sync-views` は全トラックの spec.md を再生成する。新しいレンダラーは spec.json の `domain_states` フィールドを読み取らないため、旧トラックの spec.md から Domain States セクションと Stage 2 シグナルが消失する。

これはレビューアが繰り返し指摘していた「legacy fallback」問題そのもの。当時「旧トラックマイグレーション不要」と判断したが、**views の再生成が全トラックに影響する** ことを見落としていた。

### 根本原因

`track-sync-views` (`sotp track views sync`) が**全トラック**の spec.md / plan.md を再生成する設計になっている。完了済み・アーカイブ済みトラックの views を新しいレンダラーで再生成すると、旧フォーマットのデータが消失する。

### 必要な修正 (最優先)

**`track-sync-views` をアクティブトラック + registry.md のみに制限する。**

具体的には `sotp track views sync` の実装 (`libs/infrastructure/src/track/render.rs` の `sync_rendered_views`) を修正し:
- `metadata.json` の `status` が `done` または `archived` のトラックはスキップする
- `registry.md` は常に再生成する
- これにより完了済みトラックの spec.md が新しいレンダラーで上書きされることを防ぐ

spec.json codec に legacy decode を戻す必要はない。問題は再生成の対象範囲。

### 教訓

レビューアの指摘を「旧トラックはアクティブにしない限り影響しない」と dismissed したが、`track-sync-views` が全トラックに影響するパスであることを見落としていた。レビューアの懸念は正当だった。

## Implementation Order

0. **[最優先] track-sync-views をアクティブトラック + registry.md に制限** (infra, S 難易度) — done/archived トラックの views 再生成をスキップ
1. **collect_type_names 修正** (infra, S 難易度) — Result/Option 以外の generic 展開を停止
2. **TypeGraph リネーム + outgoing フィールド** (domain, M 難易度) — CodeProfile → TypeGraph, typestate フィルタを構築時に移動
3. **module_path 追加** (domain + infra, S 難易度) — TypeInfo と TypeNode に追加
4. **逆方向チェック** (domain + CLI, M 難易度) — undeclared types/traits 検出 + CLI コマンド
5. **ConsistencyReport + verify 統合** (CLI, S 難易度) — sotp verify spec-code-consistency

## Key Files

| File | Role | Changes |
|------|------|---------|
| `libs/infrastructure/src/schema_export.rs` | rustdoc JSON → SchemaExport | collect_type_names 修正, module_path 抽出 |
| `libs/domain/src/schema.rs` | CodeProfile → TypeGraph | リネーム, outgoing フィールド, module_path |
| `libs/infrastructure/src/code_profile_builder.rs` | build_code_profile → build_type_graph | typestate フィルタ移動, module_path 設定 |
| `libs/domain/src/domain_types.rs` | evaluate_domain_type_signals | TypeGraph 使用, typestate_names 引数削除 |
| `apps/cli/src/commands/track/domain_state_signals.rs` | CLI | TypeGraph 使用 |
| `apps/cli/src/commands/verify.rs` | CLI | spec-code-consistency サブコマンド追加 |

## Rejected Alternatives

- **syn ベースの遷移検出を復活**: rustdoc JSON が return type の構造化データを持つ。syn は冗長
- **CodeProfile を残して TypeGraph を別に追加**: 二重管理。リネーム+拡張の方がシンプル
- **逆方向チェックを信号機に統合**: 信号機は前方向 (spec → code) 専用。逆方向は別の ConsistencyReport

## Consequences

- Good: collect_type_names の false positive が解消。`Vec<Published>` が遷移として誤検出されない
- Good: TypeGraph で到達可能性・サイクル検出が将来可能に
- Good: 逆方向チェックで「spec に書き忘れた型」を CI で検出
- Bad: TypeGraph への移行で CodeProfile を使う全コードの更新が必要 (影響範囲: ~17%)
- Bad: module_path は rustdoc JSON の path 構造に依存 (nightly バージョン間で変わる可能性)

## Reassess When

- TypeGraph のグラフアルゴリズム (到達可能性, サイクル検出) が実際に必要になった時点で、petgraph crate の導入を検討
- module_path の衝突が実運用で頻発する場合、domain-types.json に module_path 指定フィールドを追加
- 逆方向チェックの auto-add (AI が未宣言型を自動追加) が実装された時点で Yellow 再導入を検討
