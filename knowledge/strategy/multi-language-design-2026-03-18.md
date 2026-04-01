# STRAT-11: 多言語プロジェクト対応 — ハーネスの言語非依存化設計

> **作成日**: 2026-03-18
> **ステータス**: 設計検討中
> **関連 TODO**: STRAT-11, SPEC-08, GAP-08

---

## 1. 背景

SoTOHE-core のハーネス（`sotp` CLI + track workflow + agent orchestration）は Rust 製だが、管理対象プロジェクトが Rust である必要は本質的にはない。現状は CI ゲート・コーディングルール・レイヤーチェックが Rust にハードコードされているため、他言語プロジェクトでの利用が困難。

## 2. 方針

ハーネスの核（状態遷移・ファイルロック・レジストリ管理・agent 協調・hook ガード・Git ワークフロー）は言語非依存に保ち、言語固有の部分をプラグイン的に差し替え可能な設計へ移行する。

**対象外**: `sotp` CLI 自体の実装言語は Rust を維持。多言語対応はハーネスが**管理するプロジェクト**の言語を選択可能にすることであり、ハーネス自体を他言語で書き直すことではない。

---

## 3. 現状の言語依存度分析

### 3.1 既に言語非依存な部分（そのまま使える）

- Track ワークフロー全体（metadata.json SSoT, spec.md, plan.md, verification.md）
- `sotp` CLI の大半（状態遷移、ファイルロック、レジストリ管理、worktree guard）
- Agent 協調（agent-profiles.json, capability routing）
- Hook ガード（shell コマンド検証、テストファイル保護 — テストファイルパターンの設定化が必要）
- Git ワークフロー（ブランチ戦略、コミットガード、PR ライフサイクル）
- 設計哲学・不変条件（README.md）

### 3.2 Rust にハードコードされている部分（差し替えが必要）

| 箇所 | Rust 固有の内容 | 汎用化方針 | 難易度 |
|------|----------------|-----------|--------|
| `cargo make ci` の中身 | clippy, rustfmt, nextest, deny | 言語別の CI タスクセットを定義可能に。`Makefile.toml` に `[tasks.ci-lang]` を言語別に切り替えるディスパッチ層を追加 | 低 |
| `.claude/rules/04-coding-principles.md` | Rust の所有権、ライフタイム、trait 設計 | 言語別ルールファイル（`.claude/rules/lang/rust.md` 等）に分離。共通原則は `04-coding-principles-common.md` に残す | 低 |
| `.claude/rules/05-testing.md` | `#[cfg(test)]`, `cargo make test`, mockall | 同上。テスト哲学（TDD, naming convention）は共通、ツール固有部分は言語別 | 低 |
| `track/tech-stack.md` | Rust toolchain, MSRV, Edition | テンプレート変数化。初回 `/track:bootstrap` 時に言語選択→テンプレート展開 | 低 |
| `check-layers` | Cargo workspace のクレート依存方向チェック | `docs/architecture-rules.json` は言語非依存。チェッカー実装を言語別に | 中 |
| `verify-*` サブコマンド | 一部が `.rs` ファイル構造を前提 | spec/plan/metadata 検証は既に汎用。コード構造検証のファイルパターンを設定ファイル化 | 中 |
| `is_test_file` パターン | `*_test.rs`, `tests/**/*.rs` | `harness.toml` の `test_file_patterns` で外部設定化 | 低 |
| `Dockerfile` / `docker-compose.yml` | Rust toolchain コンテナ | 言語別 Dockerfile テンプレートを提供 | 中 |

---

## 4. ディレクトリ構造方針

現状、共通ルールと言語固有ルールがフラットに混在（`.claude/rules/` 内に `01-language.md`（共通）と `04-coding-principles.md`（Rust 固有）が同列）。「何を差し替えればいいか」が不明瞭。

### 4.1 推奨案（言語固有を子ディレクトリに分離）

```
.claude/rules/
  01-language.md              ← 共通（移動なし）
  06-security.md              ← 共通（移動なし）
  08-orchestration.md         ← 共通（移動なし）
  09-maintainer-checklist.md  ← 共通（移動なし）
  10-guardrails.md            ← 共通（移動なし）
  lang/
    rust/
      coding-principles.md    ← 現 04 から移動
      testing.md              ← 現 05 から移動
    typescript/               ← 将来追加
      coding-principles.md
      testing.md
```

### 4.2 却下案（common/ + lang/ の完全分離）

全ファイル移動が必要で CLAUDE.md, settings.json, hook 参照パスの大規模書き換えが発生。ROI が悪い。

### 4.3 同じ方針を適用すべき他の箇所

| 箇所 | 現状 | 分離方針 |
|------|------|---------|
| `track/tech-stack.md` | Rust toolchain 前提 | `harness.toml` の `language` に基づくテンプレート展開 |
| `Makefile.toml` CI タスク | `clippy`, `rustfmt`, `nextest` がハードコード | `[tasks.ci-lang-rust]`, `[tasks.ci-lang-typescript]` 等の言語別タスクセットを定義し、`[tasks.ci]` がディスパッチ |
| `Dockerfile` | Rust toolchain コンテナ | `docker/rust/Dockerfile`, `docker/typescript/Dockerfile` 等に分離 |
| `is_test_file` パターン | `*_test.rs`, `tests/**/*.rs` ハードコード | `harness.toml` の `test_file_patterns` で外部設定化 |

### 4.4 移行時の制約

既存プロジェクト（Rust 利用中）の参照パスが壊れないよう、シンボリックリンクまたは deprecation 期間を設ける。

---

## 5. 実装ロードマップ（段階的）

