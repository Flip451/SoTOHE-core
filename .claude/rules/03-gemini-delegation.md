# Gemini Provider Guidance

**Gemini CLI は、active profile が `researcher` / `multimodal_reader` を Gemini に割り当てた時の specialist provider として使う。**

## 既定 profile での主担当 capability

### 1. `researcher`: コードベース・リポジトリ理解（1M context）

```bash
gemini -p "Analyze this Rust codebase:
- Cargo workspace structure and crate organization
- Key traits (domain ports) and their implementations (adapters)
- Async patterns and Tokio usage
- Error handling strategy
- Test structure and coverage approach" 2>/dev/null
```

### 2. `researcher`: 外部リサーチ・サーベイ（Google Search grounding）

```bash
# Rust クレート調査
gemini -p "Research Rust crate: {name}.
Latest version, key features, async support, idiomatic usage,
known issues, alternatives. Include docs.rs links." 2>/dev/null

# ベストプラクティス
gemini -p "Research Rust best practices for {topic}.
Latest recommendations from Rust community." 2>/dev/null
```

### 3. `multimodal_reader`: マルチモーダルファイル読取

PDF/動画/音声/画像ファイルが登場したら、active profile の `multimodal_reader` が Gemini の場合は次を使う：

```bash
gemini -p "Extract from /path/to/file.pdf: {what to extract}" 2>/dev/null
```

> **Note**: path-in-prompt 形式を使う。

補足: どの capability が実際に Gemini を使うかは `.claude/agent-profiles.json` を正本とする。

## In The Default Profile, Gemini Usually Does Not Own

| Task | Who Does It |
|------|-------------|
| `planner` / `reviewer` | **Codex CLI** |
| `debugger` | **Codex CLI** |
| `implementer` | **Claude Code / Subagent** |

## Output

調査結果は `knowledge/research/{topic}.md` に保存する。

## Language Protocol

1. Ask Gemini in **English**
2. Receive response in **English**
3. Report to user in **Japanese**
