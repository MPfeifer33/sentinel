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

Each file row contains:

- `path`
- `known_in_matrix`
- `risk_score`
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

## JSON Contract

Every JSON response includes:

```json
{
  "ok": true
}
```

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
