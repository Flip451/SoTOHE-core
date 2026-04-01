# テンプレート過学習分析レポート

> **作成日**: 2026-03-23
> **起点**: vision v3 で「ハーネス実装 vs テンプレート出力」を分離したが、テンプレートの非 Rust 部分（CI, hooks, scripts, rules）が SoTOHE 自身に過学習しているのではないかという疑問
> **関連**: `tmp/vision-2026-03-22.md` (v3), `tmp/TODO-PLAN-2026-03-22.md` (v3)

---

## 1. 調査方法

3 つの並列調査エージェントで以下を分析:

1. **CI/Makefile 分析**: Makefile.toml の全 86 タスクを SoTOHE 固有 / 汎用 / 曖昧に分類。GitHub Actions CI, Dockerfile, compose も対象
2. **Hooks/Scripts 分析**: `.claude/settings.json` のフック登録、`.claude/hooks/*.py`、`scripts/*.py`、`.claude/rules/*.md` を分類
3. **構造分析**: テンプレート生成メカニズムの有無、`docs/architecture-rules.json`、`deny.toml`、`agent-profiles.json` の結合度

---

## 2. 定量結果: 過学習度

### 2.1 Makefile.toml（86 タスク）

| カテゴリ | タスク数 | % |
|---------|---------|---|
| **汎用テンプレート** | 37 | 43% |
| **SoTOHE 固有** | 43 | 50% |
| **曖昧** | 6 | 7% |

**汎用テンプレート（37）**: fmt, clippy, test, deny, machete, llvm-cov の local/compose/daemon 3 層ラッパー + Docker lifecycle（build-tools, up, down, shell 等）

**SoTOHE 固有（43）**: track-* ワークフロー操作（22）、sotp バイナリ管理（6）、verify-* チェック（7）、selftest（6）、guides/conventions 管理（6）

**曖昧（6）**: check-layers, verify-arch-docs, verify-domain-purity, verify-usecase-purity 等 — 概念は汎用だが実装が `sotp verify` 経由

### 2.2 Claude hooks

