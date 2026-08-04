# PROJECT.md — sentinel

**What:** Continuous regression watcher. Builds a git-history fragility matrix
and warns agents when changed files carry historical risk.

**Status:** Quality pass in progress. Scan, matrix, risk, tests, status, and
doctor commands are available with text and JSON output.

**Tech:** Rust 2021, clap 4, serde/serde_json, thiserror.

**Storage:** `.agent-sentinel/matrix.json` under repo root, gitignored.

## Module Ownership

| Module | Owner | Status |
|--------|-------|--------|
| cli.rs | Bjarn | Done |
| main.rs | Bjarn | Done |
| git.rs | Bjarn | Done |
| analyze.rs | Bjarn | Done |
| model.rs | Bjarn | Done |
| store.rs | Bjarn | Done |
| report.rs | Bjarn | Done |

## Usage

```sh
sentinel scan --force               # build fragility matrix
sentinel risk                       # inspect changed files
sentinel risk --file src/main.rs    # inspect explicit file
sentinel matrix --top 20            # top risky files
sentinel tests src/main.rs          # historically related tests
sentinel status                     # storage and source status
sentinel doctor                     # agent preflight summary
```

## Risk Signals

- commit frequency and recency
- failure-like commit subjects
- revert/rollback commit subjects
- source/test co-change
- line churn
- matrix freshness and sparse-history confidence
- unknown-file coverage (`known_in_matrix: false`)

## Last Updated

2026-08-04 — Added matrix-health trust signals and agent doctor preflight.
