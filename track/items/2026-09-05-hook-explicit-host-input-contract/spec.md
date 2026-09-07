<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 32, yellow: 0, red: 0 }
---

# hook 接続元の host 明示と入力契約の選択

## Goal

- [GO-01] agent hook の接続元を入力 JSON から推定せず明示的な host 引数で選択し、互換フィールドが同居する PreToolUse envelope でも選択済み host の入力契約を一貫して解釈できるようにする [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D1, knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D2]
- [GO-02] PreToolUse と UserPromptSubmit の既存ガードおよび advisory 動作を保ったまま、新バイナリと各接続設定を一組で host 明示契約へ移行する [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D3, knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D4]
- [GO-03] 実装 fingerprint の workspace 入力集合から root 直下の再生成可能な .cache だけを除外しつつ、その他の記録対象の変更検知と新規則による再取得を維持する [adr: knowledge/adr/2026-09-05-1031-rustdoc-fingerprint-excludes-workspace-cache.md#D1, knowledge/adr/2026-09-05-1031-rustdoc-fingerprint-excludes-workspace-cache.md#D3]

## Scope

### In Scope
- [IN-01] agent hook の PreToolUse および UserPromptSubmit dispatch で --host claude|codex|grok を必須にし、未指定または不正な値を CLI 入力エラー（終了コード 2）として扱う。Git process hook は従来の位置引数経路を維持し、--host との組み合わせを拒否する [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D1] [tasks: T002, T003, T006, T008, T009, T010, T011, T013]
- [IN-02] Claude の接続設定と Codex・Grok の wrapper を、各々が固定の host を渡して PreToolUse と skill-compliance を起動する接続契約へ更新する [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D1, knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D3] [tasks: T005, T006, T012]
- [IN-03] Grok host の PreToolUse は toolName と toolInput を正とし、同居する Claude 互換別名を無視して既存 terminal と search_replace の写像へ正規化する。未知の Grok tool、必須値の欠落、または型不正は拒否する [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D2, knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D9] [tasks: T002, T006, T008]
- [IN-04] Claude と Codex host の PreToolUse は既存 snake_case 契約を共有して解釈し、Codex の apply_patch 処理、共通ハンドラーへの正規化、および既存ガードポリシーを維持する [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D2] [tasks: T002, T006, T008]
- [IN-05] シェルを実行できる環境では cargo make build-sotp で新バイナリを構築し、host 明示済みの接続設定と一組で切り替える [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D4] [tasks: T004, T005, T006, T012]
- [IN-06] 実装 fingerprint の workspace 走査で root 直下の .cache だけを列挙・読取・内容 hash 化・件数または bytes 予算計上から除外し、root .cache 自体が symlink の場合も参照先へ進まない [adr: knowledge/adr/2026-09-05-1031-rustdoc-fingerprint-excludes-workspace-cache.md#D1] [tasks: T001]
- [IN-07] 新しい入力集合の規則で fingerprint と関連する型シグナルを取得し直し、root .cache 以外の Rust source、Cargo manifest、lockfile など記録対象の内容変更を引き続き input identity に反映する [adr: knowledge/adr/2026-09-05-1031-rustdoc-fingerprint-excludes-workspace-cache.md#D3] [tasks: T001]

### Out of Scope
- [OS-01] --host を接続元の認証、権限付与、または信頼境界の根拠として扱うこと [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D1] [tasks: T002, T003, T006, T008, T010]
- [OS-02] host 未指定時または選択済み形式の解釈失敗時に、JSON キーから host や別入力形式を自動推定してフォールバックする互換経路 [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D1, knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D2] [tasks: T002, T003, T006, T008, T010]
- [OS-03] wrapper による入力 JSON の加工、Claude 互換フィールドの除去、または旧バイナリを前提にした接続設定との互換化 [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D2, knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D4] [tasks: T004, T005, T006]
- [OS-05] 深い階層の同名 directory、任意の Git ignored path、または拡張子に基づく一般的な生成物除外と、Cargo の意味的な完全入力集合を独自に保証すること [adr: knowledge/adr/2026-09-05-1031-rustdoc-fingerprint-excludes-workspace-cache.md#D1] [tasks: T001]

## Constraints
- [CN-01] agent hook の標準呼び出し順は hook dispatch --host <host> <hook-id> とし、入力 JSON のキーの有無や組み合わせを host 選択に用いない [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D1] [tasks: T002, T003, T005, T010, T012, T013]
- [CN-02] 選択済み host の入力契約を解釈できない場合は別形式へフォールバックせず拒否し、Grok の同居別名は一致確認を要求せず無視する [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D2] [tasks: T002, T006, T008, T010]
- [CN-03] 入力契約の解釈は Rust の入力境界に集約し、wrapper は JSON を加工せず、正規化後の共通ハンドラーと既存ガードポリシーを変更しない [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D2] [tasks: T002, T005, T006, T008, T009, T011]
- [CN-04] skill-compliance は各 host の既存 prompt 処理を再利用し、接続設定の || exit 0 を維持して、host 指定ミス・CLI 起動失敗・入力パース失敗が UserPromptSubmit を停止させない [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D3] [tasks: T005, T006, T009, T011, T012, T014]
- [CN-05] 接続設定を切り替える前に新バイナリを用意し、旧バイナリを救済する自動判定や互換 wrapper を追加しない [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D4] [tasks: T004, T005, T012]
- [CN-06] root .cache の限定除外によって、除外対象外の入力に対する既存の件数・サイズ・path・環境値・時間上限、symlink・I/O error・途中変更・metadata 取得失敗の拒否、または fail-closed を弱めない [adr: knowledge/adr/2026-09-05-1031-rustdoc-fingerprint-excludes-workspace-cache.md#D2] [tasks: T001]
- [CN-07] 評価開始時と終端の入力確認には同じ root .cache 除外規則を適用し、規則変更前の fingerprint または型シグナルを新規則の成功結果として流用しない [adr: knowledge/adr/2026-09-05-1031-rustdoc-fingerprint-excludes-workspace-cache.md#D3] [tasks: T001]

## Acceptance Criteria
- [ ] [AC-01] agent PreToolUse と UserPromptSubmit の direct hook dispatch は claude、codex、または grok の --host を要求し、未指定・不正値では終了コード 2 を返す。Git process hook に --host を組み合わせた呼び出しは拒否される [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D1] [tasks: T002, T003, T006, T008, T009, T010, T011, T013]
- [ ] [AC-02] Claude の設定、Codex wrapper、Grok wrapper の PreToolUse と skill-compliance 呼び出しが、それぞれの固定 host を --host で渡す [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D1, knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D3] [tasks: T005, T006, T012]
- [ ] [AC-03] grok host で toolName、toolInput、および Claude 互換別名が同居する PreToolUse envelope は、camelCase の Grok 値だけを用いて既存 terminal または search_replace の処理へ到達する。別名の一致は要求されない [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D2] [tasks: T002, T006, T008]
- [ ] [AC-04] grok host の未知 tool、必須値欠落、または型不正は拒否され、Claude/Codex 形式へのフォールバックを起こさない。Claude と Codex は既存 snake_case PreToolUse を処理し、Codex の apply_patch 処理を維持する [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D2] [tasks: T002, T006, T008]
- [ ] [AC-05] skill-compliance の入力パース失敗、host 指定ミス、または CLI 起動失敗は接続設定で advisory として吸収され、UserPromptSubmit をブロックしない [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D3] [tasks: T005, T006, T009, T011, T012, T014]
- [ ] [AC-06] wrapper は入力 JSON を加工せず、選択済み host の入力境界で正規化された処理が既存の共通ハンドラーとガードポリシーへ合流する [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D2] [tasks: T002, T005, T006, T008]
- [ ] [AC-07] シェル実行可能な検証環境で cargo make build-sotp により構築した新バイナリと host 明示済み接続設定の組み合わせで、hook dispatch の自動テストが通る [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D4] [tasks: T004, T005, T006, T012]
- [ ] [AC-08] 固定した正常な UserPromptSubmit envelope（通常文と skill guidance 対象の prompt）を claude、codex、grok の各 --host で skill-compliance dispatch に与え、各 host の Allow 判定と additionalContext の有無・内容が、同じ入力を既存の共通 prompt 処理へ与えた結果と一致することを自動テストで確認する [adr: knowledge/adr/2026-09-05-0600-hook-explicit-host-input-contract.md#D3] [tasks: T006, T014]
- [ ] [AC-09] workspace root 直下の .cache 配下だけを変更し、そこに 64 MiB を超える生成ファイルが存在しても、実装 fingerprint の取得はその内容を読取・hash・予算計上せず成功し、入力 identity は変化しないことを決定的な自動テストで確認する [adr: knowledge/adr/2026-09-05-1031-rustdoc-fingerprint-excludes-workspace-cache.md#D1, knowledge/adr/2026-09-05-1031-rustdoc-fingerprint-excludes-workspace-cache.md#D3] [tasks: T001]
- [ ] [AC-10] root 直下ではない同名 .cache directory と任意の非除外 regular file は入力として扱い、後者が 64 MiB 上限を超えると authoritative-input error で失敗することを決定的な自動テストで確認する [adr: knowledge/adr/2026-09-05-1031-rustdoc-fingerprint-excludes-workspace-cache.md#D1, knowledge/adr/2026-09-05-1031-rustdoc-fingerprint-excludes-workspace-cache.md#D2] [tasks: T001]
- [ ] [AC-11] 新しい除外規則で fingerprint を取得し直し、root .cache 以外の記録対象である Rust source、Cargo manifest、または lockfile の内容変更は入力 identity を変化させ、取得失敗時に旧結果へ fallback しないことを決定的な自動テストで確認する [adr: knowledge/adr/2026-09-05-1031-rustdoc-fingerprint-excludes-workspace-cache.md#D3] [tasks: T001]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 32  🟡 0  🔴 0