1. **Phase A: 設定ファイル導入** — `harness.toml` を新設し、`language`, `test_file_patterns`, `ci_tasks`, `layer_checker` を外部化
2. **Phase B: ルールファイル分離** — `.claude/rules/lang/` ディレクトリを作成し、言語別ルールを分離。共通ルールはそのまま維持
3. **Phase C: CI ゲートのディスパッチ化** — `cargo make ci` が `harness.toml` の `language` フィールドに基づいて言語別タスクセットを呼び出す
4. **Phase D: テンプレートジェネレータ** — `sotp init --language rust|typescript|go` で言語別の初期ファイル群を生成

---

## 6. 設計哲学の言語別適用度

「コンパイラが最終審判」をコンパイラのない言語でどう読み替えるか。

| 設計哲学 | 静的型付き言語 (Rust/Go/TS) | 動的+静的解析 (PHP 8+/Python+mypy) | 動的のみ (Shell/Lua) |
|---------|---------------------------|-----------------------------------|---------------------|
| コンパイラ/解析が最終審判 | ◎ コンパイラで保証 | ○ 静的解析で近似（ランタイム漏れあり） | △ テストのみが証拠 |
| 不正状態を型で表現不可能に | ◎ 型システムで強制 | ○ enum/union types で近似（PHP 8.1+） | × 実現困難 |
| レイヤー依存の CI 強制 | ◎ crate/module 境界 | ○ Deptrac/PHPArkitect/import-linter | △ ディレクトリ規約のみ |
| unsafe/危険操作の禁止 | ◎ `#![forbid(unsafe_code)]` | ○ PHPStan ルールで eval/exec 禁止 | × 手段なし |

**結論**: 静的解析エコシステムが成熟した動的言語（PHP 8+, Python+mypy）なら哲学の9割は維持可能。エコシステムが弱い言語ではテストカバレッジへの依存度が上がり、「客観証拠」の強度が下がる。

---

## 6.5. ドメインモデリング強制の言語別戦略

> **追記**: 2026-03-19 — Phase 1.5 (構造リファクタリング) の設計議論を踏まえた補足。
> 詳細は [`tmp/refactoring-plan-2026-03-19.md`](refactoring-plan-2026-03-19.md) §0 を参照。

### 問題の構造

Phase 1.5 で判明した根本課題: **domain 層に `String` が残っている → その String を解釈するロジックが複数層に散らばる → ロジック流出**。

Rust では enum 化 + exhaustive match でコンパイル時に強制できるが、動的言語では型システムによる強制が弱い〜不可能。
言語の型システム強度に応じて、**強制メカニズムを使い分ける 3 段階のレベル**を定義する。

### 検証レベル定義

| レベル | 対象言語 | 型による保証 | 補償手段 |
|---|---|---|---|
| **Gold** | Rust, TypeScript (strict) | コンパイラが exhaustive match を強制。`pub(crate)` で構築制限。crate/package 依存方向をビルドシステムで強制 | `sotp verify` は追加安全網 |
| **Gold 候補** | MoonBit (1.0 未リリース) | ADT + exhaustive match + trait。型システムは Gold 水準だがエコシステム未成熟 | 1.0 リリース後に再評価 |
| **Silver** | Python (mypy strict), PHP 8.1+, Kotlin, Swift | 型ヒント + 静的解析で大部分を検出。ランタイムに漏れる可能性あり | `sotp verify` + import linter で補強 |
| **Bronze** | Ruby, JavaScript (untyped), Shell, Lua | 型による強制なし | **spec.md Domain States + 信号機が生命線**。テスト + CI linter で補償 |

### 言語別の強制メカニズム対応表

| メカニズム | Gold (Rust) | Gold (TypeScript) | Silver (Python mypy) | Silver (PHP 8.1+) | Bronze (Ruby/JS) |
|---|---|---|---|---|---|
| domain 状態を enum で表現 | `enum Verdict { ZeroFindings, FindingsRemain }` | `type Verdict = "zero_findings" \| "findings_remain"` (discriminated union) | `class Verdict(Enum): ZERO_FINDINGS = ...` + mypy strict | `enum Verdict: string { case ZeroFindings = ... }` | convention のみ。定数で代替 |
| exhaustive match | コンパイラ保証 | `switch` + `never` 型で tsc が検出 | `match` + mypy exhaustiveness check (3.10+) | PHPStan が `match` 網羅性を検査 | **不可能** → テストで補償 |
| 層境界（import 方向） | Cargo.toml | tsconfig paths + eslint-plugin-import | import-linter (`importlinter.toml`) | Deptrac | `sotp verify layer-imports` (AST) |
| 構築制限 (pub(crate)) | コンパイラ | `export` を制限 + barrel file | `__all__` + `_` prefix + mypy | `@internal` annotation + PHPStan | **不可能** → linter で警告 |
| pub String 検出 | `sotp verify domain-strings` (syn AST) | `sotp verify domain-strings` (ts-morph AST) | `sotp verify domain-strings` (ast module) | `sotp verify domain-strings` (nikic/PHP-Parser) | `sotp verify domain-strings` (対応 AST パーサー) |

### Bronze レベルの補償戦略（動的言語で型がない場合）

型で守れない言語では、以下の 3 層で補償する:

**層 1: spec.md Domain States + 信号機（仕様レベル）**

Bronze 言語では spec.md が唯一の「ドメインモデルの SSoT」になる。
信号機による品質管理の重要度が Gold/Silver より高い。

```markdown
## Domain States
| Entity | States | Signal | Source | Enforcement |
|---|---|---|---|---|
| Order status | pending, confirmed, shipped, delivered | 🔵 | [source: PRD §3] | test exhaustiveness |
| Payment method | credit_card, bank_transfer, cash_on_delivery | 🟡 | [source: inference] | constants module |
```