| カテゴリ | settings.json 登録 | hooks/*.py | 合計 |
|---------|-------------------|-----------|------|
| **SoTOHE 固有** | 9 | 8 | 17 |
| **汎用** | 3 | 3 | 6 |
| **曖昧** | 1 | 1 | 2 |
| **過学習率** | 69% | 67% | **68%** |

**汎用フック**: lint-on-save.py（rustfmt + cargo check）、python-lint-on-save.py（ruff）、_shared.py（汎用ユーティリティ）、PermissionRequest[Bash] インライン

**SoTOHE 固有フック**: agent-router.py（capability routing）、check-codex-before-write.py、suggest-gemini-research.py、error-to-codex.py、post-test-analysis.py、post-implementation-review.py、check-codex-after-plan.py、block-direct-git-ops（sotp）、block-test-file-deletion（sotp）— 全て `_agent_profiles.py` 経由で capability model に依存

### 2.3 Scripts

| カテゴリ | ファイル数 | 過学習率 |
|---------|----------|---------|
| **SoTOHE 固有** | 8 | **73%** |
| **汎用** | 2 | |
| **曖昧** | 1 | |

**SoTOHE 固有**: track_schema.py, track_markdown.py, track_state_machine.py, track_branch_guard.py, track_registry.py, track_resolution.py, external_guides.py, convention_docs.py

**汎用**: atomic_write.py, architecture_rules.py

### 2.4 Rules

| カテゴリ | ファイル数 | 過学習率 |
|---------|----------|---------|
| **SoTOHE 固有** | 6 | **55%** |
| **汎用** | 5 | |

**汎用**: 01-language.md, 04-coding-principles.md, 05-testing.md, 06-security.md, 11-subagent-model.md

**SoTOHE 固有**: 02-codex-delegation.md, 03-gemini-delegation.md, 07-dev-environment.md, 08-orchestration.md, 09-maintainer-checklist.md, 10-guardrails.md

### 2.5 CI / Docker

| 対象 | 過学習率 | 備考 |
|------|---------|------|
| `.github/workflows/ci.yml` | 10%（1/10 ステップ） | `cargo make ci-container` のみ SoTOHE 固有 |
| `Dockerfile` | ~5%（vendor/ COPY のみ） | ほぼ汎用 |
| `compose.yml` / `compose.dev.yml` | **0%** | 完全に汎用 |

### 2.6 総合

| レイヤー | 過学習率 |
|---------|---------|
| Makefile.toml | **50%** |
| hooks | **68%** |
| scripts | **73%** |
| rules | **55%** |
| CI/Docker | **~5%** |
| **加重平均** | **~55%** |

---

## 3. 根本原因

### 3.1 テンプレート生成メカニズムの不在

SoTOHE-core 自体がテンプレートであり、fork/clone して使うモデル。`cargo generate` やスキャフォールドコマンドは存在しない。そのため「テンプレートのインフラ」と「SoTOHE のインフラ」が物理的に同一ファイル上で混在している。

### 3.2 自己参照的ドッグフーディング

SoTOHE-core は自分自身を使って自分自身を開発している:
- `track/items/` に `statemachine-rust`, `shell-wrapper-rust`, `review-escalation-threshold` など sotp CLI 自体を構築したトラックが 30+ 件
- `libs/domain/` の track state machine が管理するのは、まさにその domain 自体の開発プロセス
- 「テンプレート」と「プロダクト」が物理的に同一リポジトリ

### 3.3 vision v3 との矛盾

vision v3 は投資比率を「ハーネス保守 20% + テスト生成ツール 40% + テンプレート品質 40%」と掲げるが、非 Rust 部分の実態:

```
SoTOHE 固有インフラ   ≈ 60%   ← ハーネス自身の品質管理
汎用テンプレートインフラ ≈ 30%   ← どのプロジェクトでも使える
テスト生成関連         ≈  0%   ← Phase 3 未着手
曖昧（sotp 経由の汎用概念）≈ 10%
```

---

## 4. 提案方針: sotp CLI 外部ツール化

### 4.1 基本方針

「テンプレート利用者は SoTOHE の正規ワークフロー（track, hooks, review cycle）を使う前提」とし、sotp CLI をスタンドアロンツールとして切り出す。テンプレート利用者向けの外枠は後から実装する。

### 4.2 分離後の構造

```
リポジトリ A: sotp (スタンドアロン CLI ツール)
├── libs/domain          # track state machine, guard, review 等
├── libs/usecase          # application services
├── libs/infrastructure   # adapters (fs, git, gh)
├── apps/cli              # sotp バイナリ
├── vendor/conch-parser   # vendored shell parser
└── 独自の CI / テスト

リポジトリ B: SoTOHE テンプレート (fork/clone して使う)
├── libs/domain          ← 空のスケルトン（ユーザーのコード用）
├── libs/usecase          ← 空のスケルトン
├── libs/infrastructure   ← 空のスケルトン
├── apps/cli              ← ユーザーの CLI エントリーポイント
├── Makefile.toml         ← track-* タスクは sotp を呼ぶだけ
├── .claude/              ← hooks, rules, commands, skills, agent-profiles
├── scripts/              ← Python helpers
├── docs/                 ← architecture-rules.json, deny.toml
├── track/                ← workflow.md, tech-stack.md, product.md
├── project-docs/conventions/
├── Dockerfile            ← sotp をインストール済み
└── compose.yml
```

### 4.3 解決される問題

| 問題 | 解決の仕方 |
|------|-----------|
| Cargo workspace の汚染 | テンプレートの workspace がユーザーのコード専用になる。sotp ソースが消える |
| 「曖昧」カテゴリの解消 | `sotp verify *` は `cargo-deny` と同じ外部ツール呼び出しの立ち位置に |
| CI の混在 | `ci` が sotp verify を呼ぶのは自然（外部ツール依存）。`ci-rust` との二重性も解消 |
| track/items/ の汚染 | テンプレートには空の `track/items/` だけ。SoTOHE の開発履歴はリポ A に残る |
| hooks の capability routing | そのまま維持。sotp ワークフローがテンプレートの価値そのもの |
| 過学習の定義変更 | hooks/rules/scripts の SoTOHE 固有部分は「sotp エコシステムの一部」となり、過学習ではなくなる |

### 4.4 新たに生じる課題

| 課題 | 深刻度 | 対策 |
|------|--------|------|
| **sotp の配布方法** | HIGH | `cargo install sotp`、GitHub Releases バイナリ、Dockerfile に `COPY --from=builder` |
| **bin/sotp → PATH 上の sotp** | MEDIUM | hooks/Makefile の `bin/sotp` 参照を `sotp` に変更。Dockerfile で PATH に配置 |
| **自己参照開発ループの喪失** | MEDIUM | リポ A は自分自身の sotp で開発可能（bootstrap: `cargo run -p cli` で回避） |
| **テンプレートと sotp のバージョン互換** | MEDIUM | `sotp --version` チェックを bootstrap に入れる。semver 互換性保証 |
| **2 リポ管理のオーバーヘッド** | LOW | 当面はモノリポ内ワークスペース分離でも可（物理リポ分割は後から） |

---

## 5. 段階的な実行計画

### Step 1: 論理分離（同一リポ内）

- Cargo workspace を 2 グループに分ける
  - sotp グループ: `libs/{domain,usecase,infrastructure}`, `apps/cli`
  - template グループ: `apps/server`（空スケルトン）
- Makefile.toml の `bin/sotp` 参照はそのまま
- **効果**: 概念的な境界が明確になる。コード変更は最小限

### Step 2: sotp を配布可能に

- sotp の Cargo.toml を publish 可能な状態に整備
- GitHub Actions で sotp バイナリをリリース
- Dockerfile に sotp インストールステップ追加
- **効果**: テンプレート利用者が sotp をビルドせずインストール可能に

### Step 3: リポ分割

- sotp を別リポに切り出し
- テンプレートリポから sotp ソースを削除
- テンプレートは sotp インストール済み前提に
- `bin/sotp` 参照を `sotp`（PATH）に一括置換
- **効果**: テンプレートの Cargo workspace が完全にユーザー専用に

### Step 4: テンプレート利用者向けの外枠

- `sotp init` / `sotp scaffold` コマンド（STRAT-11 多言語対応と合流）
- テンプレートカスタマイズ（言語、フレームワーク、レイヤー名）
- `/architecture-customizer` の発展形
- **効果**: fork/clone モデルからジェネレータモデルへの進化

---

## 6. vision v3 への影響

### 投資比率の再定義

```
v3 現在:   ハーネス保守 20% + テスト生成ツール 40% + テンプレート品質 40%
v3 改訂案: sotp CLI 開発 30% + テスト生成パイプライン 40% + テンプレートインフラ 30%
```

### Phase との対応

| Step | 対応 Phase | 備考 |
|------|-----------|------|
| Step 1 | Phase 1.5 延長 or 新 Phase | 既存リファクタリングと直交 |
| Step 2 | Phase 4（インフラ） | 配布インフラ |
| Step 3 | Phase 4 以降 | 物理分割 |
| Step 4 | Phase 3 以降 | STRAT-11 合流 |

### Phase 3 との整合

Phase 3 の BRIDGE-01（`sotp domain export-schema`）やテスト生成パイプラインは自然に sotp CLI 側のサブコマンドとして実装される。テンプレートはそれを呼び出すだけ。

---

## 7. 判断ポイント

1. **Step 1 の論理分離だけで十分か、Step 3 の物理分割まで行くか？**
   - 論理分離だけでも過学習問題の大部分は概念的に解決する
   - 物理分割は配布の利便性が動機（テンプレート利用者が sotp をビルドしなくてよい）

2. **Phase 1.5 の残りタスク（D, E, F, G）との優先順位**
   - CLI 品質改善は sotp 側の改善なので、先にやっても後にやっても問題ない
   - ただし Step 3（物理分割）前にやっておく方が移動コストが小さい

3. **TODO-PLAN v3 への反映タイミング**
   - Step 1 を Phase 1.5 に追加するか
   - 新 Phase（1.75?）として挿入するか
   - Phase 4 に統合するか

---

## 付録: 汎用テンプレートとして抽出可能なファイル一覧

### hooks（そのまま抽出可能）

- `.claude/hooks/lint-on-save.py`
- `.claude/hooks/python-lint-on-save.py`
- `.claude/hooks/_shared.py`

### scripts（そのまま or 最小変更で抽出可能）

- `scripts/atomic_write.py`（pure-Python fallback あり）
- `scripts/architecture_rules.py`（データ駆動、`architecture-rules.json` 依存のみ）

### rules（そのまま抽出可能）

- `.claude/rules/04-coding-principles.md`
- `.claude/rules/05-testing.md`
- `.claude/rules/06-security.md`
- `.claude/rules/11-subagent-model.md`

### settings.json パターン（そのまま抽出可能）

- `PermissionRequest[Bash]` インラインプロンプト（Read/Grep/Glob 強制）
- `TeammateIdle` ワークログリマインダー
- `PostToolUse[Edit|Write]` lint-on-save 登録

### Docker/compose（そのまま抽出可能）

- `Dockerfile`（vendor/ COPY 行を除く）
- `compose.yml`
- `compose.dev.yml`

### Makefile.toml の汎用タスク（37 タスク）

- Rust 品質: fmt, clippy, test, deny, machete, llvm-cov（local/compose/daemon 3 層）
- Docker lifecycle: build-tools, up, down, logs, ps, shell, clean
- 開発支援: bacon, bacon-test, check
