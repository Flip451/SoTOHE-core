# TDDD 型カタログ Taxonomy 拡張 — アプリケーション層パターンの幅を広げる

## Status

Accepted (implemented by track tddd-02-usecase-wiring-2026-04-14, 2026-04-14)

## Context

SoTOHE-core は **テンプレートリポジトリ** として設計されており、複数のプロジェクトで採用されることを想定している。採用先プロジェクトはそれぞれ異なるアプリケーション層アーキテクチャを選択しうる:

- **Struct-only Use Case**: `pub struct FooUseCase<W: Port>` を CLI から直接呼び出す (pragmatic Rust パターン、現 SoTOHE 自体が採用している)
- **Clean Architecture (Uncle Bob)**: `pub trait FooInputPort` (primary port) + `pub struct FooInteractor` (実装)
- **DDD Application Service**: `pub struct FooApplicationService` がドメインサービスを調整する
- **CQRS**: `pub struct FooCommand` / `pub struct FooQuery` のデータオブジェクト + ハンドラ
- **DTO-heavy**: `pub struct CreateUserDto` / `pub struct UserResponseDto` の層横断データ容器

既存の `TypeDefinitionKind` enum (tddd-01, ADR `2026-04-11-0002-tddd-multilayer-extension.md`) は `Typestate` / `Enum` / `ValueObject` / `ErrorType` / `TraitPort` の 5 variant のみ。これらは **domain 層** には十分だが、複数の異なるアーキテクチャパターンが存在する **usecase/application 層** では semantic な表現力が不足している。

tddd-02 計画段階 (2026-04-13) で観測された具体的な摩擦:

1. `SaveTrackUseCase<W>` を `value_object` として登録すると、「これは use case である」という semantic な信号が失われる。
2. 将来の採用プロジェクトが Clean Architecture を採用した場合、`FooInteractor` を通常の struct と区別する方法がない。
3. `TraitPort` が primary port (driving) と secondary port (driven) の区別なく使われ、hexagonal architecture の方向性 (CLI → primary port → interactor → secondary port → adapter) が不明瞭になる。
4. CQRS パターンの Command/Query オブジェクトが generic な value object と区別できない。
5. 集約コンストラクト用の Factory struct に専用 variant がない。

tddd-01 で意図的に `libs/domain/src/tddd/` は layer-agnostic に保たれている (`MethodDeclaration`, `check_consistency`, `evaluate_trait_port` はすべて抽象的概念で動作する)。そのため variant set の拡張は domain ロジックの変更を要求せず、`TypeDefinitionKind` enum、`kind_tag` マッピング、および `catalogue_codec` / `type_catalogue_render` インフラの変更で完結する。

## Decision

### D1: `TypeDefinitionKind` に 7 新 variants を追加する

```rust
pub enum TypeDefinitionKind {
    // 既存 (変更なし)
    Typestate { transitions: TypestateTransitions },
    Enum { expected_variants: Vec<String> },
    ValueObject,
    ErrorType { expected_variants: Vec<String> },

    // TraitPort からリネーム (D2 参照)
    SecondaryPort { expected_methods: Vec<MethodDeclaration> },

    // 新規
    ApplicationService { expected_methods: Vec<MethodDeclaration> },
    UseCase,
    Interactor,
    Dto,
    Command,
    Query,
    Factory,
}
```

| Variant | `kind_tag` | Semantic | Forward check 振る舞い |
|---|---|---|---|
| `ApplicationService` | `application_service` | Primary port trait — 外部 actor から application を駆動する境界 | `SecondaryPort` と同じ L1 メソッドシグネチャ検証 (`expected_methods`) |
| `UseCase` | `use_case` | Struct-only use case — trait 抽象なしの業務操作調整者 | 存在チェックのみ (`ValueObject` と同じ) |
| `Interactor` | `interactor` | `ApplicationService` trait の実装 struct (Clean Architecture) | 存在チェックのみ |
| `Dto` | `dto` | 層境界を越える純粋データ容器 | 存在チェックのみ |
| `Command` | `command` | CQRS command オブジェクト (immutable 入力データ) | 存在チェックのみ |
| `Query` | `query` | CQRS query オブジェクト (読取パラメータ) | 存在チェックのみ |
| `Factory` | `factory` | 集約/エンティティ構築を担う struct | 存在チェックのみ |

