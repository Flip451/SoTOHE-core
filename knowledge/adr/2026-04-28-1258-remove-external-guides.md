---
adr_id: 2026-04-28-1258-remove-external-guides
decisions:
  - id: D1
    user_decision_ref: "chat_segment:adr-add-remove-external-guides:2026-04-28"
    candidate_selection: "from:[A,B,C] chose:D1-draft"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:adr-add-remove-external-guides:2026-04-28"
    status: proposed
---

# external_guides 撤去 — Python migration roadmap Phase 3 supersede

## Context

`scripts/external_guides.py` は当初 agent-router フックから `find_relevant_guides_for_track_workflow()` 経由で auto-injection されることで価値を提供していた。`knowledge/adr/2026-04-08-1200-remove-agent-router-hook.md` の決定で agent-router フックが撤去された結果、external_guides の主要 caller が消失し、現在は以下の用途のみが残っている:

- `cargo make guides-{list,fetch,usage,setup,clean,add}` の手動 CLI 経由
- `/guide:add` command (`.claude/commands/guide/add.md`) から呼び出される interactive 登録
- `knowledge/external/guides.json` registry(現時点で entry 1 件: `harness-design-long-running-apps`)

agent-router 削除後に external guide が AI session で活用された形跡は乏しい。一方で `external_guides.py` は次の依存連鎖を block している(`knowledge/adr/2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md` が Phase 3 として「Rust 化」を計画していた範囲):

- `scripts/atomic_write.py`(`external_guides.py` が現存する唯一の Python caller — Roadmap ADR が言及する `track_registry.py` は Phase 1 で削除済み、`scripts/track_registry.py` は実体が存在しない)
- `scripts/track_resolution.py::latest_legacy_track_dir()`(同上)
- `scripts/track_schema.py` 周辺の Phase 3 削除も間接 block
- `knowledge/external/POLICY.md` + `guides.json` registry SSoT

並行して、複数 worktree での並列開発環境整備(別 ADR で扱う)に着手する前提として、これら dead 依存連鎖を整理することで survey scope と blocker list が大幅に縮小する。

外部 guide 機能を後日改めて必要とする場合、Claude Code の WebFetch / WebSearch による直接参照、もしくは小規模な Rust 実装 (`bin/sotp guide` 等)で再導入できるため、本撤去は不可逆ではない。

## Decision

### D1: external_guides 機能の撤去と関連 Python helper の連鎖削除

external guide registry 機能 (`scripts/external_guides.py` の HTTP fetch / list / setup / add 等)を撤去し、依存していた Python helper を連鎖削除する。具体的削除対象:

- **機能本体**: `scripts/external_guides.py` + `scripts/test_external_guides.py`
- **registry SSoT**: `knowledge/external/POLICY.md`, `knowledge/external/guides.json`, `knowledge/external/` ディレクトリ自体
- **連鎖削除 Python helper**:
  - `scripts/atomic_write.py` + `scripts/test_atomic_write.py` — `external_guides.py` が現存する唯一の Python caller (Roadmap ADR が言及する `track_registry.py` は既に削除済み)
  - `scripts/track_resolution.py::latest_legacy_track_dir()` 関数 — 同上(`track_resolution.py` / `track_schema.py` 全体の扱いは impl 時に再評価し、test-only に成り下がる場合は同 track 内で完全削除する)
- **Makefile.toml**:
  - `[tasks.guides-list]` / `[tasks.guides-fetch]` / `[tasks.guides-usage]` / `[tasks.guides-setup]` / `[tasks.guides-clean]` / `[tasks.guides-add]` (`Makefile.toml:27-55`)
  - `[tasks.guides-selftest]` / `[tasks.guides-selftest-local]` (`Makefile.toml:97-107`)
  - `[tasks.scripts-selftest-local]`(`Makefile.toml:109-123`) の args から `scripts/test_atomic_write.py` / `scripts/test_external_guides.py` を除去(`scripts/test_track_resolution.py` は `track_resolution.py` 全体削除時に除去)
- **skill / command**: `.claude/commands/guide/add.md`(`/guide:add` slash command の実体 — `.claude/skills/` 側に対応定義はない)
- **doc 参照**:
  - `CLAUDE.md` の `knowledge/external/POLICY.md` / `knowledge/external/guides.json` 参照行
  - `.claude/rules/09-maintainer-checklist.md` の `scripts/external_guides.py` 参照
  - grep で発見したすべての他参照

Rust 側の `infrastructure::track::atomic_write_file` は別物で、`bin/sotp file write-atomic` CLI として現役で使われているため本撤去対象外。

### D2: Roadmap ADR Phase 3 の supersede

`knowledge/adr/2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md` は Phase 3 で external_guides + 関連 helper を「Rust 化」する計画を提示していたが、本 ADR の D1 で external_guides 機能そのものを撤去するため、Phase 3 の前提が消失する。本 ADR で Phase 3 の方向性を「Rust 化」から「機能撤去」へ転換し、Roadmap ADR の該当 decision を **supersede** する。

具体的な処置(impl 時に adr-editor 経由で実施):

