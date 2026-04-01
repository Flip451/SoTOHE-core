# knowledge/ ディレクトリ再編 + ADR 導入 設計メモ

> **作成日**: 2026-03-20
> **ステータス**: 設計完了、トラック化待ち
> **出典**: HARNESS-05 (ADR 導入) + DESIGN.md 分解 + `.claude/docs/` 許可問題の解消
> **関連 TODO-PLAN**: Phase 6 6-9 (HARNESS-05) を前倒し実施

---

## 動機

1. **ADR の必要性**: 設計判断の「なぜ」と「却下した選択肢」が体系的に残っていない
2. **DESIGN.md の問題**: AI が自然に作ったもので意図的な設計がない。Canonical Blocks（型シグネチャの複製）が全体の 2/3 を占め、コードと乖離するリスク
3. **`.claude/docs/` の許可問題**: `.claude/` 配下はハーネス設定領域。ドキュメント編集のたびに許可が必要になり体験が悪い
4. **ディレクトリの役割曖昧**: `docs/` は外部ガイド用、`project-docs/` は規約用、`.claude/docs/` は設計用と分散しており統一性がない

---

## 決定事項

### ADR テンプレート

- **フォーマット**: Nygard 式（Context / Decision / Consequences）
- **言語**: 日本語
- **採番**: `YYYY-MM-DD-HHMM-slug.md`（例: `2026-03-11-1430-track-status-derived.md`）

### ADR テンプレート（ファイル内容）

```markdown
# {タイトル}

## Status

Accepted / Superseded / Deprecated

## Context

{なぜこの判断が必要だったか}

## Decision

{何を選んだか}

## Rejected Alternatives

- {選択肢B}: {却下理由}
- {選択肢C}: {却下理由}

## Consequences

- Good: {良い影響}
- Bad: {悪い影響・トレードオフ}

## Reassess When

- {前提が変わる条件}
```

> Note: Rejected Alternatives と Reassess When は Nygard 原型にはないが、
> 先行議論で「却下理由」と「再評価条件」が最大のギャップと特定されたため追加。

### ADR と Convention の関係

| | ADR | Convention |
|---|---|---|
| 問い | 「なぜこうした？」 | 「これからどうする？」 |
| 時制 | 過去形（あの時点で判断した） | 現在形（今後はこうせよ） |
| 寿命 | 永続（superseded でも残る） | 現行ルールのみ有効 |
| 例 | 「conch-parser を選んだ。理由は...」 | 「shell パースは conch-parser を使え」 |

Convention に `## Decision Reference` セクションを追加し ADR にリンクする。

### ディレクトリ構成

```
knowledge/                       # NEW: プロジェクト知識体系の統合ルート
├── README.md                    # 索引 + 読み順
├── architecture.md              # DESIGN.md → スリム化（概要+図+ADR索引のみ）
├── WORKFLOW.md                  # .claude/docs/WORKFLOW.md から移動
├── adr/                         # NEW: 設計判断記録
│   ├── README.md                # テンプレート + 運用ルール + 索引
│   └── YYYY-MM-DD-HHMM-*.md
├── conventions/                 # project-docs/conventions/ から移動
│   ├── README.md
│   └── *.md
├── research/                    # .claude/docs/research/ から移動
│   └── *.md
├── designs/                     # .claude/docs/designs/ + schemas/ から統合移動
│   └── *.md
└── external/                    # docs/ の外部ガイド系から移動
    ├── POLICY.md                # docs/EXTERNAL_GUIDES.md
    └── guides.json              # docs/external-guides.json

architecture-rules.json          # docs/ からルートに移動（CI 設定ファイル寄り）
```

### 廃止されるディレクトリ