`Enforcement` 列を追加し、型がない場合の代替強制手段を明示する。

**層 2: `sotp verify` による AST 検査（CI レベル）**

- `sotp verify domain-strings`: 言語別 AST パーサーで domain 層の生文字列フィールドを検出
- `sotp verify layer-imports`: import/require 文を走査して依存方向違反を検出
- `sotp verify test-exhaustiveness`: spec の Domain States に列挙された全状態に対応するテストケースが存在するか検査

**層 3: テスト網羅率（ランタイムレベル）**

domain 状態を追加したら全 switch/case 箇所のテストが落ちるように設計する:

```ruby
# Ruby: 状態追加時にテストが落ちることを保証するパターン
VALID_STATUSES = %w[pending confirmed shipped delivered].freeze

def handle_status(status)
  case status
  when "pending"    then handle_pending
  when "confirmed"  then handle_confirmed
  when "shipped"    then handle_shipped
  when "delivered"  then handle_delivered
  else raise ArgumentError, "Unknown status: #{status}"
  end
end

# テスト: VALID_STATUSES の全要素が case 文で処理されることを検証
VALID_STATUSES.each do |status|
  test "handles #{status}" do
    assert_nothing_raised { handle_status(status) }
  end
end
```

### `harness.toml` の言語設定への反映

```toml
[language]
name = "ruby"
verification_level = "bronze"  # gold | silver | bronze

[language.domain_modeling]
# Bronze: spec.md Domain States が唯一の SSoT
spec_domain_states_required = true
spec_enforcement_column_required = true  # Bronze のみ必須

[language.verification]
# 言語別の AST パーサーとパターン
domain_string_detector = "sotp verify domain-strings --lang ruby"
layer_import_checker = "sotp verify layer-imports --lang ruby"
test_exhaustiveness_checker = "sotp verify test-exhaustiveness --lang ruby"
```

### Phase 2 信号機との統合

Bronze 言語では信号機の `Enforcement` 列が追加される以外は Gold/Silver と同じフローが使える:

```
domain_modeler → spec.md Domain States + 信号機 + Enforcement 列
    ↓
spec_reviewer → 信号機評価 + 「Bronze なので test exhaustiveness は足りているか？」
    ↓
planner → タスク分解（テスト追加タスクが自動的に含まれる）
    ↓
implementer → 実装 + テスト
    ↓
code_reviewer + acceptance_reviewer
```

**核心**: 検証レベルが下がるほど spec.md と信号機の重要度が上がる。Bronze では「信号機が 🔵 = テストで全状態が検証済み」という意味になる。

### Silver 具体例: PHP 8.1+ / Laravel でのドメインモデリング

PHP 8.1 の native enum は Rust の enum にかなり近い。抽象クラスではなく **enum が主役**。

#### Rust との対応表

| 概念 | Rust | PHP 8.1+ / Laravel |
|---|---|---|
| 有限状態の型 | `enum Verdict { ZeroFindings, FindingsRemain }` | `enum Verdict: string { case ZeroFindings = 'zero_findings'; ... }` |
| 不変値オブジェクト | `struct` | `readonly class` (PHP 8.2) |
| exhaustive match | コンパイラ保証 | PHPStan level 8+ で検出 |
| 層境界の強制 | Cargo crate 依存 | Deptrac |
| DB ↔ enum 変換 | serde `#[serde(rename_all)]` | Eloquent `$casts` |

#### domain 層の enum（Rust の enum に相当）

```php
// app/Domain/Review/Verdict.php
enum Verdict: string
{
    case ZeroFindings = 'zero_findings';
    case FindingsRemain = 'findings_remain';
}

// app/Domain/Review/Severity.php
enum Severity: string
{
    case P0 = 'P0';
    case P1 = 'P1';
    case Low = 'LOW';
    case Info = 'INFO';

    public function isActionable(): bool
    {
        return match($this) {
            self::P0, self::P1 => true,
            self::Low, self::Info => false,
        };
    }
}
```

#### 不変の状態遷移（readonly class + 新インスタンス返却）

```php
// app/Domain/Review/ReviewState.php
readonly class ReviewState
{
    public function __construct(
        public ReviewStatus $status,
        public ?string $codeHash,
        public EscalationPhase $escalation,
    ) {}

    public function recordRound(Verdict $verdict): self
    {
        return new self(
            status: match([$this->status, $verdict]) {
                [ReviewStatus::InProgress, Verdict::ZeroFindings] => ReviewStatus::FastPassed,
                // ... 他の遷移
            },
            codeHash: $this->codeHash,
            escalation: $this->escalation,
        );
    }
}
```

#### Laravel での層配置

```
app/
├── Domain/              ← 純粋。Eloquent に依存しない
│   ├── Review/
│   │   ├── Verdict.php          (enum)
│   │   ├── ReviewStatus.php     (enum)
│   │   └── ReviewState.php      (readonly class, 状態遷移)
│   └── PR/
│       ├── GhReviewState.php    (enum)
│       └── Severity.php         (enum)
├── UseCases/            ← Repository interface を引数で受け取るオーケストレーション
│   └── RecordRoundAction.php
├── Infrastructure/      ← Eloquent model, API client
│   └── EloquentReviewRepository.php
└── Http/Controllers/    ← 薄いアダプター（CLI 層に相当）
```

#### Laravel の典型的な罠: Eloquent Model へのロジック流出

Active Record パターンの Eloquent Model にドメインロジックを書くのが「`String` が domain に残る」の PHP 版。

