# PROJECT.md — sentinel

**What:** Continuous regression watcher. Builds a git-history fragility matrix
and warns agents when changed files carry historical risk.

**Status:** Quality pass in progress. Scan, matrix, risk, tests, status, and Shared plumbing (repo resolution, `--format`, exit codes incl. strict gate codes, error report) comes from `agent-tools-core`; `cargo test` passes with 20 tests.
doctor commands are available with text and JSON output. Doctor now exposes
agent-facing schema/action/gate/recommendation fields and strict gate exits.



**Storage:** `.agent-sentinel/matrix.json` under repo root as generated local
cache. Sentinel writes `.agent-sentinel/` into `.git/info/exclude`
idempotently; durable evidence belongs in Switchboard/Latch history or future
explicit exports.

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
sentinel doctor --strict            # same report, gate-coded exit
```

## Risk Signals

- commit frequency and recency
- failure-like commit subjects
- revert/rollback commit subjects
- source/test co-change
- line churn
- matrix freshness and sparse-history confidence
- unknown-file coverage (`known_in_matrix: false`)
- agent gates/action levels/recommended commands in `doctor`
- score breakdown and `coverage_status` for risk rows

## Last Updated

2026-09-11 — Moved repo resolution, `--format`, exit codes (strict gate
10/20/30 now named in the shared table), and the stderr error report onto
`agent-tools-core`; added a `--version` test. `cargo test` passes with 20 tests.

2026-08-06 — Added agent-first doctor parity: schema/scoring versions,
`status`, `action_level`, gates, typed recommendations, strict gate exits,
`coverage_status`, and `score_breakdown`.