| 廃止 | 移動先 |
|---|---|
| `.claude/docs/DESIGN.md` | `knowledge/architecture.md` + `knowledge/adr/` に分解 |
| `.claude/docs/WORKFLOW.md` | `knowledge/WORKFLOW.md` |
| `.claude/docs/research/` | `knowledge/research/` |
| `.claude/docs/designs/` + `schemas/` | `knowledge/designs/` |
| `project-docs/conventions/` | `knowledge/conventions/` |
| `project-docs/` | 廃止（`knowledge/conventions/` に吸収） |
| `docs/EXTERNAL_GUIDES.md` | `knowledge/external/POLICY.md` |
| `docs/external-guides.json` | `knowledge/external/guides.json` |
| `docs/architecture-rules.json` | `./architecture-rules.json`（ルート） |
| `docs/README.md` | 廃止（`knowledge/README.md` に統合） |
| `docs/` | 廃止 |

### DESIGN.md の Canonical Blocks

- **削除**。コードが正。track の plan.md に歴史的記録として残る。

### DESIGN.md Key Design Decisions → ADR 分解

現在のテーブル（7行）を個別 ADR に分解する。具体的な分解リストはトラック計画時に DESIGN.md を読んで確定する。

---

## 実装方針

### 1 トラックで実施

1. `knowledge/` ディレクトリ構造の作成
2. ファイル移動（git mv）
3. DESIGN.md の分解（architecture.md + ADR 群）
4. Convention への `Decision Reference` セクション追加
5. ADR テンプレート + 運用ルールの convention 文書作成
6. 全参照パスの書き換え（CLAUDE.md, rules/, skills/, workflow.md 等）
7. `architecture-rules.json` のルート移動 + CI スクリプト更新
8. 旧パス検知の CI ガード追加（`Grep` で `.claude/docs/`, `project-docs/`, `docs/` の残存参照を検出）

### 参照更新の保証

- `Grep` で旧パス（`.claude/docs/`, `project-docs/`, `docs/`）を全ファイルから検索
- 残存参照があればトラック内で修正
- CI に旧パス残存検知を追加して再発防止

---

## CI ガード: 旧パス残存検知

### 現状の CI に足りないもの

現在の `cargo make ci` には以下の verify チェックがあるが、**いずれも「ドキュメント内のパス参照が有効か」を検査していない**:

- `verify-arch-docs`: architecture-rules.json と workspace 構造の乖離
- `verify-view-freshness`: plan.md / registry.md と metadata.json の乖離
- `verify-orchestra`: hooks, delegation, agent 定義の整合性
- `verify-canonical-modules`: canonical module の再実装検知
- `verify-domain-strings`: domain 層の pub String 検知
- その他: `verify-plan-progress`, `verify-track-metadata`, `verify-track-registry`, `verify-tech-stack`, `verify-module-size`, `check-layers`

つまり移動後に `.claude/docs/DESIGN.md` や `project-docs/conventions/` を参照している箇所が残っていても CI は素通りする。

### 追加するガード: `sotp verify doc-links`

旧パス専用の一時チェックではなく、**汎用的なドキュメントリンク存在チェック**として実装する。
移行後も永続的に価値がある（ファイルのリネーム・削除のたびにリンク切れは発生しうる）。

検査内容:
- Markdown ファイル内の相対パスリンク（`[text](path)`, `[text]: path`）のリンク先が実在するか
- CLAUDE.md, rules/, skills/ 等の参照リスト内のパスが実在するか

検査対象ファイル:
- `*.md`（主要）、必要に応じて `*.json`, `*.toml`
- `tmp/` と `.git/` と `target*/` は除外

実装方針:
- `sotp verify doc-links` として Rust サブコマンドに追加（他の verify と統一）
- `cargo make ci` の dependencies に `verify-doc-links-local` を追加
- 旧パスのハードコードは不要 — リンク先の実在性だけを検査するため、移行後もデッドコードにならない

---

## 未決事項

- [ ] DESIGN.md Key Design Decisions テーブル → ADR 分解の具体的なリスト（トラック計画時に確定）
- [ ] `knowledge/README.md` の読み順（トラック計画時に設計）
- [ ] specialist 出力先の更新（skills/ 内のプロンプトで `.claude/docs/research/` を指定している箇所）
- [ ] `.claude/docs/` ディレクトリ自体を残すか完全削除するか（空ディレクトリ or .gitkeep）
