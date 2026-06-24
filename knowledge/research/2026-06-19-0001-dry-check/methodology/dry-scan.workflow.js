export const meta = {
  name: 'dry-violation-scan',
  description: 'Enumerate DRY-principle violations across the Rust workspace (intra-unit + thematic finders), adversarially verify, and write a snapshot report (findings.json/summary.json/report.md). Run identically on before/after checkouts for a fair comparison.',
  phases: [
    { title: 'Load', detail: 'load unit partition (units.json)' },
    { title: 'Scan', detail: 'intra-unit + thematic finders (sonnet/medium)' },
    { title: 'Verify', detail: 'adversarial batched verification (opus/high)' },
    { title: 'Synthesize', detail: 'write findings.json + summary.json + report.md' },
  ],
}

// ---- args (defensive: the runtime may deliver args as a JSON string) ----
const A = (typeof args === 'string') ? JSON.parse(args) : (args || {})
const label = A.label
const commit = A.commit
const outDir = A.outDir
const unitsPath = A.unitsPath
let totalLoc = 0
let units = []

// ---- shared rubric (identical for finders and verifiers) ----
const RUBRIC = `DRY (Don't Repeat Yourself) RUBRIC — apply EXACTLY:

A DRY violation = the SAME knowledge/logic/data is expressed in 2+ places, so a single
conceptual change would force edits in multiple sites (change-amplification). You MUST cite
2+ concrete locations (file + line range) per finding.

CATEGORIES:
- exact-clone: byte/near-identical code block copy-pasted in 2+ places (>= ~8 lines).
- near-clone: structurally identical, differing only in renamed identifiers/types (>= ~8 lines).
- semantic-dup: different code computing the SAME result / encoding the same rule
  (e.g., two parsers for one format, two impls of one validation).
- structural-dup: repeated boilerplate scaffolding a macro/generic/helper should factor
  (repeated match-arm sequences, repeated error-conversion impls, repeated builder/setup
  blocks). Flag ONLY when abstraction is clearly warranted.
- data-dup: duplicated constants, magic strings/numbers, duplicated field-name lists /
  schema shapes / default config values in 2+ places.
- knowledge-dup: the SAME business rule / invariant / mapping encoded in multiple files or
  layers (highest-value).

SEVERITY:
- high: duplicated NON-trivial logic/knowledge where divergence would cause bugs; OR 3+
  copies; OR duplication spanning architectural layers.
- medium: 2 copies of a logical block (~8-30 lines) with a clear extraction opportunity.
- low: minor/local repetition, small duplicated constants, borderline cases.

DOES NOT COUNT (exclude — NOT violations):
- Coincidental syntactic similarity with different intent (e.g., two unrelated functions
  that both end with Ok(())).
- Idiomatic Rust the language requires and that abstraction would obscure (trivial one-line
  impl From, derivable impls, standard trait impls).
- INTENTIONAL hexagonal-architecture separation: a domain type, its usecase port, and its
  infrastructure adapter/codec are SUPPOSED to be distinct — NOT a DRY violation. A DTO/codec
  that mirrors a domain struct's fields is by-design separation, not duplication.
- Generated code, explicit test data tables, snapshot fixtures.
- Re-exports, mod declarations, use lists. Documentation/comment similarity. Cargo.toml deps.

Precision over recall: a wrong finding pollutes the comparison. When unsure, DROP.`