```php
// NG: Eloquent Model にロジックが流出（String 比較）
class Review extends Model
{
    public function isPassed(): bool
    {
        return $this->state === 'APPROVED'; // ← String 比較
    }
}

// OK: Domain enum にロジックを集約
enum GhReviewState: string
{
    case Approved = 'APPROVED';
    case ChangesRequested = 'CHANGES_REQUESTED';
    case Commented = 'COMMENTED';

    public function isPassed(int $actionableCount): bool
    {
        return match($this) {
            self::Approved => true,
            self::ChangesRequested => false,
            self::Commented => $actionableCount === 0,
        };
    }
}

// Eloquent Model は cast するだけ（infrastructure 層）
class Review extends Model
{
    protected $casts = [
        'state' => GhReviewState::class,  // DB の string → enum に自動変換
    ];
}
```

### Gold 具体例: TypeScript + Next.js + FSD でのドメインモデリング

TypeScript は Gold レベル。discriminated union が Rust の enum with data に相当し、
FSD (Feature-Sliced Design) の entities 層が domain 層に直接対応する。

#### SoTOHE-core 層 ↔ FSD 層の対応

| SoTOHE-core の層 | FSD の層 | 役割 |
|---|---|---|
| **domain** | `entities/` + `shared/types/` | 型定義 + 純粋関数 |
| **usecase** | `features/` | ユーザー操作単位のオーケストレーション |
| **infrastructure** | `shared/api/` + `shared/lib/` | API client, localStorage 等 |
| **CLI (main)** | `pages/` + `widgets/` | DI 組み立て + 表示 |

#### Rust との対応表

| 概念 | Rust | TypeScript + FSD |
|---|---|---|
| 有限状態の型 | `enum` | literal union / discriminated union |
| exhaustive match | コンパイラ保証 | `tsc --strict` + `satisfies never` |
| 不変値オブジェクト | `struct` | `Readonly<T>` / `as const` |
| API boundary バリデーション | serde + typed deserialization | Zod `.parse()` |
| 層境界の強制 | Cargo crate 依存 | FSD ルール + `eslint-plugin-boundaries` |
| 構築制限 (`pub(crate)`) | コンパイラ | barrel file (`index.ts`) で export 制御 |
| pub String 検出 | `sotp verify domain-strings` (syn) | `sotp verify domain-strings --lang ts` (ts-morph) |

#### domain 層: discriminated union（Rust の enum with data に相当）

```typescript
// entities/review/model/types.ts — domain 層
type Verdict = "zero_findings" | "findings_remain";

type ReviewStatus =
  | { kind: "not_started" }
  | { kind: "in_progress" }
  | { kind: "fast_passed"; codeHash: string }
  | { kind: "approved"; codeHash: string; approvedAt: Date }
  | { kind: "invalidated"; reason: string };

// exhaustive match — tsc が未処理の case をコンパイルエラーにする
function statusLabel(status: ReviewStatus): string {
  switch (status.kind) {
    case "not_started":   return "未開始";
    case "in_progress":   return "進行中";
    case "fast_passed":   return "Fast通過";
    case "approved":      return "承認済み";
    case "invalidated":   return "無効化";
    // case を追加忘れ → tsc エラー（satisfies never）
  }
}

type Severity = "P0" | "P1" | "LOW" | "INFO";

const isActionable = (s: Severity): boolean =>
  s === "P0" || s === "P1";
```

#### infrastructure 層: Zod で API boundary バリデーション（serde に相当）

```typescript
// shared/api/schemas/review.ts
import { z } from "zod";

const VerdictSchema = z.enum(["zero_findings", "findings_remain"]);
const SeveritySchema = z.enum(["P0", "P1", "LOW", "INFO"]);

const ReviewResponseSchema = z.object({
  state: z.enum(["APPROVED", "CHANGES_REQUESTED", "COMMENTED"]),
  verdict: VerdictSchema,
  findings: z.array(z.object({
    severity: SeveritySchema,
    path: z.string(),
    body: z.string(),
  })),
});

// API boundary で parse → domain 型に変換（String が内部に入り込まない）
export const fetchReview = async (id: number) => {
  const raw = await api.get(`/reviews/${id}`);
  return ReviewResponseSchema.parse(raw); // 不正な値は即 ZodError
};
```

#### FSD ディレクトリ構造

```
src/
├── app/                        ← Next.js App Router
│   └── review/[id]/page.tsx    ← DI 組み立て + Server Component
├── widgets/
│   └── review-panel/           ← 複数 feature を組み合わせた UI
├── features/                   ← usecase 層
│   ├── record-round/
│   │   ├── model/
│   │   │   └── record-round.ts ← (reader, writer) => domain 関数呼び出し
│   │   └── ui/
│   │       └── RecordRoundButton.tsx
│   └── resolve-escalation/
├── entities/                   ← domain 層
│   ├── review/
│   │   ├── model/
│   │   │   ├── types.ts        ← Verdict, ReviewStatus, Severity
│   │   │   └── transitions.ts  ← recordRound(), checkCommitReady() 純粋関数
│   │   └── ui/
│   │       └── StatusBadge.tsx  ← domain 型を受け取って表示するだけ
│   └── pr/
│       └── model/
│           └── types.ts        ← GhReviewState
└── shared/                     ← infrastructure 層
    ├── api/
    │   └── schemas/review.ts   ← Zod schema (API boundary)
    ├── types/
    └── lib/
```

#### FSD の層ルールが依存方向を自然に強制

FSD の import ルール: **上位層は下位層のみ import 可能**。

```
app → pages → widgets → features → entities → shared
```

`eslint-plugin-boundaries` で CI 強制:

