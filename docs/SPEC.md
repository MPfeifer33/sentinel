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
- per-file risk rows
- summary counts by risk band

Each file row contains:

- `path`
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
