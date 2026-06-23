# PROJECT.md — sentinel

**What:** Continuous regression watcher. Builds a git-history fragility matrix
and warns agents when changed files carry historical risk.

**Status:** MVP implemented. Scan, matrix, risk, tests, and status commands are
available with text and JSON output.

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
```

## Risk Signals

- commit frequency and recency
- failure-like commit subjects
- revert/rollback commit subjects
- source/test co-change
- line churn

## Last Updated

2026-06-22 — Initial MVP built.
