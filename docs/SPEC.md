# sentinel MVP Spec

## Purpose

`sentinel` gives agents an early warning before they touch historically fragile
files. It is optimized for quick local use during coding sessions.

## Non-Goals

- It does not run tests.
- It does not watch the filesystem as a daemon in the MVP.
- It does not claim real failure correlation unless evidence sources record
  failures.

## Data Source

The MVP uses only local git history:

- `git log --name-only`
- `git diff --numstat`
- `git diff --name-only HEAD`
- `git ls-files --others --exclude-standard`

## Matrix Schema

The saved matrix contains:

- generation timestamp
- repo path
- history limit
- commits scanned
- git `HEAD` SHA at scan time
- whether the worktree had changed files at scan time
- per-file risk rows
- summary counts by risk band

The default saved matrix lives at `.agent-sentinel/matrix.json` as generated
local cache. When Sentinel writes the matrix, it ensures `.agent-sentinel/` is
present in the target repository's `.git/info/exclude` file. Agents should not
commit this cache file as durable evidence; long-term run history belongs in
Switchboard/Latch or in future explicit Sentinel exports/snapshots.

Each file row contains:

- `path`
- `known_in_matrix`
- `coverage_status`: `known` or `unknown`
- `risk_score`
- `score_breakdown`
- `level`
- commit count
- recent commit count
- failure-like commit count
- revert count
- test co-change count
- total churn
- related tests
- explanatory reasons

## Scoring

Scores are capped to 100 and placed into bands:

- `high`: 70-100
- `medium`: 40-69
- `low`: 15-39
- `quiet`: 0-14

This intentionally favors interpretable heuristics over false precision.

## Matrix Health

Agent-facing JSON responses include `matrix_health` when they are backed by a
stored or freshly generated matrix.

The health object contains:

- `matrix_head_sha`
- `current_head_sha`
- `head_matches`
- `dirty_at_scan`
- `dirty_now`
- `changed_files_count`
- `stale`
- `confidence`: `high`, `medium`, or `low`
- `warnings`

Warnings are part of the trust contract. A low-confidence or stale matrix may
still be useful, but agents should refresh it or broaden validation before
treating the score as decisive.

`known_in_matrix: false` means the file had no historical signal in the scanned
matrix. It is not proof that the file is safe; it means Sentinel has no local
history for that path.

## Agent Doctor

`sentinel doctor` is the one-shot preflight command for agents. It combines:

- matrix health
- changed files
- changed-file risks
- advice
- recommended next commands

Use it before edits and before commit handoff. The lower-level commands remain
available for drill-down.

`sentinel doctor --format json` returns:

```json
{
  "ok": true,
  "schema_version": "sentinel.doctor.v1",
  "status": "caution",
  "action_level": "refresh",
  "doctor": {
    "schema_version": "sentinel.doctor.v1",
    "scoring_version": "sentinel.git-history.v1",
    "status": "caution",
    "action_level": "refresh",
    "gates": {
      "matrix_fresh": false,
      "confidence_ok": true,
      "unknown_files": true,
      "high_risk_changed": false,
      "medium_risk_changed": false,
      "dirty_now": true
    },
    "matrix_health": {},
    "changed_files": ["src/new.rs"],
    "changed_file_risks": [],
    "advice": "...",
    "recommendations": ["sentinel scan --force"],
    "recommended_commands": [
      {
        "kind": "command",
        "command": "sentinel scan --force",
        "argv": ["sentinel", "scan", "--force"],
        "label": "Refresh the matrix before relying on risk scores",
        "reason": "matrix is missing HEAD metadata or was generated from a different HEAD",
        "reason_code": "matrix_not_fresh",
        "required": true
      }
    ]
  }
}
```

For compatibility with the MVP JSON surface, doctor responses also keep
top-level aliases for `matrix_health`, `changed_files`, `changed_file_risks`,
`advice`, `recommendations`, and `recommended_commands`. New consumers should
prefer the nested `doctor` object plus the top-level schema/action fields.

Doctor statuses:

- `ready`: no Sentinel-specific action required
- `caution`: refresh or validation is required before relying on the result
- `blocked`: review or stop is required before proceeding automatically

Doctor action levels:

- `none`: normal project validation is enough
- `refresh`: run `sentinel scan --force` before interpreting risk scores
- `validate`: run focused or normal validation before handoff
- `review`: request human/agent review before proceeding
- `stop`: do not proceed automatically; matrix confidence/freshness and risk
  signals conflict too strongly

Action precedence is:

```text
stop > review > refresh > validate > none
```

Strict mode:

```sh
sentinel doctor --strict
sentinel doctor --strict --format json
```

Strict mode prints the same successful report and then exits according to the
derived action level:

| Action level | Exit |
| ------------ | ---- |
| `none` | `0` |
| `refresh` | `10` |
| `validate` | `20` |
| `review` | `30` |
| `stop` | `30` |

These are gate exits, not runtime errors. JSON should still contain `ok: true`
unless Sentinel itself failed to run.

## JSON Contract

Every JSON response includes:

```json
{
  "ok": true
}
```

Agent-facing JSON responses also include command-specific `schema_version`
fields and, where scoring is present, `scoring_version`.

Errors use:

```json
{
  "ok": false,
  "error": {
    "code": "validation_error",
    "message": "..."
  }
}
```
