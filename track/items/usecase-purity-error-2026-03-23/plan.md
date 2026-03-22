<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# INF-17: usecase-purity warning → error 昇格 — CI ブロック化

usecase-purity lint を warning-only から error に昇格し、CI で usecase 層の hexagonal violation をブロックする。INF-16 で既存 violation ゼロ達成済み。

## Phase 1: warning → error 昇格

T001: usecase_purity.rs の Finding::warning → Finding::error に変更。
CI 全通し確認（violation ゼロのため error 化しても CI はブロックされない）。

- [x] Finding::warning → Finding::error に変更 + CI 全通し確認