```jsonc
// .eslintrc.json
{
  "rules": {
    "boundaries/element-types": ["error", {
      "default": "disallow",
      "rules": [
        // entities は shared のみ import 可（domain → nothing に相当）
        { "from": "entities", "allow": ["shared"] },
        // features は entities + shared のみ（usecase → domain に相当）
        { "from": "features", "allow": ["entities", "shared"] }
      ]
    }]
  }
}
```

#### Next.js 固有の考慮: Server/Client boundary

Next.js App Router の Server Component / Client Component 境界は FSD の層とは直交する。
domain 型（entities/）は Server/Client 両方で使えるが、`"use client"` ディレクティブの配置に注意:

- `entities/review/model/types.ts` — Server/Client 両方で import 可（型定義のみ）
- `entities/review/ui/StatusBadge.tsx` — `"use client"` が必要（React hooks を使う場合）
- `features/record-round/model/` — Server Action として実装すれば Server Component 内で直接呼べる

Server Action は「CLI 層で DI を組み立てて usecase を呼ぶ」パターンと自然に対応する:

```typescript
// app/review/[id]/actions.ts — Server Action = DI 組み立て + usecase 呼び出し
"use server";
import { recordRound } from "@/features/record-round/model/record-round";
import { createApiReviewRepository } from "@/shared/api/review-repository";

export async function handleRecordRound(trackId: string, verdict: Verdict) {
  const repo = createApiReviewRepository(); // DI
  return recordRound(repo, { trackId, verdict }); // usecase 呼び出し
}
```

### Gold 候補: MoonBit（1.0 未リリース、要再評価）