// ---- thematic cross-cutting detectors (fixed; identical both snapshots) ----
const THEMES = [
  { key: 'error-types', title: 'Error types & conversion',
    desc: 'Duplicate error enums, repeated impl From<..> for ..Error, parallel error taxonomies across crates, repeated error-message string literals, repeated map_err shapes.',
    hints: 'rg "enum .*Error", rg "impl From<", rg "map_err", rg "thiserror", grep duplicated error message strings' },
  { key: 'serde-codec', title: 'Serde / codec / JSON shape',
    desc: 'Duplicate (de)serialization logic, repeated JSON field-name string literals, parallel *_codec structs encoding the same shape, duplicate schema definitions.',
    hints: 'rg "#\\[serde", rg "Serialize|Deserialize", rg "serde_json", look at *_codec.rs files, repeated field-name literals' },
  { key: 'fs-path-io', title: 'Filesystem path & IO',
    desc: 'Repeated path construction, repeated read+parse / write+serialize boilerplate, duplicate ensure-dir / atomic-write logic.',
    hints: 'rg "fs::", rg "read_to_string", rg "PathBuf|\\.join\\(", rg "create_dir|OpenOptions", atomic write helpers' },
  { key: 'validation-newtype', title: 'Validation & newtype boilerplate',
    desc: 'Repeated newtype validation (non-empty / trim / parse), duplicate TryFrom validators, repeated guard/clamp logic, parallel value-object constructors.',
    hints: 'rg "impl TryFrom", rg "fn new\\(", rg "is_empty\\(\\)", rg "trim\\(\\)", value_objects modules' },
  { key: 'cli-dispatch', title: 'CLI dispatch & args',
    desc: 'Repeated CLI command parsing/dispatch boilerplate, duplicated arg structs, repeated subcommand wiring between apps/cli and apps/cli-composition.',
    hints: 'rg "#\\[command", rg "#\\[arg", rg "Subcommand", compare apps/cli/src/commands vs apps/cli-composition' },
  { key: 'shell-proc', title: 'Shell / process execution',
    desc: 'Duplicate subprocess spawning, duplicate command-string building, repeated stdout/stderr capture/handling.',
    hints: 'rg "Command::new", rg "std::process", rg "\\.output\\(\\)|\\.spawn\\(", rg "stdout|stderr"' },
  { key: 'git-ops', title: 'Git operations',
    desc: 'Duplicate git invocation wrappers, repeated commit-hash/rev-parse/diff parsing.',
    hints: 'rg "git", rg "Command::new\\(\"git\"\\)", rg "rev-parse|diff", commit hash handling' },
  { key: 'rendering', title: 'Rendering / formatting',
    desc: 'Duplicate markdown/view rendering, repeated table/section formatting, parallel render functions (plan.md / registry.md / *-types.md / contract-map renderers).',
    hints: 'rg "writeln!|push_str", rg "format!", render_* functions, *_render*.rs, view renderers' },
  { key: 'test-fixtures', title: 'Test fixtures & setup',
    desc: 'Duplicate test setup/builders/fixtures across test modules, repeated arrange blocks, repeated tempdir/sample-data construction.',
    hints: 'rg "#\\[cfg\\(test\\)\\]", rg "#\\[fixture\\]|rstest", rg "fn setup|fn sample|tempdir", repeated arrange blocks' },
  { key: 'constants', title: 'Constants / config defaults',
    desc: 'Magic numbers/strings repeated across files, duplicate default config values, repeated thresholds/limits/timeouts, duplicated string keys.',
    hints: 'rg "const |static ", repeated numeric literals, default config values, repeated string keys/paths' },
]

// ---- schemas ----
const FINDING_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: false,
        properties: {
          category: { type: 'string', enum: ['exact-clone', 'near-clone', 'semantic-dup', 'structural-dup', 'data-dup', 'knowledge-dup'] },
          title: { type: 'string' },
          description: { type: 'string' },
          locations: {
            type: 'array', minItems: 2,
            items: {
              type: 'object', additionalProperties: false,
              properties: {
                file: { type: 'string' },
                lines: { type: 'string' },
                summary: { type: 'string' },
              },
              required: ['file', 'lines'],
            },
          },
          severity: { type: 'string', enum: ['high', 'medium', 'low'] },
          rationale: { type: 'string' },
          suggested_fix: { type: 'string' },
          confidence: { type: 'string', enum: ['high', 'medium', 'low'] },
        },
        required: ['category', 'title', 'description', 'locations', 'severity', 'rationale', 'confidence'],
      },
    },
  },
  required: ['findings'],
}