- Roadmap ADR 本文の Phase 3 セクションに、本 ADR (`2026-04-28-1258-remove-external-guides.md`) で Phase 3 の方向性が「Rust 化」から「機能撤去」へ転換された旨の back-reference note を追記する(convention `adr.md` §Lifecycle が post-merge ADR に許容する「newer ADR への back-reference 追加」に該当)
- Roadmap ADR の YAML front-matter は **変更しない**。同 ADR の `decisions[]` は単一の grandfathered entry (`status: accepted`) で構成されており、Phase 3 に特化した独立 entry は存在しない。front-matter の semantic 変更 (`status: superseded` への書き換え / `superseded_by` の追記) は post-merge ADR に許容されない編集範囲を超えるため実施しない
- Phase 3 の supersede 意図は本 ADR の `decisions[]` D2 entry に encode されており、これで記録として十分

Roadmap ADR は post-merge(commit 履歴あり)のため、本 ADR で supersede する形を取る(convention `adr.md` §Lifecycle の post-merge 編集ルールに従う)。

## Rejected Alternatives

### A. external_guides を Rust 化(Roadmap ADR Phase 3 の元案)

Roadmap ADR が当初提示していた選択肢。Python helper を Rust 側に移植し `bin/sotp guide` 等のサブコマンドとして残す。

**却下理由**: agent-router 削除以降、external guide が AI session で活用された形跡が乏しく、Rust 化のメリットが薄い。HTTP fetch / retry / registry 管理などのロジックを再実装するコストが利用実態に見合わない。手動 CLI 用途であれば Claude Code の WebFetch / WebSearch で代替できる。

### B. 現状維持(external_guides を Python のまま残す)

auto-injection なしの手動 CLI 用途として `external_guides.py` を維持し、registry 機能を温存する。

**却下理由**: 主要 caller の agent-router が削除済みで registry の価値が著しく低下。registry 維持コストおよび依存連鎖 (`atomic_write.py` / `track_resolution.py::latest_legacy_track_dir` / `track_schema.py` Phase 1-3 削除の block)が得られる価値を上回る。並行進行する worktree ADR の survey scope も dead 依存連鎖の影響で膨張する。

### C. auto-injection を別機構(skill / hook)で復活させて external_guides を保持

agent-router 削除後の代替として、新しい skill / hook 経由で external_guides の auto-injection を再導入し機能維持を正当化する。

**却下理由**: Claude Code の WebFetch / WebSearch が十分高速で ad-hoc な外部ドキュメント参照は AI session 内で完結できる。auto-injection を再構築するには新たな skill / hook 設計が必要で、コストに見合うだけの活用シナリオが見えない。需要が再燃した場合は別 ADR で再導入を検討する道を残す。

## Consequences

### Positive

- workspace 全体がシンプルになる: `scripts/` 配下の Python helper 数が減り、依存連鎖が clean state になる
- worktree 並列対応 ADR の survey scope が縮小: 連鎖削除により `latest_legacy_track_dir` 並列性懸念や `atomic_write.py` probe path 問題が消える
- Roadmap ADR Phase 3 の実装コストが消失: Rust 移植作業が不要になる
- registry 維持の負担消滅: external guide の追加 / 更新 / fetch を運用する必要がなくなる
- AI session の context 占有減: 実質未使用の機能を削除することで CLAUDE.md / 各 doc の参照行も整理される

### Negative

- 外部 long-form guide の registry 化された参照体系を失う: 必要な場合は AI session ごとに WebFetch / WebSearch を呼ぶ必要がある
- 既存 cache (`.cache/external-guides/` 等)が dead state になる: 利用者は手動で削除する必要がある
- `harness-design-long-running-apps` 等の既存 entry の registry 価値が失われる: 保持したい場合は別 doc (`knowledge/strategy/` 等)に手動移行する必要がある
- Roadmap ADR の機械検証可能な supersede signal を front-matter に encode できない: Roadmap の `decisions[]` は単一の grandfathered entry で構成されており Phase 3 専用 entry が存在しないため、supersede 関係は本 ADR の D2 本文と Roadmap 側の back-reference note でのみ表現される(`verify-adr-signals` は両 ADR を独立に Blue 評価し、関係性は narrative 記録のみ)

### Neutral

- `infrastructure::track::atomic_write_file` (Rust)は別物として残り、`bin/sotp file write-atomic` CLI も継続利用される
- `bin/sotp` の commit 系コマンドや track flow は影響を受けない

## Reassess When

- Claude Code の WebFetch / WebSearch では代替できない external long-form guide 参照需要が継続的に発生し、ad-hoc fetch のコストが registry 維持コストを上回った場合
- agent-router 相当の auto-injection 機構(skill / hook)が別形で復活し、registry 化された参照体系の価値が再認識された場合
- Roadmap ADR (`2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md`) の Phase 1-2 を全面的に見直す必要が生じ、Phase 3 を含む全体方針の再検討が必要になった場合
- 多人数開発体制への移行などにより external guide の共有 SSoT が必要になった場合

## Related

- `knowledge/adr/2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md` — 本 ADR の D2 で Phase 3 を supersede する対象
- `knowledge/adr/2026-04-08-1200-remove-agent-router-hook.md` — external_guides の主要 caller を撤去した先行 ADR
- `knowledge/conventions/pre-track-adr-authoring.md` — ADR 配置・lifecycle ルール
- `knowledge/conventions/adr.md` — ADR YAML front-matter / decision status schema