### D2: `TraitPort` → `SecondaryPort` リネーム

既存の `TraitPort` variant を `SecondaryPort` として再定義する:

- `kind_tag`: `"trait_port"` → `"secondary_port"`
- Enum variant: `TraitPort { expected_methods }` → `SecondaryPort { expected_methods }`
- Rendered section header: `## Trait Ports` → `## Secondary Ports`
- 関数: `evaluate_trait_port` → `evaluate_secondary_port` (および `evaluate_application_service` を追加し、共通 L1 ロジックをヘルパー経由で共有)

これにより primary/secondary port の対称性が確立される: `ApplicationService` (primary, driving) と `SecondaryPort` (secondary, driven) は構造的には同一だが semantic に区別される。

### D3: 新 struct variants は存在チェックのみ

`UseCase`, `Interactor`, `Dto`, `Command`, `Query`, `Factory` はすべて **`ValueObject` と同じ存在チェックのみ** を forward check として使用する。本 track では variant 固有の検証ルールを持たせない。

これは意図的な YAGNI 判断である:

- Variant 固有の検証 (例: `Dto` は「メソッドなし」または「getter のみ」を要求、`Interactor` は「`ApplicationService` Y を impl すること」を要求) は将来の track で追加可能。
- 具体的な採用プロジェクトからの要求がない状態で先行的なルールを追加すると、テンプレート採用者の意図を拘束してしまう。
- 現在の振る舞いは、enforcement を fabrication せずに完全な semantic 語彙 (新 7 ラベル + 1 rename) を開発者に提供する。

### D4: `ApplicationService` は `SecondaryPort` と L1 メソッドシグネチャ検証を共有する

`ApplicationService` と `SecondaryPort` は両方とも `expected_methods: Vec<MethodDeclaration>` を持ち、同じ forward check ロジックを使用する。実装は共通ロジックを共有ヘルパー (例: `evaluate_trait_methods`) に抽出し、`evaluate_application_service` と `evaluate_secondary_port` の両方から呼び出す。

ロジックが同一にもかかわらず両 variant を持つ理由:

- **Semantic な区別**: カタログ読者は `application_service` と `secondary_port` を見て即座に、その trait が driving (primary) port か driven (secondary) port かを判断できる。
- **将来の検証分化**: variants は後から variant 固有のルールを追加する余地を与える (例: 「ApplicationService のメソッドは `Result<_, _>` を返さなければならない」)。
- **Rendered view の明瞭さ**: Section header が `## Application Services` と `## Secondary Ports` を区別する。

### D5: CLI シーケンス制約 — `architecture-rules.json` flip は CLI 一般化後に commit する

tddd-02 において `architecture-rules.json` の `usecase.tddd.enabled = true` flip (T007) は、CLI 一般化タスク (T005 `signals.rs`, T006 `baseline.rs`) の **後** に commit しなければならない。逆順で commit すると既存 CLI (`"domain"` ハードコードと非 domain 層 reject を持つ) は `usecase`-enabled な設定を処理できず runtime error を起こす。

このシーケンスは新しいアーキテクチャ判断ではなく実装制約だが、将来類似の層有効化 track が同じ順序に従えるよう本 ADR に記録する。

### D6: domain 層 layer-agnostic 不変条件 (確認)

tddd-01 は `libs/domain/src/tddd/` が layer-agnostic であることを確立した — evaluator, baseline builder, consistency check には `"domain"` に紐づくハードコード文字列がない。本 ADR は tddd-02 の domain 層変更を以下の範囲に限定することを確認する:

1. **`catalogue.rs`**: `TypeDefinitionKind` enum への 7 新 variants 追加 + `TraitPort` → `SecondaryPort` リネーム。
2. **`signals.rs`**: `TypeDefinitionKind::TraitPort { .. }` match arm を `SecondaryPort { .. }` にリネーム + `ApplicationService { .. }` の新 match arm を追加 (`evaluate_application_service` 関数として共通ヘルパー経由で実装)。`evaluate_trait_port` → `evaluate_secondary_port` リネーム。`Delete` 評価の `TraitPort` matches を `SecondaryPort` に更新。新 struct variants (UseCase/Interactor/Dto/Command/Query/Factory) は forward check `match` arm で `evaluate_existence_only` (ValueObject と同等) パスに追加する。
3. **`consistency.rs`**: trait vs type の分類判定パターンを以下に更新: `matches!(entry.kind(), TypeDefinitionKind::SecondaryPort { .. } | TypeDefinitionKind::ApplicationService { .. })` → trait 分類 (expected_methods を持つ両 variant); それ以外 → type 分類。新 struct variants (UseCase/Interactor/Dto/Command/Query/Factory) は自動的に type 分類に属するため追加の match arm は不要。

これらの変更はすべて **`TypeDefinitionKind` の variant 定義変更に連動する機械的リネーム + 新 variant 追加のみ** であり、forward/reverse check pipeline のアルゴリズム自体は変更しない。`check_consistency` は `MethodDeclaration` + `check_consistency` を通じて任意の `kind_tag` 値を既に処理できる設計であり、layer-agnostic 不変条件は維持される。infrastructure/CLI 層の変更 (codec/renderer/CLI) は domain の変更に依存するが、domain は他の層に依存しない (依存方向の不変条件を維持)。

### D7: Rendered section header の拡張

`libs/infrastructure/src/type_catalogue_render.rs` に新 variants 用のセクションヘッダを追加する: `## Application Services`, `## Use Cases`, `## Interactors`, `## DTOs`, `## Commands`, `## Queries`, `## Factories`。既存の `## Trait Ports` は `## Secondary Ports` にリネームする。これにより rendered markdown view が enum taxonomy と semantic に整合する。

## Rejected Alternatives

### A1: `TypeDefinitionKind` を 5 variants のまま維持し、すべてを `value_object` で扱う

却下理由:

- アプリケーション層の semantic 表現力が失われる。
- 将来のテンプレート採用者がカタログで自らのアーキテクチャパターンを区別できない。
- `value_object` による拡張はアプリケーション層パターンが増えるほどスケールしない。

### A2: `UseCase` のみを追加する (現 SoTOHE 構造に一致)

却下理由:

- SoTOHE はテンプレートであり、struct-only パターンは採用者が選びうる多くのパターンのうちの 1 つに過ぎない。
- taxonomy を SoTOHE 現状に narrowing すると、このパターンを使わない採用プロジェクトで将来 rename track が必要になる。

### A3: `ApplicationService` と `SecondaryPort` を `primary: bool` フラグ付きの `TraitPort` 単一 variant に統合する

却下理由:

- enum-first 原則 (`.claude/rules/04-coding-principles.md`) 違反: variant レベル分離の代わりに boolean 判別を導入している。
- Rendered section header の区別が失われる。
- hexagonal architecture が明示的に命名する primary/secondary 対称性が隠れる。

### A4: 全 variants を追加し、同時に variant 固有検証ルールも実装する (Dto はメソッドなし、Interactor は ApplicationService の impl、など)

却下理由:

- 先行的な検証は採用者の意図と合わない可能性がある。
- track scope が大幅に拡大する (cascade tests, rustdoc エッジケース, 追加の forward check パス)。
- 具体的な採用プロジェクトからの要求があった時点で段階的に追加可能。

