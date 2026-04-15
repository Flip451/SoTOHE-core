# Catalogue active-track guard + rendered view source-file-name fix + sync_rendered_views multi-layer rollout

## Status

Accepted

## Context

### バグ観測と直接的な影響

2026-04-15、Track 1 (`domain-serde-ripout-2026-04-15`, status=done, PR #99 merged as `6f0d200`) 完了後の新 track 計画中に、main branch 上で以下のコマンドを実行した際に bug が顕在化した:

```
bin/sotp track type-signals domain-serde-ripout-2026-04-15 --layer infrastructure
```

このコマンドは既に merged 済 (status=done) の Track 1 の catalogue (`infrastructure-types.json`) と rendered view (`infrastructure-types.md`) を意図せず書き換えた。`infrastructure-types.md` のヘッダが `<!-- Generated from infrastructure-types.json -->` → `<!-- Generated from domain-types.json -->` に drift した。

ユーザー報告 (2026-04-15): 「`sotp track type-signals` が現在アクティブなトラック外でビューファイルを生成する」。

この drift は 3 つの構造的バグが組み合わさって発生している:

1. **`execute_type_signals` に active-track guard が存在しない** — `apps/cli/src/commands/track/tddd/signals.rs:96-128` は `TrackId::try_new` のパス走査チェックのみで、`metadata.json.status` が `Done` / `Archived` でも catalogue の write を許してしまう。merged track のデータ不変性が this コマンドから侵害される。
2. **`render_type_catalogue` のヘッダが hardcode** — `libs/infrastructure/src/type_catalogue_render.rs:64` が `"Generated from domain-types.json"` で固定されており、`doc` に layer 情報が含まれていないため、infrastructure / usecase layer の rendered view でも常に `"domain-types.json"` と表示される。
3. **`sync_rendered_views` が domain-only の domain-types.md しか render しない** — `libs/infrastructure/src/track/render.rs:568-606` は `track/items/<id>/domain-types.json` のみを対象に `domain-types.md` を生成する設計で、tddd-01 (domain 層 only TDDD) 時代の実装がそのまま残っている。tddd-02 (usecase 層 opt-in) / Track 1 (infrastructure 層 opt-in) の TDDD 多層化に **update が追従していない**。結果として `cargo make track-sync-views` を実行しても `usecase-types.md` / `infrastructure-types.md` は更新されず、`sotp track type-signals` コマンド経由でしか該当 layer の rendered view が refresh されない。

### 非対称性 (バグ 1)

同じ catalogue を write する別経路である `sync_rendered_views` (`libs/infrastructure/src/track/render.rs:573`) は既に `is_done_or_archived` guard を持ち、done/archived track の domain-types.md 生成を skip している。一方 `execute_type_signals` にはこの guard が無く、**同じ write 経路 (catalogue file + rendered md の atomic write) なのに protection level が違う**。

この非対称性は、`sync_rendered_views` が track lifecycle の自然な sync point として設計されたのに対し、`type-signals` コマンドが「on-demand 評価ツール」として設計された経緯の差異から生まれたと推測される。しかし結果として、merged track のデータ不変性が signals コマンドから侵害される。

### Multi-layer rendered view の sync gap (バグ 3)

`sync_rendered_views` は「track state を rendered view に反映する single 統合 sync point」として設計されているが、現状は domain-types.md だけを扱っている。tddd-02 以降の多層化で以下の drift が発生しうる:

- 開発者が `cargo make track-sync-views` を呼んだだけでは `usecase-types.md` / `infrastructure-types.md` が更新されない
- `sotp track type-signals --layer <usecase|infrastructure>` を呼び忘れると、catalogue (`<layer>-types.json`) と rendered view (`<layer>-types.md`) が drift する
- planner / reviewer が `usecase-types.md` / `infrastructure-types.md` を読んでも古い情報に基づく判断になる

この欠陥は D1 の active-track guard / D2 の hardcoded header と同じく catalogue rendering pipeline の構造的 bug であり、本 track で一括修正する。3 つのバグは密結合であり、別々の track に分離するよりも同 ADR で 1 つの theme (multi-layer catalogue rendering pipeline の整合) として扱うべき。

### Process lesson: SKILL.md SSoT との整合漏れ

本 bug の調査過程で、`.claude/skills/track-plan/SKILL.md` の 2 箇所に **古い記述** が残っていることが判明した:

- **line 165-167** の classification table に feedback を Blue 行として分類する行があった
- **line 283-285** の diff hearing update guidance に `feedback — {内容} を追加（→ Blue に昇格）` の記述があった

しかし既に `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md` (Accepted, 2026-04-12) で **feedback を Blue → Yellow に降格する** 設計判断がされ、`knowledge/conventions/source-attribution.md` (SSoT) は更新済 (table で feedback=Yellow 明記、Blue sources は document / convention のみ)。SKILL.md の更新だけが **漏れていた**。

この更新漏れは、本 track の plan 起草時に Claude Code が誤って「`feedback — ユーザー指摘` で Blue 化できる」と判断した誤認識の **直接の原因** となった。Plan 起草段階の spec signals では `blue=12 yellow=7 red=0` となり、strict merge gate を通過できない状態だった (ユーザー指摘により判明し修正)。

## Decision

### D1: `execute_type_signals` に status-based fail-closed guard を追加する

`apps/cli/src/commands/track/tddd/signals.rs::execute_type_signals` の先頭 (`TrackId::try_new` 検証直後、`resolve_layers` 呼び出し前) に以下の guard を追加する。疑似コード:

```rust
// Load track metadata to check lifecycle status.
// `read_track_metadata` returns `(TrackMetadata, DocumentMeta)` — destructure to get metadata.
let (metadata, _) = infrastructure::track::fs_store::read_track_metadata(&items_dir, &track_id)
    .map_err(|e| CliError::Message(format!("cannot load metadata for '{track_id}': {e}")))?;

// fail-closed + exhaustive: reject type-signals on completed/archived tracks.
// Exhaustive `match` on `TrackStatus` (6 variants) — adding a new variant
// triggers a compile error, forcing explicit classification (frozen vs active).
// `TrackStatus` implements `Display` (no `as_str()`) — use via format string directly.
use domain::track::TrackStatus;
match metadata.status() {
    TrackStatus::Done | TrackStatus::Archived => {
        return Err(CliError::Message(format!(
            "cannot run type-signals on '{track_id}' (status={}). \
             Completed tracks are frozen — run on an active track instead.",
            metadata.status()
        )));
    }
    TrackStatus::Planned
    | TrackStatus::InProgress
    | TrackStatus::Blocked
    | TrackStatus::Cancelled => {
        // active / pending — proceed with type-signals evaluation
    }
}
```

判断根拠:

1. **`sync_rendered_views` との対称性**: 同じ catalogue file + rendered view を触る別経路 (`libs/infrastructure/src/track/render.rs:573`) は既に `is_done_or_archived` guard を持っている。signals.rs 側にも同じ guard を置くことで、merged track のデータ不変性を複数経路から保護する。
2. **fail-closed**: guard が発動するケースは明示的な `CliError::Message` で reject し、silent skip や warning のみには留めない。merged track に対する write は **明確に誤った操作** であり、silent fail は bug を隠蔽する。
3. **layer 依存方向**: guard のロジックは `apps/cli` 層に置き、`infrastructure::track` の metadata loader を呼ぶ。domain は touch しない (domain → infrastructure の依存は禁止のまま)。
4. **exhaustive match で型安全な compile-time check**: `match metadata.status() { ... }` で `TrackStatus` の 6 variants (`Planned` / `InProgress` / `Done` / `Blocked` / `Cancelled` / `Archived`) を全て明示的に列挙する。`matches!` マクロは内部的に `_ => false` を暗黙展開するため非網羅的で新 variant を silent に通過させる (fail-open) が、exhaustive `match` 文は `_` を持たず `#[forbid(unreachable_patterns)]` 下でも全 variant を網羅的に要求するため、新 variant 追加時に **compile error** が発生し、開発者は「frozen 扱い (reject arm)」と「active 扱い (proceed arm)」のどちらに分類するか明示的に選択せざるを得ない。これは fail-closed を維持するための structural guarantee であり、将来 `TrackStatus` に新 variant (例: `Pending`, `Superseded`, `Deprecated`) が追加されても guard の挙動が silent に変わらない。なお既存 `libs/infrastructure/src/track/render.rs:505` の `is_done_or_archived` は `matches!(parsed.status.as_str(), "done" | "archived")` という文字列ベースの非網羅的実装であり、本 track では signals.rs 側のみ enum 型を使った type-safe 版として実装する。render.rs 側の同種 upgrade は scope 外 (Consequences Neutral で deferred 扱いと記載、将来別 sub-track で対応)。

### D2: `render_type_catalogue` の signature を `(doc, source_file_name: &str)` に変更する

`libs/infrastructure/src/type_catalogue_render.rs::render_type_catalogue` の signature を以下に変更する。

```rust
pub fn render_type_catalogue(
    doc: &TypeCatalogueDocument,
    source_file_name: &str,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "<!-- Generated from {source_file_name} — DO NOT EDIT DIRECTLY -->\n"
    ));
    // ... 以降の section rendering 部分は変更なし
}
```

呼び出し側を更新する (D3 の multi-layer loop 化と連動):

- **`apps/cli/src/commands/track/tddd/signals.rs::validate_and_write_catalogue` (line 347 付近)**: `render_type_catalogue(doc)` → `render_type_catalogue(doc, catalogue_file)` (`catalogue_file` は `domain_types_path.file_name()` から導出した catalogue JSON のファイル名 — e.g. `infrastructure-types.json`。`binding` は `validate_and_write_catalogue` のスコープに存在しないため `domain_types_path` から derive する。`rendered_file_stem` は rendered markdown のパス — e.g. `infrastructure-types.md` — であり source_file_name には渡さない)
- **`libs/infrastructure/src/track/render.rs::sync_rendered_views` 内 (D3 で multi-layer loop 化された後)**: loop 内で `render_type_catalogue(doc, binding.catalogue_file())` を呼ぶ。layer 毎に catalogue_file が変わるので hardcode は発生しない

既存 test 2 件 (`type_catalogue_render.rs:211`, `render.rs:1965`) を signature 変更に追従させ、新規 test (non-domain source file 名、例: `infrastructure-types.json`) を追加する。

判断根拠:

1. **Single Source of Truth**: 複数 layer で共通に使われる renderer のヘッダは、caller が source file 名を知っているべき (layer-aware な情報は呼び出し側)。`"domain-types.json"` hardcode は layer に中立でなく、SSoT が呼び出し側にあるべきという原則に反する。
2. **Backward compat 不要**: workspace 内の呼び出し側は 2 箇所のみで、一括修正可能 (`rg 'render_type_catalogue\('` で確認済)。breaking change のコストは limited。
3. **D3 との密結合**: `render_type_catalogue` の引数化と `sync_rendered_views` の multi-layer 化は同じ PR で完結させる必要がある (D3 の正当化と Rejected Alternative B4 を参照)。

### D3: `sync_rendered_views` を multi-layer 対応に拡張する

`libs/infrastructure/src/track/render.rs::sync_rendered_views` の line 568-606 (現行 domain-types.md 専用ブロック) を `architecture-rules.json` の `tddd.enabled=true` 全 layer を iterate するループに置き換える。

疑似コード:

```rust
// Resolve tddd-enabled layer bindings from architecture-rules.json.
// Implementation may live as a private helper in render.rs or (preferably) in
// infrastructure::verify::architecture_rules as a shared resolver.
let bindings = resolve_tddd_layer_bindings(root)?;

for binding in &bindings {
    let catalogue_file = binding.catalogue_file(); // e.g. "usecase-types.json"
    let catalogue_path = track_dir.join(catalogue_file);
    if is_done_or_archived || !catalogue_path.is_file() {
        continue;
    }
    let content = std::fs::read_to_string(&catalogue_path)?;
    match catalogue_codec::decode(&content) {
        Ok(doc) => {
            let rendered = type_catalogue_render::render_type_catalogue(&doc, catalogue_file);
            let rendered_md_path = track_dir.join(binding.rendered_file()); // e.g. "usecase-types.md"
            let old_md = match std::fs::read_to_string(&rendered_md_path) {
                Ok(c) => Some(c),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(RenderError::Io(e)),
            };
            if old_md.as_deref().is_none_or(|existing| !rendered_matches(existing, &rendered)) {
                atomic_write_file(&rendered_md_path, rendered.as_bytes())?;
                changed.push(rendered_md_path);
            }
        }
        Err(catalogue_codec::TypeCatalogueCodecError::Json(_)) => {
            eprintln!(
                "warning: skipping {} render for {} (malformed JSON)",
                binding.rendered_file(),
                track_dir.display()
            );
        }
        Err(e) => {
            return Err(RenderError::Io(std::io::Error::other(format!(
                "{} error at {}: {e}",
                catalogue_file,
                track_dir.display()
            ))));
        }
    }
}
```

判断根拠:

1. **track-sync-views の完全性**: `track-sync-views` は「track state を rendered view に反映する single 統合 sync point」として設計されたが、現状 domain-types.md だけを扱っていたため multi-layer track で partial sync になっていた。本修正で `track-sync-views` が full sync point として機能する。
2. **既存 3 pattern の保持**: `is_done_or_archived` guard、`rendered_matches` drift check、`TypeCatalogueCodecError::Json` warn-and-continue の 3 pattern は loop 内の各 layer に個別適用する。既存 domain-types.md の挙動は完全互換。
3. **per-layer opt-out の尊重**: `catalogue_path.is_file()` check で layer の catalogue file が不在なら skip。domain-serde-ripout Track 1 の「domain-types.json を作らない per-layer opt-out」pattern (ADR 2026-04-14-1531 §D8) を壊さない。
4. **layer binding 解決の置き場所**: `resolve_tddd_layer_bindings` は `libs/infrastructure/src/track/render.rs` 内に private helper として新設する。`apps/cli/src/commands/track/tddd/signals.rs::resolve_layers` は既に architecture-rules.json を parse しており、そのロジックを参考に infrastructure 層に独立実装する (apps/cli → infrastructure の依存は禁止のため cli 側から import 不可)。
5. **test coverage**: 既存 test `sync_rendered_views_generates_domain_types_md_from_domain_types_json` は維持。新 test `sync_rendered_views_generates_usecase_types_md_from_usecase_types_json` と `sync_rendered_views_generates_infrastructure_types_md_from_infrastructure_types_json` を追加し、domain 以外の layer で loop が正しく動作することを検証する。加えて「1 track に複数 catalogue file が存在するケース」test も追加し、loop の独立性を保証する。
6. **atomic な D2 + D3 修正**: D2 で render_type_catalogue signature を引数化した時点で、sync_rendered_views 内の `render_type_catalogue(doc)` 呼び出しが引数を渡す必要があり、どのみち render.rs を touch する。同じタイミングで loop 化まで進めるのが自然で、render.rs を 2 度 touch する必要がない。

### D4: `.claude/skills/track-plan/SKILL.md` の 2 箇所を SSoT に整合させる

本 bug の plan 起草時に feedback=Blue と誤認識した structural root cause として、SKILL.md の 2 箇所を修正する:

- **line 165-167**: classification table の Blue 行から `feedback` を削除し、Yellow 行に移動する
  - 修正前: `| 🔵 確定済み | 最高信頼度の source が document / feedback / convention → Blue signal | 不要（スキップ） |`
  - 修正後: `| 🔵 確定済み | 最高信頼度の source が document / convention → Blue signal | 不要（スキップ） |`
  - Yellow 行に feedback を追加: `| 🟡 要確認 | 最高信頼度の source が feedback / inference / discussion → Yellow signal | 確認を推奨 |`
- **line 283-285**: diff hearing update guidance で `feedback — {内容} を追加（→ Blue に昇格）` を修正
  - 修正前: 「`feedback — {内容}` を追加（→ Blue に昇格）」
  - 修正後: 「`feedback — {内容}` を追加（Yellow のまま保持。Blue 昇格には ADR/convention への永続化が必要 — source-attribution.md §Upgrading Yellow to Blue 参照）」

判断根拠:

1. **SSoT との整合**: `knowledge/conventions/source-attribution.md` が source tag signal の SSoT (§Source Tag Types Table で feedback=Yellow 明記)。SKILL.md は 2026-04-12 `strict-signal-gate-v2` ADR の実施時に更新漏れがあり、2026-04-15 の本 track で誤認識を誘発した。
2. **Structural fix for future**: この修正を行わない限り、将来 `/track:plan` skill を使う度に同種の誤認識が起きうる。bug fix と同 track で処理することで、関連する全修正を 1 PR にまとめる。
3. **歴史 track は触らない**: `track/items/diff-hearing-2026-03-27/`, `track/items/signal-evaluation-2026-03-23/` などの merged/done track にも古記述が残っているが、これらは当時の決定の歴史記録として残すのが正しい (git blame の尊重)。

> **Note**: 本 ADR の ADR-first gate (本 ADR Accepted までは Phase B 実装に着手しない) は architectural decision ではなく **process constraint** であり、spec.json の `constraints` フィールドで記録する (本 track `spec.json::constraints[ADR-first gate]`)。ADR は architectural decisions の記録に限定し、運用制約を ADR に書いて重複させない。

## Rejected Alternatives

### B1: Fix B — `--read-only` / `--dry-run` フラグを `sotp track type-signals` に追加する

過去 track の signal を再計算したい use case に対応するため、`--read-only` flag で signals 計算のみ行い catalogue write を skip する案。

**却下理由**:

1. **YAGNI**: merged track の signals は strict merge gate (`verify-spec-states`) により **必然的に全 blue** で main に入る。再計算しても情報獲得にならない。
2. **既に rendered view に永続化**: 計算結果は `<layer>-types.md` の Signal 列に既に埋め込まれて git history に残っている。view を `head -20` で見れば現状の signals が分かるため、再実行の必要性自体が存在しない。
3. **ユーザーからの補強** (2026-04-15): 「過去 track のシグナルはビューファイルを見ればわかるし、全部青確定なんだから」。

### B2: Fix C — current git branch と track.branch の一致を強制する guard

active track の定義として「現在の git branch が track.branch と一致する」という更に厳密な条件を課す案。

**却下理由**:

1. **D1 (status guard) が本 bug の core protection を完全カバーする**: root cause は「done/archived track への write が許される」ことで、D1 の status guard がこれを完全に拒否する。branch match は D1 の上に重ねる defense-in-depth に過ぎず、新しい protection value は限定的 — active track 同士の cross-track 誤操作 (例: track-A 作業中に track-B に type-signals を誤指定) のみを追加で防ぐ。
2. **「他 track 状態確認」は track artifact 直読で代替可能 (本 guard が workflow を阻害しない根拠)**: 過去 track の signals / catalogue を見たい場合、`cat track/items/<id>/<layer>-types.md` 等で rendered artifact を直接読めば十分である (strict merge gate で merged track は全 blue 確定、signals は rendered view に永続化済)。type-signals コマンド自体の実行は不要なので branch match guard を導入しても「他 track 状態確認」 workflow は阻害されない。したがって B2 採用/却下の判断は「阻害するか否か」ではなく「追加 protection の価値 vs 複雑度増」の純粋な cost/benefit に帰着する。
3. **Over-engineering の回避 (cost/benefit 判断)**: active track 同士の cross-track 誤操作は現実的には rare で、`cargo make track-branch-switch` の明示運用により回避されている。rare case の defense-in-depth のために追加の metadata load + `track.branch` field 読取 + current branch 取得 + 比較 という複雑度を払うのは本 track の scope からすると over-engineered (`.claude/rules/10-guardrails.md` §Small task commits 原則)。workflow-level の mistake が観測された段階で Reassess When §7 に従い再評価する。

### B3: hardcoded header を `#[cfg(test)]` assertion で detect する (D2 代替)

SKILL.md 修正の代替として、test 側で hardcoded header を検出する assertion を追加する案。

**却下理由**:

1. **対症療法**: hardcoded が何故生まれたか (API design の欠陥) を根本修正しない。
2. **signature 変更の方が cheaper**: 2 箇所の呼び出し側を更新するだけで終わる小さい変更。
3. **再発しない保証**: signature 変更後はコンパイル時に call site が強制的に引数を渡す必要があり、regression が structurally 不可能。

### B4: `sync_rendered_views` の multi-layer 対応を別 track に分離する (D3 代替)

D1 (active-track guard) と D2 (render_type_catalogue signature 変更) だけを本 track の scope とし、`sync_rendered_views` の multi-layer 化は別 track (例: `sync-views-multi-layer-rollout-*`) で扱う案。

**却下理由**:

1. **D2 との密結合**: `render_type_catalogue` signature を引数化した時点で、呼び出し側 (sync_rendered_views) の `"domain-types.json"` hardcoded 回避が自然に必要になる。別 track にすると signature 変更後に一時的な `render_type_catalogue(doc, "domain-types.json")` hardcode が残る中途半端な状態が生じる。
2. **現状 domain-only 挙動自体がバグ**: 「他 layer を render しない」のは設計ミス/追従漏れで、D1 / D2 と同じく catalogue rendering pipeline の構造的欠陥。同じ ADR の 1 つの theme として扱うべき (ユーザー指摘: 「sync_rendered_views が domain-types.md だけをレンダーするのはバグです。このトラックで直しましょう」)。
3. **scope overhead**: 別 track を作ると artifacts / ADR / review サイクルを repeat する必要があり、fix の実変更 (~80-150 行) に対してオーバーヘッドが大きい (small-task commit 原則の小サイズ目安 < 500 行の範囲内)。
4. **Render pipeline の atomicity**: D1-D3 は catalogue rendering pipeline の 3 つの構造的 gap であり、1 つずつ別々に fix すると pipeline の挙動が中途半端に変わり、reviewer / 使用者が「この PR 時点では何が動くか」を追跡するのが難しい。atomic な fix で 1 PR にまとめる方が整合性が高い。

### B5 (旧 B4): SKILL.md 修正を別 track に分離する

bug fix scope を pure code fix に限定し、SKILL.md の structural fix は別 track で処理する案。

**却下理由**:

1. **因果関係が強い**: SKILL.md 古記述は本 bug の plan 誤認識の直接原因で、同一 PR に含めることで「なぜこの修正が必要か」が明確になる。別 track では reader が文脈を再構築する必要がある。
2. **Small-task commit 原則の例外**: 通常は scope 分離が望ましいが、本ケースは修正 1 件あたり ~5 行の cosmetic diff であり、追加で別 track artifacts を作成するコストの方が大きい。
3. **将来のレビュー誘導**: 同 track で修正することで、将来同種の SSoT drift を発見した reviewer が「2026-04-15 ADR のように同 track で fix する」という precedent を持つ。

## Consequences

### Good

- **データ不変性の回復**: merged/archived track の catalogue ファイルが type-signals 経由で書き換えられる経路が塞がれる。`sync_rendered_views` と signals.rs の protection level が揃う。
- **rendering の帰属情報整合**: rendered view のヘッダが実際の source file 名を反映するようになり、読み手の混乱が解消される。
- **`track-sync-views` の完全性**: multi-layer rendered view の sync が `cargo make track-sync-views` 経由で一括処理される。これまで `usecase-types.md` / `infrastructure-types.md` は `sotp track type-signals` 経由でしか update されず、開発者が signals を呼び忘れると view が drift する設計上の gap が解消される。
- **SSoT との整合**: `.claude/skills/track-plan/SKILL.md` が `source-attribution.md` と一致し、plan 起草時の feedback=Blue 誤認識が再発しない。
- **ADR-first 原則の実践**: Track 1 §D1 の教訓を引き継いだ ADR-first 手順で bug fix を行い、プロセス違反を repeat しない。

### Bad

- **breaking change**: `render_type_catalogue` の signature 変更で既存呼び出し側 (2 箇所) の一括修正が必要。workspace 内なので影響範囲は限定的だが、将来このレンダラーを外部から呼ぶケースが増えた場合は同じ breaking change が再度必要になる。
- **non-atomic 修正**: bug fix (D1 signals.rs + D2/D3 render.rs code) と SKILL.md structural fix (D4) を同 track にまとめたため、PR diff が 4 つの密接に関連する修正を含む。reviewer は全体を読む必要がある (B5 で justified)。
- **sync_rendered_views の処理コスト増**: 各 layer の catalogue file を decode + render する iteration が必要。現 workspace (3 layer) では追加コスト数 ms 程度で実用上の impact は無視できる。layer 数が 10+ に増えた場合は parallel iter 化を検討する (Reassess When §2)。

### Neutral

- 既存の破損 rendered view (tddd-02 `usecase-types.md`, Track 1 `infrastructure-types.md`) のヘッダ復旧は本 track のデータ修復タスク (T005) で手動 Edit で行う。これらは status=done なので `sync_rendered_views` の `is_done_or_archived` guard で自動復旧されず、一時的に guard bypass の手動更新が必要。git 履歴には破損 → 修復の diff が残る。
- `resolve_tddd_layer_bindings` の重複実装: 本 track では `apps/cli::resolve_layers` と同等のロジックを `libs/infrastructure::track::render` 内に独立実装する。将来的に共通化する場合は `infrastructure::verify::architecture_rules` に shared resolver を移す候補 (本 track scope 外)。
- 既存 `libs/infrastructure/src/track/render.rs:505` の `is_done_or_archived = matches!(parsed.status.as_str(), "done" | "archived")` は文字列ベース非網羅的実装だが、本 track では signals.rs 側の D1 guard のみを exhaustive `match` on `TrackStatus` enum にアップグレードする。render.rs 側の同種 upgrade (文字列 match を enum 型に置換) は scope 外で deferred、将来別 sub-track (例: `render-guard-enum-upgrade-YYYY-MM-DD`) で対応する。signals.rs 側だけ先行する根拠: (a) 本 track の直接的 root cause が signals.rs の guard 欠如、(b) render.rs 側の文字列 `matches!` は metadata codec の raw string field に依存しており upgrade には metadata parser の touch が必要で scope creep を招く、(c) 両経路の guard は意味的に等価なので一方が強化されれば merged track 保護は確保される。

## Reassess When

1. **`architecture-rules.json` の `tddd.enabled` 判定ロジックが変わる場合**: 例えば per-track override / feature flag が導入された場合。本 track の `resolve_tddd_layer_bindings` 実装は `architecture-rules.json` 直接 parse に依存しており、判定ロジック変更時は loop の binding source を update する必要がある。
2. **layer 数が 10+ に増えた場合**: `sync_rendered_views` の iteration cost がボトルネックになった場合、parallel iter (`rayon::par_iter`) 化を検討する。現 workspace (3 layer) では impact 無視可能。
3. **catalogue write 経路が他にも追加される場合**: 例えば `baseline-capture` / `design` / 新 subcommand が catalogue を write するようになったら、それぞれに active-track guard を追加する。本 track は `execute_type_signals` のみを対象とし、他の write 経路は out-of-scope (別 sub-track に切り出し)。
4. **TrackStatus enum に新 variant が追加される場合**: D1 の guard は exhaustive `match metadata.status() { ... }` で `TrackStatus` の 6 variants (`Planned` / `InProgress` / `Done` / `Blocked` / `Cancelled` / `Archived`) を全て明示列挙しており、新 variant が追加されると **compile error** が発生する (match arms は全 variant を網羅的に要求)。開発者は compile error に従って新 variant を frozen 側 (`Done | Archived` と同じ reject arm) か active 側 (`Planned | InProgress | Blocked | Cancelled` と同じ proceed arm) に分類する必要がある。これにより fail-closed な挙動が structurally 保証される。既存 `render.rs:505` の文字列ベース `matches!` は本 track scope 外で upgrade されないため、別 sub-track での refactor 時に同じ exhaustive match pattern を適用する。
5. **feedback 再昇格の要求**: 2026-04-12 `strict-signal-gate-v2` ADR §Reassess When にあるように、feedback を Blue に再昇格する設計判断が将来なされた場合、SKILL.md / source-attribution.md の両方を同時に更新する必要がある。本 ADR の D4 修正はその時点で obsolete になる。
6. **`resolve_tddd_layer_bindings` の共通化**: `apps/cli::resolve_layers` と `libs/infrastructure::track::render::resolve_tddd_layer_bindings` が重複する状態は Neutral として許容するが、将来 3 箇所目の resolver が必要になった場合は `infrastructure::verify::architecture_rules` 等の shared module に抽出する。
7. **B2 (branch match guard) の採用を再評価する条件**: active track 同士の cross-track 誤操作 (例: track-A 作業中に track-B に type-signals を誤指定) が実際に観測された場合、D1 の上に branch match guard を defense-in-depth として追加する。現時点では rare case と判断し却下しているが、workflow-level の mistake が increase した場合は `metadata.branch` と current git branch の一致チェックを追加 protection として導入することを検討する。

## References

- **`knowledge/adr/2026-04-14-1531-domain-serde-ripout.md` §D1** — ADR-first 原則の source (プロセス違反の教訓)
- **`knowledge/adr/2026-04-14-1531-domain-serde-ripout.md` §D8** — per-layer opt-out pattern (domain-serde-ripout Track 1 で確立)
- **`knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md` §問題 2 / §Reassess When** — feedback=Yellow 降格の根拠
- **`knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md`** — TDDD 多層化の parent ADR
- **`knowledge/conventions/source-attribution.md` §Source Tag Types Table / §Upgrading Yellow to Blue** — source tag SSoT
- **`knowledge/conventions/hexagonal-architecture.md`** — layer 依存方向制約
- **`.claude/rules/10-guardrails.md` §Small task commits** — small commit 原則 (本 track は 4 関心事を同 PR にまとめる例外として B4 / B5 で正当化済)
- **`libs/infrastructure/src/track/render.rs:573` `is_done_or_archived` guard pattern** — D1 の reference implementation
- **`libs/infrastructure/src/track/render.rs:568-606` `sync_rendered_views` domain-only loop** — D3 の修正対象
- **`apps/cli/src/commands/track/tddd/signals.rs:96-128` `execute_type_signals`** — D1 の修正対象
- **`apps/cli/src/commands/track/tddd/signals.rs::resolve_layers`** — D3 `resolve_tddd_layer_bindings` の参考実装
- **`libs/infrastructure/src/type_catalogue_render.rs:64` hardcoded header** — D2 の修正対象
- **`architecture-rules.json` §layers `tddd.enabled`** — D3 の layer iteration 基準
- **`.claude/skills/track-plan/SKILL.md:165-167, :283-285`** — D4 の修正対象