> **追記**: 2026-03-19
> **参考**: [moonbitlang.com](https://www.moonbitlang.com/), [MoonBit 1.0 Roadmap](https://www.moonbitlang.com/blog/roadmap)

**作者**: Hongbo Zhang（OCaml コアコントリビューター、ReScript 作者）
**ターゲット**: WASM-GC / JavaScript / Native
**メモリ管理**: GC（Rust の所有権/ライフタイムなし）
**状態**: ベータ。1.0 は 2026 年予定

#### Rust との比較

| 要件 | Rust | MoonBit |
|---|---|---|
| ADT (enum with data) | `enum` | `enum` — ほぼ同じ構文 |
| exhaustive match | コンパイラ保証 | コンパイラ警告（非網羅検出） |
| trait | `trait` + `impl` | `trait` + `impl` — 同等 |
| 不変デフォルト | `let` (mut で可変) | struct フィールドがデフォルト不変 |
| visibility | `pub(crate)` | `pub` / `priv`（粒度は要確認） |
| メモリ管理 | 所有権 + ライフタイム | **GC** — domain 層がシンプルに |
| WASM 出力 | wasm32-unknown-unknown (手動 bindgen) | **WASM-GC ネイティブ** |

#### ドメインモデリングの例

```moonbit
enum Verdict {
  ZeroFindings
  FindingsRemain
}

enum ReviewStatus {
  NotStarted
  InProgress
  FastPassed(String)   // code_hash
  Approved(String)     // code_hash
  Invalidated(String)  // reason
}

enum Severity {
  P0; P1; Low; Info
}

fn is_actionable(s : Severity) -> Bool {
  match s {
    P0 | P1 => true
    Low | Info => false
  }
}
```

#### 評価

**メリット**:
- 型システムは Rust/OCaml 水準。ドメインモデリングの表現力はほぼ Gold
- GC なのでライフタイム不要 → domain 層の純粋関数がよりシンプル
- WASM-GC ネイティブ → フロントエンド/バックエンドで domain 層を共有可能
- 「AI-native」を謳い、SoTOHE のようなエージェント基盤と方向性が一致

**懸念**:
- **1.0 未リリース** — プロダクション利用はリスク大
- **エコシステムが極小** — crates.io / npm 相当の充実度がない
- **`pub(crate)` 相当の粒度が不明** — 層境界の CI 強制力が未確認
- **AI の学習データが少ない** — ハルシネーションリスクが高い（Bronze 的弱点）
- **ツールチェーンの成熟度** — linter, formatter, CI 統合が Rust/TS ほど整っていない

**結論**: 型システムは Gold だがエコシステムは Bronze。**1.0 リリース後に再評価**。WASM domain 共有のユースケースは Rust + wasm-bindgen でも実現可能だが、MoonBit の GC + WASM-GC ネイティブの方が DX は良い可能性がある。

---

## 7. フレームワーク別の組み合わせ考察

### 7.1 Laravel (PHP)

**相性**: ◎ — 動的言語フレームワークの中で最も良い部類

**CI ゲート対応**:

| Rust (現行) | Laravel 代替 |
|------------|-------------|
| `clippy -D warnings` | **Larastan** level max（Laravel マジック対応） |
| `rustfmt --check` | **Laravel Pint** `--test`（Laravel 公式フォーマッタ） |
| `cargo nextest` | **Pest** `--parallel`（Laravel 同梱テストフレームワーク） |
| `cargo deny` | `composer audit` + **Enlightn**（Laravel セキュリティ監査） |
| `check-layers` | **Deptrac**（Laravel 向けプリセットあり） |
| `cargo-llvm-cov` | **Pest** `--coverage --min=80` |
| `#![forbid(unsafe_code)]` | Larastan カスタムルール + Enlightn 危険関数検出 |

**追加で活きるエコシステム**:
- `php artisan`: `cargo make` と同じタスクランナー的役割
- Eloquent Model の型安全化: `@property` PHPDoc + Larastan で「不正状態を型で表現不可能に」を近似
- Feature Test + `RefreshDatabase` trait: テスト独立性をフレームワークが自動保証
- Enlightn: セキュリティ・パフォーマンスの自動監査
- `php artisan migrate --pretend`: 破壊的 DB マイグレーションの事前検出

**注意点**:
- Eloquent の暗黙的振る舞い（マジックメソッド、アクセサ、ミューテタ）— Larastan なしだと静的解析がほぼ無力
- Facade パターン — ドメイン層から排除する規約が必要（Deptrac で強制可能）
- artisan tinker — REPL から本番 DB を直接操作可能（運用ルール + 環境分離で対処）

### 7.2 React / Next.js (TypeScript)

**相性**: ◎ — TypeScript の型システムが設計哲学の適用度を動的言語より高くする

**CI ゲート対応**:

| Rust (現行) | React / Next.js 代替 |
|------------|---------------------|
| `clippy -D warnings` | **tsc --strict --noEmit** + **ESLint** (typescript-eslint) |
| `rustfmt --check` | **Prettier** `--check` + ESLint stylistic rules |
| `cargo nextest` | **Vitest** `--run`（単体）+ **Playwright**（E2E） |
| `cargo deny` | `npm audit` + **license-checker** |
| `check-layers` | **eslint-plugin-boundaries** or **dependency-cruiser** |
| `cargo-llvm-cov` | **Vitest** `--coverage --threshold 80`（v8/istanbul） |
| `#![forbid(unsafe_code)]` | ESLint `@typescript-eslint/no-explicit-any` + `no-unsafe-*` ルール群 |

**追加で活きるエコシステム**:
- Next.js App Router: `app/` ディレクトリ規約がレイヤー構造と自然に対応
- Server Actions / Server Components: サーバー側ロジックの分離が `"use server"` ディレクティブで型レベル強制
- Zod: ランタイムバリデーション + 型推論。API レスポンスの型安全もカバー
- Turborepo / pnpm workspaces: Cargo workspace と同じモノレポ構造

**フロントエンド特有の課題**:
- UI テストの非決定性 — Visual Regression Testing（Chromatic 等）が追加レイヤーとして必要
- クライアント状態管理 — ドメイン層から状態管理ライブラリへの依存を禁止する規約が必要
- ビルド時型チェック vs ランタイム — Zod 等のランタイム検証が必須
- CSS / スタイリング — 型安全の範囲外

### 7.3 Feature-Sliced Design (FSD) + Next.js

**相性**: ◎◎ — React/Next.js との組み合わせでは最良のアーキテクチャパートナー

FSD は React/Vue/Angular 向けのアーキテクチャ方法論。`app → processes → features → entities → shared` の一方向依存ルールでフロントエンドコードを構造化する（https://feature-sliced.design/）。

**レイヤー対応**:

| FSD レイヤー | SoTOHE-core 対応 | 役割 |
|------------|-----------------|------|
| `app/` | `apps/cli` (Composition Root) | アプリ初期化・ルーティング |
| `processes/` | `libs/usecase`（ビジネスフロー） | 複数 feature にまたがるフロー |
| `features/` | `libs/usecase`（個別ユースケース） | 自己完結した機能単位 |
| `entities/` | `libs/domain` | ビジネスエンティティ・型 |
| `shared/` | `libs/domain`（共有型） | 共有ユーティリティ・UI 基盤 |

**CI ゲートでの FSD 強制**:

| SoTOHE-core | FSD + React/Next.js |
|------------|---------------------|
| `check-layers`（Cargo crate 依存） | **@feature-sliced/eslint-config**（FSD 公式 ESLint プラグイン） |
| `architecture-rules.json` | FSD ESLint がレイヤー/スライス/セグメントの依存ルールを自動強制 |
| `deny.toml`（禁止クレート） | ESLint `no-restricted-imports`（レイヤー違反インポート禁止） |

**垂直スライス原則との融合**: FSD の「feature スライス」と SPEC-08 の「垂直スライス原則」は思想がほぼ同一。spec.md の要件を FSD の feature スライスに 1:1 マッピングすれば、CC-SDD-01 のトレーサビリティがディレクトリ構造レベルで可視化される。

**注意点**:
- FSD はフロントエンド専用。フルスタック Next.js ではフロントのみ FSD
- FSD の feature 粒度と track タスクの粒度を合わせる規約が必要
- `shared/` レイヤーが UI コンポーネントも含むため肥大化しやすい

---

## 8. マルチワークスペース構成（Rust backend + Next.js frontend）

バックエンドを Rust、フロントエンドを Next.js + FSD で構成する場合のモノレポ構造。

### 8.1 ディレクトリ構造

```
project-root/
├── harness.toml                ← NEW: ハーネス設定（ワークスペース定義）
├── bin/sotp                    ← ハーネス CLI（言語非依存で動作）
├── track/                      ← 共通
│   ├── registry.md
│   ├── workflow.md
│   ├── tech-stack.md           ← Rust + Next.js 両方を記載
│   └── items/
├── .claude/
│   ├── rules/
│   │   ├── 01-language.md          ← 共通
│   │   ├── 06-security.md          ← 共通
│   │   ├── 08-orchestration.md     ← 共通
│   │   └── lang/
│   │       ├── rust/
│   │       │   ├── coding-principles.md
│   │       │   └── testing.md
│   │       └── typescript/
│   │           ├── coding-principles.md
│   │           └── testing.md
│   └── agent-profiles.json
├── backend/                    ← Rust ワークスペース
│   ├── Cargo.toml
│   ├── libs/
│   │   ├── domain/
│   │   ├── usecase/
│   │   └── infrastructure/
│   └── apps/
│       └── api/
├── frontend/                   ← Next.js + FSD
│   ├── package.json
│   ├── tsconfig.json
│   ├── next.config.ts
│   └── src/
│       ├── app/                ← FSD: app layer
│       ├── processes/          ← FSD: cross-feature flows
│       ├── features/           ← FSD: feature slices
│       ├── entities/           ← FSD: business entities
│       └── shared/             ← FSD: shared UI + utils
├── Makefile.toml               ← 統合タスクランナー
├── docker-compose.yml
└── docs/
    └── architecture-rules.json ← 両方のレイヤールールを定義
```

### 8.2 `architecture-rules.json` v3（マルチワークスペース対応）

```json
{
  "version": 3,
  "workspaces": {
    "backend": {
      "type": "rust-cargo",
      "root": "backend/",
      "layers": [
        { "name": "domain", "path": "backend/libs/domain", "allowed_deps": [] },
        { "name": "usecase", "path": "backend/libs/usecase", "allowed_deps": ["domain"] },
        { "name": "infrastructure", "path": "backend/libs/infrastructure", "allowed_deps": ["domain"] },
        { "name": "api", "path": "backend/apps/api", "allowed_deps": ["domain", "usecase", "infrastructure"] }
      ],
      "checker": "cargo-deny + check_layers.py"
    },
    "frontend": {
      "type": "fsd-nextjs",
      "root": "frontend/",
      "layers": [
        { "name": "shared", "path": "frontend/src/shared", "allowed_deps": [] },
        { "name": "entities", "path": "frontend/src/entities", "allowed_deps": ["shared"] },
        { "name": "features", "path": "frontend/src/features", "allowed_deps": ["shared", "entities"] },
        { "name": "processes", "path": "frontend/src/processes", "allowed_deps": ["shared", "entities", "features"] },
        { "name": "app", "path": "frontend/src/app", "allowed_deps": ["shared", "entities", "features", "processes"] }
      ],
      "checker": "@feature-sliced/eslint-config"
    }
  }
}
```

### 8.3 CI ゲートの統合

```toml
[tasks.ci]
dependencies = ["ci-backend", "ci-frontend", "ci-integration"]

[tasks.ci-backend]
dependencies = ["fmt-check-backend", "clippy-backend", "test-backend", "deny-backend", "check-layers-backend"]

[tasks.ci-frontend]
dependencies = ["tsc-frontend", "lint-frontend", "test-frontend", "audit-frontend", "check-layers-frontend"]

[tasks.ci-integration]
dependencies = ["check-api-contract"]
```

### 8.4 Track ワークフローの運用

1つの track が両方にまたがるケースがある（例: 「ユーザー登録機能」= backend API + frontend feature slice）。

```markdown
# spec.md — 1つのトラックで両方をカバー

### S-REG-01: ユーザー登録 API
🔵 [source: PRD §2.1]
**workspace: backend**

### S-REG-02: 登録フォーム UI
🟡 [source: inference — デザインカンプから推定]
**workspace: frontend**

## Requirements Coverage
- S-REG-01 → T001 (backend/libs/usecase/register.rs)
- S-REG-02 → T002 (frontend/src/features/registration/)
```

### 8.5 Agent 協調の分担

| タスク | 担当 | 根拠 |
|--------|------|------|
| Rust backend 実装 | Claude Code (implementer) | 現行と同じ |
| Next.js frontend 実装 | Claude Code (implementer) | TypeScript も得意 |
| API スキーマ設計 | Codex (planner) | 両側の整合性判断 |
| FSD レイヤー構造レビュー | Codex (reviewer) | アーキテクチャ違反検出 |
| 依存クレート/パッケージ調査 | Gemini (researcher) | Rust + npm 両方をカバー |

### 8.6 STRAT-11 の最小実装で始める方法

フルの多言語対応を待たずに始めるなら:

1. ディレクトリだけ先に切る — `backend/` と `frontend/` を分離
2. `Makefile.toml` に `ci-backend` / `ci-frontend` を追加
3. `.claude/rules/lang/` を作成 — Rust と TypeScript のルールを分離
4. `is_test_file` パターンを両方対応 — `*_test.rs` + `*.test.ts` + `*.spec.ts`
5. FSD ESLint を導入 — `frontend/.eslintrc` に `@feature-sliced/eslint-config`

`harness.toml` や `architecture-rules.json` v3 の正式対応は後から追いつける。

---

## 9. WASM Domain 共有 — 値オブジェクトとバリデーションのフルスタック SSoT

> **追記**: 2026-03-19
> **前提**: Phase 1.5 で domain 層が純粋化（I/O なし、型定義 + 純粋関数のみ）されていること

### 9.1 問題: バリデーションの二重実装

フロントエンドとバックエンドで同じビジネスルールを別言語で二重実装するのが現状の典型的な問題。

```
フロント (JS):  if (email.includes('@')) ...   ← 手書き、ルール乖離のリスク
バック (Rust):  Email::new(s) → Result<Email>  ← domain の値オブジェクト
```

ルールが乖離すると「フロントでは通るがバックで弾かれる」「バックの制約がフロントに伝わっていない」が発生する。

### 9.2 解決: domain 層を WASM で共有

domain 層が純粋（I/O なし）であれば、そのまま WASM にコンパイルしてフロントエンドから import できる。

```
libs/domain/ (Rust)
    ↓ wasm-pack build
domain.wasm + domain.d.ts
    ↓
フロント: import { validate_email, Severity } from 'domain-wasm'
バック:   use domain::{Email, Severity}
```

**Single Source of Truth がコード + 型 + バリデーションの 3 つ全てで成立する。**

### 9.3 実現パターン: Rust + wasm-bindgen + tsify

#### domain 層の値オブジェクト

```rust
// libs/domain/src/email.rs
use tsify::Tsify;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct Email(String);

impl Email {
    pub fn new(s: &str) -> Result<Self, ValidationError> {
        if !s.contains('@') { return Err(ValidationError::InvalidEmail); }
        if s.len() > 254 { return Err(ValidationError::TooLong); }
        Ok(Self(s.to_string()))
    }

    pub fn as_str(&self) -> &str { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub enum Severity { P0, P1, Low, Info }

impl Severity {
    pub fn is_actionable(&self) -> bool {
        matches!(self, Self::P0 | Self::P1)
    }
}
```

#### WASM 公開ラッパー

```rust
// libs/domain-wasm/src/lib.rs — 薄いラッパー（domain 本体はそのまま）
use wasm_bindgen::prelude::*;
use domain::{Email, ValidationError};

#[wasm_bindgen]
pub fn validate_email(s: &str) -> Result<Email, JsValue> {
    Email::new(s).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
pub fn is_severity_actionable(s: &str) -> Result<bool, JsValue> {
    let severity: domain::Severity = serde_json::from_str(&format!("\"{s}\""))
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(severity.is_actionable())
}
```

#### フロントエンドでの利用

```typescript
// entities/email/model/validate.ts (FSD entities 層)
import init, { validate_email } from 'domain-wasm';

await init(); // WASM 初期化（一度だけ）

export function validateEmail(input: string): { ok: true; value: string } | { ok: false; error: string } {
  try {
    const email = validate_email(input);
    return { ok: true, value: email };
  } catch (e) {
    return { ok: false, error: String(e) };
  }
}
```

```tsx
// features/registration/ui/EmailInput.tsx
import { validateEmail } from '@/entities/email/model/validate';

function EmailInput() {
  const [error, setError] = useState<string | null>(null);

  const handleChange = (e: ChangeEvent<HTMLInputElement>) => {
    const result = validateEmail(e.target.value);
    setError(result.ok ? null : result.error);
  };

  return (
    <div>
      <input onChange={handleChange} />
      {error && <span className="error">{error}</span>}
    </div>
  );
}
```

### 9.4 共有すべき範囲

| 共有すべき | 共有不要 |
|---|---|
| 値オブジェクト (`Email`, `TrackId`, `Severity`) | UI 状態 (`isLoading`, `selectedTab`) |
| バリデーションルール | API 通信ロジック |
| enum 定義（discriminated union として TS に型生成） | レンダリング |
| 状態遷移ロジック（純粋関数） | 副作用（fetch, localStorage） |
| エラー型（`ValidationError` の variant） | フレームワーク固有のエラーハンドリング |

**domain 層が純粋であればあるほど共有しやすい。** Phase 1.5 で「domain は I/O なし、純粋関数 + 型定義のみ」と定義したのは、まさにこの共有を可能にする設計。

### 9.5 技術選択肢の比較

| | Rust + wasm-bindgen | Rust + tsify | MoonBit (将来) |
|---|---|---|---|
| WASM 出力 | wasm32-unknown-unknown | 同左 | WASM-GC ネイティブ |
| TS 型生成 | 手動 or `wasm-bindgen` の TS glue | **自動** (`#[derive(Tsify)]`) | 言語レベルで JS interop |
| String 受け渡し | Linear Memory + TextEncoder | serde-wasm-bindgen で自動 | GC 統合で直接 |
| バイナリサイズ | 小さい（GC なし）| 同左 | WASM-GC ランタイム依存 |
| エコシステム成熟度 | **◎** | **◎** | △（1.0 未リリース） |
| 推奨 | バック専用 WASM | **フルスタック共有に最適** | 1.0 後に再評価 |

### 9.6 ワークスペース構成（§8 の拡張）

§8 のマルチワークスペース構成に domain-wasm crate を追加:

```
project-root/
├── backend/
│   ├── libs/
│   │   ├── domain/           ← 純粋。WASM 共有の対象
│   │   ├── domain-wasm/      ← wasm-bindgen ラッパー（薄い）
│   │   ├── usecase/
│   │   └── infrastructure/
│   └── apps/cli/
├── frontend/
│   ├── src/
│   │   ├── entities/         ← domain-wasm を import
│   │   ├── features/
│   │   └── shared/
│   └── package.json          ← "domain-wasm": "file:../backend/libs/domain-wasm/pkg"
└── harness.toml
```

### 9.7 Phase との関係

| Phase | 役割 |
|---|---|
| **1.5** | domain を純粋化（I/O なし）→ WASM 共有の**前提条件**が成立 |
| **STRAT-11** (多言語対応) | `domain-wasm` crate と `tsify` 導入。`harness.toml` に `wasm` workspace を追加 |
| **Phase 7** | `/track:auto` のフルスタック実装で domain 共有を実運用 |

**今すぐやる必要はない**が、Phase 1.5 の domain 純粋化が**将来この選択肢を自然に開く**。domain に I/O が残っている限り WASM 共有はできないため、1.5 の設計判断が正しいことの追加的な裏付けになる。

### 9.8 信号機との統合

WASM 共有された domain のバリデーションルールに信号機を付ける:

```markdown
## Domain States
| Entity | States | Signal | Source | Shared via |
|---|---|---|---|---|
| Email | valid / InvalidEmail / TooLong | 🔵 | [source: RFC 5321] | WASM |
| Severity | P0 / P1 / Low / Info | 🔵 | [source: team convention] | WASM |
| PR review state | Approved / ChangesRequested / Commented | 🟡 | [source: GitHub API docs] | WASM |
```

`Shared via` 列で「この型は WASM 経由でフロントエンドと共有される」ことを spec で明示。
フロントエンドの `acceptance_reviewer` は「spec に WASM 共有と書かれた型がフロントで実際に import されているか」を検証できる。