### A5: `EventHandler`, `Saga`, `Policy`, `Specification` も追加する

本 track では却下 (follow-up に延期):

- 一般的なプロジェクトでの出現頻度が相対的に低い。
- SoTOHE 現状にカタログ化対象のコードがない。
- 具体的な採用プロジェクトが必要とするタイミングで追加可能。

## Consequences

### Positive

- **テンプレート表現力**: 採用プロジェクトがカタログで自らのアーキテクチャパターンを semantic ラベルで表現できる。すべてを `value_object` でまとめる必要がなくなる。
- **Primary/secondary port の明瞭さ**: hexagonal architecture の port 方向性がカタログに可視化される。
- **YAGNI-safe**: variant 固有検証は将来、既存カタログを壊さずに追加できる。
- **Dogfooding**: tddd-02 の `usecase-types.json` seed が新 variants のうち 4 つ (`application_service`, `secondary_port`, `error_type`, `use_case`) を使用し、実際の usecase 層型に対して拡張 taxonomy を検証する。

### Negative

- **Cascade rename コスト**: `TraitPort` → `SecondaryPort` は ~5 ファイル / ~39 occurrences に触れる必要がある。有界だが非自明。
- **後方互換性の破壊**: `"trait_port"` kind_tag は codec から削除される。これを使う既存カタログは再生成が必要。tddd-01 の「後方互換なし」前例に従い、ユーザー指示 (2026-04-13) と整合する。
- **未使用 variant のノイズ**: `Interactor`, `Dto`, `Command`, `Query`, `Factory` variants は SoTOHE 自体では使われない。読者が「なぜ存在するのか」と疑問に思う可能性がある。本 ADR によるテンプレート採用者動機の文書化で緩和する。

### Neutral

- `TypeDefinitionKind` が 5 variants から 12 variants に拡大する。カタログファイルサイズ / decode 性能への直接影響はない。
- `baseline_codec` の schema は v2 のまま (変更なし)。Baseline は `TypeDefinitionKind` を保存しない — 代わりに `TypeBaselineEntry` / `TraitBaselineEntry` を保存し、これらは `TypeKind` (rustdoc 由来の struct/enum 分類、別の enum) を使う。

## Reassess When

- **採用プロジェクトからのフィードバック**: SoTOHE 採用プロジェクトから「7 新 variants では不足」または「自らのパターンと重複する」と報告された場合、taxonomy を見直す。
- **Variant 固有検証の要求増加**: 複数の track が「Dto はメソッドなし」や「Interactor は ApplicationService を impl する」の検証を求めた場合、variant 固有 forward check ルールの追加を検討する。
- **新しいアプリケーション層パターンの出現**: `EventHandler`, `Saga`, `Policy`, `Specification` などが採用プロジェクトで頻出するようになったら taxonomy を拡張する。
- **Cross-layer 型参照が gating 関心事になった場合**: ADR `2026-04-11-0002` Phase 2 の cross-layer catalogue references 作業が始まったら、ある層の primary port trait が別層の secondary port を参照する必要があるかを再評価する。

## Related

- **ADR `2026-04-11-0002-tddd-multilayer-extension.md`**: 本 ADR の親 ADR。multilayer TDDD インフラを導入し、本 ADR はその `TypeDefinitionKind` を拡張する。Phase 1 の「`domain/tddd/` を layer-agnostic にする」目標の完遂を担う。
- **Track `tddd-01-multilayer-2026-04-12`**: 本 ADR が土台とする multilayer インフラを実装した track。
- **Track `tddd-02-usecase-wiring-2026-04-14`**: 本 ADR を実装する track。
- **`.claude/rules/04-coding-principles.md` (Enum-first パターン)**: boolean フラグより variant を優先する設計原則。
- **`knowledge/conventions/hexagonal-architecture.md`**: primary/secondary port の区別を説明する convention。
