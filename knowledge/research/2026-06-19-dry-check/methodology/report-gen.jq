def layerof($p):
  if   ($p|startswith("libs/domain"))          then "domain"
  elif ($p|startswith("libs/usecase"))         then "usecase"
  elif ($p|startswith("libs/infrastructure"))  then "infrastructure"
  elif ($p|startswith("apps/cli-composition")) then "cli-composition"
  elif ($p|startswith("apps/cli"))             then "cli"
  else "other" end;
def loc($x): ($x.locations | map(.file + ":" + (.lines|tostring)) | join(", "));
def crosslayer($x): (($x.locations | map(layerof(.file)) | unique | length) > 1);

$s[0] as $sum |
"# DRY Violation Snapshot — \($sum.label) (\($sum.commit))",
"",
"Independent AI-based DRY-violation census (intra-unit + thematic finders -> adversarial verification).",
"Scope: src/ of the 5 first-party crates (incl. inline #[cfg(test)] modules). Excluded: vendor/** and integration tests/ dirs.",
"",
"## Overview",
"",
"| metric | value |",
"|---|---|",
"| totalLoc | \($sum.totalLoc) |",
"| unitCount | \($sum.unitCount) |",
"| totalFindings | \($sum.totalFindings) |",
"| weightedScore (high*3+med*2+low*1) | \($sum.weightedScore) |",
"| densityPerKLoc | \($sum.densityPerKLoc) |",
"| weightedDensityPerKLoc | \($sum.weightedDensityPerKLoc) |",
"| crossLayerFindings | \($sum.crossLayerFindings) |",
"| unverifiedKept | \($sum.unverifiedKept) |",
"",
"## Breakdown by severity",
"",
"| severity | count |",
"|---|---|",
"| high | \($sum.bySeverity.high) |",
"| medium | \($sum.bySeverity.medium) |",
"| low | \($sum.bySeverity.low) |",
"",
"## Breakdown by category",
"",
"| category | count |",
"|---|---|",
($sum.byCategory | to_entries | sort_by(-.value) | .[] | "| \(.key) | \(.value) |"),
"",
"## Breakdown by layer (primary location)",
"",
"| layer | count |",
"|---|---|",
($sum.byLayer | to_entries | sort_by(-.value) | .[] | "| \(.key) | \(.value) |"),
"",
"## Cross-layer findings — DRY gate blind-spot candidates",
"",
($f[0] | map(select(crosslayer(.))) | sort_by(.severity, .category) | .[] |
  "- **\(.title)** _[\(.severity)/\(.category)]_ (`\(.id)`)\n  - \(loc(.))"),
"",
"## Full enumeration",
"",
( ["high","medium","low"][] as $sev |
  "### Severity: \($sev)",
  "",
  ( $f[0] | map(select(.severity==$sev)) | sort_by(.category, .title) | .[] |
    "#### [\(.category)] \(.title) (`\(.id)`)",
    "",
    "- Rationale: \(.rationale)",
    "- Locations: \(loc(.))",
    ( if (.suggested_fix // "") != "" then "- Fix: \(.suggested_fix)" else empty end),
    ""
  )
)
