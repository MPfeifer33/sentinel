# Sentinel Quality Improvement Plan

Status: active
Date: 2026-08-04
Owners: Builder architecture/implementation, Reviewer read-only review, operator product direction

## Product Goal

Sentinel should be the agent-facing regression-risk preflight for local coding
work. Before an agent edits or commits, Sentinel should answer:

- which files are historically risky
- why they are risky
- which tests or validation paths deserve attention
- how complete and fresh the risk matrix is
- when the result is weak because history is sparse, stale, or absent

The quality bar is honesty over confidence. A thin or stale matrix is still
useful, but it must name its limits.

## Current Baseline

Observed on 2026-08-04:

- `cargo test --offline` passes 9/9.
- `sentinel scan --force --format json` works.
- Current repo history is very small: 3 commits scanned, 13 tracked files.
- Stored matrix exists under `.agent-sentinel/matrix.json`.
- `sentinel risk --format json` returns an empty changed-file set cleanly.

The MVP already has the core loop:

- build a git-history matrix
- rank files by risk
- inspect explicit or changed files
- report historically co-changed tests

## Primary Gaps

### 1. Matrix Freshness Is Not Explicit

The stored matrix does not currently record the git head it was built from, and
commands do not warn when the repository has moved since the matrix was saved.

This can make stale risk output look current.

### 2. Sparse History Can Look Too Certain

A matrix built from 3 commits is materially weaker than a matrix built from 300
commits, but the output does not label the confidence level.

This can make early-project repos look safer than they are.

### 3. Unknown Files Are Reported As Quiet

When a file has no historical signal, Sentinel returns a synthetic quiet row.
That is useful, but agents need to distinguish "historically quiet" from
"unknown to the matrix."

### 4. Agent JSON Needs More Context

`risk`, `matrix`, `tests`, and `scan` JSON should include enough metadata for
agents to decide whether to trust, refresh, or escalate validation.

### 5. Validation Advice Should Mention Signal Quality

Advice should react to:

- high/medium file risk
- unknown files
- stale matrix
- thin history

## Phase 1: Trust Surface Hardening

Implement first.

- Store `head_sha` in the matrix at scan time.
- Store whether the worktree had changed files at scan time.
- Add `known_in_matrix` to file risk rows.
- Add a shared matrix-health summary:
  - `current_head_sha`
  - `matrix_head_sha`
  - `head_matches`
  - `stale`
  - `confidence`
  - `warnings`
  - `changed_files_count`
- Include matrix health in JSON outputs.
- Print text warnings when the matrix is stale, thin, or generated from a dirty
  worktree.

Verification:

- Regression test for stale head detection.
- Regression test that synthetic unknown files are marked `known_in_matrix:
  false`.
- Existing tests stay green.

## Phase 2: Agent Preflight Command

Add a higher-level command, likely:

```sh
sentinel doctor
```

It should summarize:

- matrix exists/loadable
- freshness/confidence
- changed files
- changed-file risk distribution
- top risky changed files
- recommended next commands

This gives agents one stable preflight surface before editing or committing.

## Phase 3: Better Windowing And History Semantics

Improve scan semantics:

- expose total reachable commit count if cheap
- identify when `--limit` truncated history
- consider `--since` or `--base` in future
- document that scores are local-history heuristics, not test-failure truth

## Phase 4: Test Mapping Improvements

Improve related-test suggestions:

- detect Rust integration tests and module-adjacent unit tests more explicitly
- separate direct co-change tests from inferred tests
- include confidence/source for each related test

## Phase 5: Cross-Tool Integration

Sentinel should fit naturally with the local agent ops suite:

- `probe doctor` checks environment readiness
- `sentinel doctor` checks regression-risk readiness
- `atlas impact` checks code graph blast radius
- `sieve analyze` checks likely test impact
- `witness` records command evidence
- `rivet` verifies patch intent
- `latch` records coordination truth

Sentinel should avoid duplicating those tools. Its lane is historical
fragility, signal quality, and validation urgency.

## Reviewer Review Focus

Reviewer should review for:

- misleading or overconfident output
- JSON contract awkwardness for agents
- unclear text warnings
- missing trust-surface tests
- commands that claim more than the git data can prove

## Done Criteria For This Pass

- Plan artifact committed.
- Phase 1 implemented.
- Docs updated with new fields and warnings.
- `cargo fmt --all --check` passes.
- `cargo test --offline` passes.
- `cargo build --offline` passes.
- `cargo clippy --offline --all-targets -- -D warnings` passes.
- Switchboard/Latch run history shows task/review handoff and completion.