const UNITS_LOADER_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    totalLoc: { type: 'integer' },
    units: {
      type: 'array',
      items: {
        type: 'object', additionalProperties: false,
        properties: {
          name: { type: 'string' },
          layer: { type: 'string' },
          approxLoc: { type: 'integer' },
          fileCount: { type: 'integer' },
          pathsFile: { type: 'string' },
        },
        required: ['name', 'layer', 'pathsFile'],
      },
    },
  },
  required: ['totalLoc', 'units'],
}

const VERDICT_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    verdicts: {
      type: 'array',
      items: {
        type: 'object', additionalProperties: false,
        properties: {
          id: { type: 'string' },
          keep: { type: 'boolean' },
          adjusted_severity: { type: 'string', enum: ['high', 'medium', 'low'] },
          reason: { type: 'string' },
        },
        required: ['id', 'keep', 'reason'],
      },
    },
  },
  required: ['verdicts'],
}

// ---- helpers ----
function parseStart(s) { const m = String(s || '').match(/\d+/); return m ? parseInt(m[0], 10) : 0 }
function layerOf(f) {
  if (/^libs\/domain\//.test(f)) return 'domain'
  if (/^libs\/usecase\//.test(f)) return 'usecase'
  if (/^libs\/infrastructure\//.test(f)) return 'infrastructure'
  if (/^apps\/cli-composition\//.test(f)) return 'cli-composition'
  if (/^apps\/cli\//.test(f)) return 'cli'
  return 'other'
}
function sig(f) {
  const locs = (f.locations || []).map(l => `${l.file}#${Math.floor(parseStart(l.lines) / 30)}`).sort()
  return (f.category || '') + '::' + locs.join('|')
}
function dedupe(cands) {
  const seen = new Map()
  for (const f of cands) {
    if (!f || !f.locations || f.locations.length < 2) continue
    const k = sig(f)
    const ex = seen.get(k)
    if (!ex) seen.set(k, f)
    else if ((f.description || '').length > (ex.description || '').length) seen.set(k, f)
  }
  return [...seen.values()]
}
function chunk(arr, n) { const out = []; for (let i = 0; i < arr.length; i += n) out.push(arr.slice(i, i + n)); return out }

// Run thunks in small sequential waves (caps peak concurrency at `size` to avoid
// server-side rate limiting), then retry any that returned null once more.
async function runChunkedThunks(thunks, size) {
  const results = new Array(thunks.length).fill(null)
  for (let i = 0; i < thunks.length; i += size) {
    const idx = []
    for (let k = i; k < Math.min(i + size, thunks.length); k++) idx.push(k)
    const res = await parallel(idx.map(j => thunks[j]))
    idx.forEach((j, k) => { results[j] = res[k] })
  }
  const failed = []
  results.forEach((r, i) => { if (r === null) failed.push(i) })
  if (failed.length) {
    log(`${label}: retrying ${failed.length} failed/rate-limited agents`)
    for (let i = 0; i < failed.length; i += size) {
      const idx = failed.slice(i, i + size)
      const res = await parallel(idx.map(j => thunks[j]))
      idx.forEach((j, k) => { if (res[k] !== null) results[j] = res[k] })
    }
  }
  return results
}

function intraPrompt(u) {
  return `You are a DRY-violation auditor for a Rust hexagonal-architecture workspace (layers: domain -> usecase -> infrastructure; apps: cli, cli-composition).
SNAPSHOT: ${label} (commit ${commit}).
Your scan unit: "${u.name}" (layer: ${u.layer}, ~${u.approxLoc} LOC, ${u.fileCount} files).

Your file list is in: ${u.pathsFile}  (one repo-relative path per line).
Read that .paths file FIRST, then read/skim ALL the source files it lists; use Grep to confirm any suspected duplication.

${RUBRIC}

TASK:
1. Find DRY violations PRIMARILY WITHIN these files (duplicated logic/knowledge/data across 2+ of these files, or a clearly-repeated block appearing 2+ times).
2. If you spot an obvious duplicate of this code OUTSIDE the unit (a sibling module), you MAY report it, citing the external file:lines (use Grep to locate the twin).
3. Cite 2+ concrete file:line locations per finding. Precision over recall — do NOT pad with weak or speculative findings. Return an empty array if nothing solid.
Return findings via the schema.`
}

function themePrompt(t) {
  return `You are a DRY-violation auditor for a Rust hexagonal-architecture workspace.
SNAPSHOT: ${label} (commit ${commit}).
Your cross-cutting theme: ${t.title}.
Scope: ${t.desc}
Search the ENTIRE first-party source tree: libs/domain/src, libs/usecase/src, libs/infrastructure/src, apps/cli/src, apps/cli-composition/src.
Search hints: ${t.hints}

${RUBRIC}

TASK: Use Grep/Glob to locate candidate duplicates for THIS theme across the whole repo, then Read to confirm the code is genuinely duplicated/equivalent. Report only solid DRY violations of this theme. Cite 2+ concrete file:line locations per finding. Precision over recall.
Return findings via the schema.`
}

function verifyPrompt(batch) {
  const items = batch.map(f => ({
    id: f.id, category: f.category, title: f.title, severity: f.severity,
    description: f.description, rationale: f.rationale,
    locations: (f.locations || []).map(l => ({ file: l.file, lines: l.lines })),
  }))
  return `You are an ADVERSARIAL DRY-finding verifier. For each candidate, decide if it is a GENUINE DRY violation per the rubric, or must be DROPPED (false positive).
Default to DROP when the separation is justified: hexagonal layer boundaries (domain type vs port vs adapter/codec), coincidental similarity, idiomatic/derivable boilerplate, intentional explicit test fixtures, or when the cited locations are not actually equivalent.
You MUST Read the cited files to confirm the line ranges really contain duplicated/equivalent code before keeping a finding.

${RUBRIC}

CANDIDATES (JSON):
${JSON.stringify(items)}

For EACH candidate id, return {id, keep:boolean, adjusted_severity, reason}. Set keep=false to drop. Adjust severity if the rubric warrants. Be strict and consistent.
Return verdicts via the schema.`
}

function computeStats(findings) {
  const bySeverity = { high: 0, medium: 0, low: 0 }
  const byCategory = {}
  const byLayer = {}
  let crossLayer = 0, unverified = 0
  for (const f of findings) {
    bySeverity[f.severity] = (bySeverity[f.severity] || 0) + 1
    byCategory[f.category] = (byCategory[f.category] || 0) + 1
    const ls = new Set((f.locations || []).map(l => layerOf(l.file)))
    const primary = (f.locations && f.locations[0]) ? layerOf(f.locations[0].file) : 'other'
    byLayer[primary] = (byLayer[primary] || 0) + 1
    if (ls.size > 1) crossLayer++
    if (f.verification === 'unverified') unverified++
  }
  const weighted = bySeverity.high * 3 + bySeverity.medium * 2 + bySeverity.low * 1
  const kloc = totalLoc / 1000
  return {
    label, commit, totalLoc, unitCount: units.length,
    totalFindings: findings.length,
    bySeverity, byCategory, byLayer,
    crossLayerFindings: crossLayer,
    unverifiedKept: unverified,
    weightedScore: weighted,
    densityPerKLoc: Number((findings.length / kloc).toFixed(3)),
    weightedDensityPerKLoc: Number((weighted / kloc).toFixed(3)),
  }
}

function synthPrompt(findings, stats) {
  // Minimal & reliable: the synth agent writes ONLY the small summary.json via the
  // Write tool. The full confirmed findings are carried verbatim inside ARCHIVE
  // markers so the orchestrator can extract findings.json deterministically with jq
  // (avoids an agent emitting a huge verbatim JSON, which previously triggered a
  // python-scripting fallback that the sandbox blocks).
  return `Use ONLY the Write tool to create the file ${outDir}/summary.json with EXACTLY this content. Do NOT use python, bash, scripts, or any other mechanism — emit the content directly in a single Write tool call, then stop:

${JSON.stringify(stats)}

After the Write succeeds, reply with the single word: done

Do NOT read, transform, or write anything else. The block below is archival data for an external tool; ignore it for your task.
ARCHIVE_FINDINGS_BEGIN
${JSON.stringify(findings)}
ARCHIVE_FINDINGS_END`
}

// =========================== run ===========================
phase('Load')
const loaded = await agent(
  `Read the JSON file at ${unitsPath}. Its shape is {"totalLoc": int, "units": [{name, layer, approxLoc, fileCount, pathsFile, paths:[...]}]}.
Return totalLoc, and the units array where each unit includes ONLY name, layer, approxLoc, fileCount, pathsFile (OMIT the paths array — finders read pathsFile themselves).
Return ALL units in the same order; do not invent, merge, or drop any.`,
  { label: 'load-units', phase: 'Load', schema: UNITS_LOADER_SCHEMA, model: 'sonnet', effort: 'low' })
totalLoc = (loaded && loaded.totalLoc) || 0
units = (loaded && Array.isArray(loaded.units)) ? loaded.units : []
log(`${label}: loaded ${units.length} units, totalLoc=${totalLoc}`)
if (units.length === 0) { throw new Error('unit load failed: no units') }

phase('Scan')
const intraThunks = units.map(u => () => agent(intraPrompt(u), { label: `scan:${u.name}`, phase: 'Scan', schema: FINDING_SCHEMA, model: 'sonnet', effort: 'medium' }))
const themeThunks = THEMES.map(t => () => agent(themePrompt(t), { label: `theme:${t.key}`, phase: 'Scan', schema: FINDING_SCHEMA, model: 'sonnet', effort: 'medium' }))
const scanRes = await runChunkedThunks([...intraThunks, ...themeThunks], 5)
let candidates = scanRes.filter(Boolean).flatMap(r => (r && Array.isArray(r.findings)) ? r.findings : [])
candidates = candidates.filter(f => f && Array.isArray(f.locations) && f.locations.length >= 2)
const deduped = dedupe(candidates)
deduped.forEach((f, i) => { f.id = `${label}-${String(i + 1).padStart(3, '0')}` })
log(`${label}: ${candidates.length} raw candidates -> ${deduped.length} unique after dedupe`)

phase('Verify')
const batches = chunk(deduped, 8)
const verifyThunks = batches.map((b, i) => () => agent(verifyPrompt(b), { label: `verify:batch-${i + 1}`, phase: 'Verify', schema: VERDICT_SCHEMA, model: 'opus', effort: 'high' }))
const vres = await runChunkedThunks(verifyThunks, 5)
const verdicts = {}
vres.filter(Boolean).forEach(r => (r.verdicts || []).forEach(v => { if (v && v.id) verdicts[v.id] = v }))
const confirmed = []
let dropped = 0
for (const f of deduped) {
  const v = verdicts[f.id]
  if (v) {
    if (v.keep) { f.severity = v.adjusted_severity || f.severity; f.verify_reason = v.reason; confirmed.push(f) }
    else { dropped++ }
  } else { f.verification = 'unverified'; confirmed.push(f) }
}
log(`${label}: ${confirmed.length} confirmed, ${dropped} dropped by verifier`)

const stats = computeStats(confirmed)

phase('Synthesize')
const synthNote = await agent(synthPrompt(confirmed, stats), { label: 'synth:write-summary', phase: 'Synthesize', model: 'sonnet', effort: 'low' })

return { label, commit, totalLoc, unitCount: units.length, rawCandidates: candidates.length, unique: deduped.length, confirmed: confirmed.length, dropped, stats, synthNote }
