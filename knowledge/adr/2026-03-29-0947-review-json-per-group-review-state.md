# review.json 分離 + グループ独立レビュー状態

## Status

Superseded by 2026-04-04-1456-review-system-v2-redesign.md

## Context

`autorecord-stabilization-2026-03-26` の進行中に、現行の track-level review state model が運用前提と噛み合っていないことが明確になった。

問題は主に 4 つあった。

1. review state が `metadata.json` に混在しており、track 定義と review の運用状態が結合している
2. track-wide hash / status / round 同期を前提にしているため、ある review group が `zero_findings` を達成しても他 group の再試行に巻き込まれる
3. stale 判定が track 全体に 1 個の review hash をぶら下げる構造になっており、将来の per-group hash 前提と整合しない
4. parallel auto-record、planning-only 判定、policy 変更、scope drift が同じ `metadata.json.review` に集約され、surface area が増え続ける

運用上の前提は次のとおりである。

- review group は重複しない partition として扱いたい
- ある group が fast round 1 で `zero_findings` を取ったら、その group は他 group の再試行とは独立に停止できるべき
- `other` を必須化し、どの差分も必ずどこかの group に属するべき
- stale 判定は「自分の scope に変更が入ったか」で決めるべきで、他 group の再試行だけでは失効させたくない

この前提に合わせるには、review policy と review state を分離し、review state を group ごとの cycle/round 履歴として持ち直す必要がある。

## Decision

review state を `metadata.json` から分離し、`review.json` に group 単位の append-only 履歴として保存する。

### 1. `metadata.json` は track 定義だけを持つ

- `metadata.json` は task 状態・plan・track status などの SSoT に限定する
- review state は `metadata.json` に持たない

### 2. policy と state を分離する

- `track/review-scope.json` は canonical policy source として残す
- `review.json` は operational review state のみを持つ
- planning-only / review-operational / other-track / normalize などのルールは policy source に残す

### 3. review cycle 開始時に partition を固定する

- review 開始時に current policy から partition を導出する
- named groups に加えて mandatory `other` を必須とする
- cycle には `base_ref`、`policy_hash`、`groups[*].scope` を固定値として保存する
- cycle 中に policy 変更や scope drift が起きたら stale / invalidated とする

### 4. round は group ごとの append-only 履歴にする

- 各 group は `fast` / `final` round を独立に持つ
- round は append-only で保存し、古い round を削除しない
- ただし commit guard の評価対象は常に最新 cycle の latest successful round とする

### 5. hash は group ごとに持つ

- 各 round の hash は、その group の frozen scope に対して計算した hash とする
- track 全体で 1 個の review hash を共有しない
- stale 判定は `base_ref` + frozen scope + policy_hash を条件に current hash を再計算して行う

### 6. group 間 round 一致は要求しない

- `group1` の fast round 1 `zero_findings` は、`group2` が round 3 まで進んでも有効なままにする
- 失効するのは、その group 自身の scope hash が stale になったときだけ
- global round synchronization は導入しない

### 7. final は全 group 必須にする

- optional final group は設けない
- 各 expected group について latest successful final round を要求する

### 8. tamper-proof は別 ADR / 別トラックで扱う

- `review.json` 内の単純 checksum は tamper resistance の根拠にはならない
- verdict provenance / trusted writer / attestation は `tamper-proof-review` 側の責務とする

## Schema Samples

### review.json (schema_version 1, cycle-based)

各 round の `verdict` フィールドはレビューワーの出力をそのまま保存する。
orchestrator が加工・要約・改変してはならない（verdict falsification 防止）。

