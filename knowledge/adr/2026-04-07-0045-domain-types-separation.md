---
adr_id: 2026-04-07-0045-domain-types-separation
decisions:
  - id: 2026-04-07-0045-domain-types-separation_grandfathered
    status: accepted
    grandfathered: true
---
# domain-types.json: Typed Domain Type Registry Separated from spec.json

## Status

Accepted

## Context

`spec.json` の `domain_states` フィールドには 3 つの設計上の問題がある:

1. **ライフサイクルの混在**: spec (要件) は承認後に凍結されるべきだが、domain_states (型宣言) は実装の進捗に応じて更新される。1 ファイルに混在すると content hash が不必要に無効化される。

2. **型カテゴリの欠如**: `DomainStateEntry` は `name + description + transitions_to` のみ。typestate 型、enum 型、値オブジェクト、エラー型、trait port の区別がない。`transitions_to: []` が「終端状態」と「遷移の概念がない型」の両方を意味する。

3. **信号の曖昧さ**: `transitions_to: None` (未宣言) が Yellow 信号を生む。spec に型を宣言したなら遷移も宣言すべきであり、未宣言は Red であるべき。

### 背景

- `domain_states` は Phase 2 (2-2, SPEC-05) で導入。当時は typestate パターンの状態型検証を目的としていた
- 実運用では `SchemaExport` (bridge01-export-schema-2026-04-06) などで全 pub 型を `transitions_to: []` として登録する運用が定着
- Phase 3 (3-12) の spec ↔ code 整合性チェックには、型カテゴリ別の検証データが必要
- `SchemaExport` の `TypeInfo.members` (variant 名) と突合するためには、spec 側にも `expected_variants` が必要

## Decision

### 1. domain-types.json として spec.json から分離

`spec.json` の `domain_states` と `domain_state_signals` を削除し、`domain-types.json` を track ごとの独立ファイルとして新設する。rendered view は `domain-types.md`。

### 2. DomainTypeEntry に approved フィールドを事前追加

`DomainTypeEntry` に `approved: bool` (デフォルト `true`) を追加する。手動で書いたエントリは `true`、将来の AI 自動追加は `false` を設定する。この track では `approved` による信号分岐は実装しない (Yellow 再導入は Reassess When に記載)。スキーマを事前に整えることで、後続 track でのスキーマ変更を回避する。

### 3. DomainTypeKind enum で型カテゴリを表現 (enum-first)

5 カテゴリを定義し、各 variant が固有の検証データのみを保持する:

| kind | 検証データ | 検証内容 |
|------|----------|---------|
| `typestate` | `transitions_to: Vec<String>` | 遷移関数の存在 |
| `enum` | `expected_variants: Vec<String>` | variant 名の双方向一致 |
| `value_object` | (なし) | 型の存在のみ |
| `error_type` | `expected_variants: Vec<String>` | variant カバレッジ (spec→code) |
| `trait_port` | `expected_methods: Vec<String>` | メソッド名の存在 |

これは 04-coding-principles.md の enum-first パターンに準拠: variant ごとに異なるデータを持つ場合は enum を使う。

### 3. Blue/Red 2 値信号 (Yellow 廃止)

Stage 2 (domain type verification) の信号を Blue/Red の 2 値に厳格化する:

- **Blue**: spec と code が完全一致
- **Red**: それ以外全て (型なし、不一致、未宣言)

Yellow は Stage 1 (spec source tag signals) でのみ使用。Stage 2 では「spec に書いたなら完全に書け」を強制する。

## Rejected Alternatives

- **spec.json 内で domain_states → domain_types に置換**: ライフサイクルの混在が解消されない
- **後方互換 (schema_version 1 のデコード維持)**: 中間状態を排除しシンプルに保つため不要
- **Yellow を残す (transitions_to 未宣言時)**: spec に型を宣言したなら遷移も宣言すべき。未宣言は不完全な spec であり Red が正しい
- **全 pub 型を warning (逆方向フィルタ)**: enum のみ・struct のみなどのフィルタは恣意的。DomainTypeKind で意図的に分類する方が明確

## Consequences

- Good: spec と型宣言のライフサイクルが独立。spec 承認後も型宣言は更新可能
- Good: 型カテゴリ別に検証内容が明確。不正な組み合わせ (ValueObject に transitions_to) が型レベルで不可能
- Good: Blue/Red 2 値で判定が明確。曖昧な Yellow ゾーンがない
- Good: Phase 3 (3-12) の spec ↔ code 整合性チェックの前提が整う
- Bad: 既存全 track の spec.json からのマイグレーションが必要
- Bad: domain-types.json + domain-types.md が track ディレクトリのファイル数を増やす
- Bad: ConfidenceSignal::Yellow は Stage 2 で使用されなくなるが、Stage 1 が使うため型自体は残る

## Reassess When

- Phase 3 (3-12) の実装で domain-types.json の構造が不足していると判明した場合
- 型カテゴリの 5 分類では表現できないケースが出現した場合 (例: associated type を持つ trait)
- Stage 1 の信号も Blue/Red 2 値に統一すべきと判断された場合
- **Yellow の再導入 (承認状態シグナル)**: 逆方向チェック (code → spec) で未宣言の型が検出された際、AI が自動で domain-types.json にエントリを追加するフローが実装された場合、Yellow = 「AI 自動追加・人間未承認」として再導入を検討する。Blue = 人間承認済み、Red = 未宣言/不一致、Yellow = 自動追加・要確認。エントリに `approved: bool` フィールドを追加して表現する案