```json
{
  "schema_version": 1,
  "cycles": [
    {
      "cycle_id": "2026-03-29T09:47:00Z",
      "started_at": "2026-03-29T09:47:00Z",
      "base_ref": "main",
      "policy_hash": "sha256:abc123...",
      "groups": {
        "infra-domain": {
          "scope": ["libs/domain/src/review/state.rs", "libs/usecase/src/review_workflow/mod.rs"],
          "rounds": [
            {
              "round_type": "fast",
              "success": "success",
              "error_message": null,
              "timestamp": "2026-03-29T09:48:23Z",
              "hash": "rvw1:sha256:def456...",
              "verdict": {
                "verdict": "zero_findings",
                "findings": []
              }
            }
          ]
        },
        "harness-policy": {
          "scope": [".claude/rules/10-guardrails.md"],
          "rounds": []
        },
        "other": {
          "scope": ["Makefile.toml"],
          "rounds": []
        }
      }
    }
  ]
}
```

### track/review-scope.json (base policy, groups 拡張後)

```json
{
  "version": 1,
  "groups": {
    "domain": {
      "patterns": ["libs/domain/**"]
    },
    "usecase": {
      "patterns": ["libs/usecase/**"]
    },
    "infrastructure": {
      "patterns": ["libs/infrastructure/**"]
    },
    "cli": {
      "patterns": ["apps/**"]
    },
    "harness-policy": {
      "patterns": [
        ".claude/commands/**", ".claude/rules/**",
        ".claude/agent-profiles.json", ".claude/settings*.json",
        ".claude/permission-extensions.json",
        "project-docs/conventions/**",
        "AGENTS.md", "CLAUDE.md"
      ]
    }
  },
  "review_operational": ["track/items/<track-id>/review.json"],
  "planning_only": [".claude/docs/**/*.md", "docs/**", "knowledge/**/*.md", "..."],
  "other_track": ["track/items/<other-track>/**", "track/archive/**"],
  "normalize": {
    "**/metadata.json": {
      "remove_fields": ["review"],
      "fixed_fields": { "updated_at": "1970-01-01T00:00:00Z" }
    }
  }
}
```

### track/items/<track-id>/review-groups.json (optional per-track override)

```json
{
  "groups": {
    "cli-only": {
      "patterns": ["apps/cli/**"]
    }
  }
}
```

per-track override が存在する場合、base policy の `groups` を完全に置換する。
`other` group は常に暗黙導出され、override にも base にも明示不要。

## Rejected Alternatives

- **`metadata.json.review` を維持したまま track-level state を調整する**: track 定義と operational review state の結合が残り、parallel auto-record、planning-only、stale 判定の論点が再び同じファイルに集中する
- **group 間で同じ round 番号を揃える**: 独立 group 運用と矛盾し、ある group の `zero_findings` が他 group の再試行だけで無効化される
- **policy source を廃止して `review.json` にルールも運用状態も同居させる**: canonical な再計算ルールが失われ、`check-approved` が current tree に対する hash 再計算を安全に行えない
- **scope を policy rule snapshot のみで固定し、実ファイル集合を保存しない**: cycle 開始時点で誰がどの差分を担当していたかが曖昧になり、mandatory `other` を含む補集合 partition を凍結しづらい
- **`review.json` checksum を tamper detection と見なす**: 同じ書き込み主体が checksum も再計算できるため、security boundary にならない

## Consequences

- Good: review state が `metadata.json` から分離され、track 定義と review 運用状態の責務が明確になる
- Good: `zero_findings` 済み group を他 group の再試行から独立させられる
- Good: per-group hash と frozen partition による stale 判定へ移行できる
- Good: `track/review-scope.json` を policy source として維持でき、planning-only / review-operational の設定駆動設計を引き継げる
- Bad: `review.json` codec、status 表示、`check-approved`、record-round をまとめて作り直す必要がある
- Bad: 旧 `metadata.review` データとの migration / backward compatibility はこの判断では提供しない。新規トラック専用とする
- Bad: tamper-proof は別トラックのままなので、この ADR 単体では改ざん耐性までは提供しない

## Reassess When

- group partition が重複を許す review model に変わる場合
- final を group ごとに required / optional へ分ける要件が入る場合
- `track/review-scope.json` 以外の canonical policy source が導入される場合
- verdict provenance / trusted writer を review state model と同時に一体設計すべき要件が出る場合
