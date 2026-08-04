# Sentinel Agent-First Improvement Plan

Status: proposed
Date: 2026-08-04
Owners: Bjarn architecture/plan, Helix implementation review and command verification, Mark product direction

## Communication Contract

Preferred project comms:

- Switchboard room: `project.sentinel`
- Switchboard thread: `sentinel-agent-first-pass`
- Meridian A2A: wake-up and direct check-in only
- Latch: durable coordination truth for claims, tasks, decisions, and completion

Reasoning:

Switchboard is the better project ledger because it captures the room/thread
history and can produce run packets. Meridian A2A is still the best wake path
when an agent needs to look now. Latch remains the source of truth for who owns
what and why.

## Product Goal

Sentinel should be easy for agents to consume without prose inference. Its
agent-facing output should answer:

- can I proceed?
- should I refresh the matrix first?
- do I need validation before editing or committing?
- should I stop and ask for human/agent review?
- which files are risky, unknown, stale, or poorly supported by evidence?
- what exact commands should I run next?

The agent-first version should still be useful for humans, but it should not
force agents to scrape natural-language advice.

The core promise is a deterministic gate: an agent should be able to run one
preflight command, inspect stable JSON fields, and decide whether to proceed,
refresh, validate, request review, or stop.

## Current Strengths

Sentinel already has the right core product shape:

- local-only git-history evidence
- no oracle claims
- file-level risk rows
- matrix health and confidence warnings
- unknown-file coverage via `known_in_matrix`
- `sentinel doctor` as a one-shot preflight

## Current Agent Gaps

### 1. No Stable Gate Object

Agents still need to interpret fields and advice text to decide whether to
proceed.

Need:

```json
{
  "gates": {
    "matrix_fresh": false,
    "confidence_ok": false,
    "unknown_files": true,
    "high_risk_changed": false,
    "medium_risk_changed": true,
    "dirty_now": true
  }
}
```

### 2. No Top-Level Action Level

Agents should get a simple action classification.

Proposed values:

- `none`: normal validation is enough
- `refresh`: run `sentinel scan --force` first
- `validate`: proceed only after targeted/full validation
- `review`: ask another agent/human before proceeding
- `stop`: do not proceed automatically

Precedence must be explicit and centralized:

```text
stop > review > refresh > validate > none
```

Mixed states are normal. For example, a stale matrix plus an unknown changed
file should report `action_level = refresh`, because stale risk rows should be
refreshed before an agent interprets the unknown-file validation requirement.
The lower-priority gate should still remain visible in `gates` and
`recommended_commands`.

Examples:

- stale matrix + unknown changed file -> `refresh`
- fresh matrix + high-risk changed file -> `review`
- fresh matrix + unknown changed file -> `validate`
- fresh matrix + no risky/unknown changed files -> `none`

### 3. No Strict Exit-Code Mode

Harnesses need a gateable command.

Proposed:

```sh
sentinel doctor --strict
```

Exit codes:

- `0`: proceed
- `10`: refresh needed
- `20`: validation needed
- `30`: review/stop needed

Strict mode should print the same JSON/text output, but exit according to the
action level.

Strict mode must not turn a gate into a program failure. For example,
`sentinel doctor --strict --format json` should still emit a normal successful
report with `ok: true`, `action_level`, `gates`, and recommendations, then exit
with the gate code. Reserve `ok: false` and normal CLI errors for cases where
Sentinel itself failed to run.

Text mode should also print the normal doctor report before exiting with the
gate code.

### 4. Unknown Files Need A Coverage Enum

`known_in_matrix: false` works, but simple agents may still sort unknown files
as quiet because `risk_score` is `0`.

Add:

```json
"coverage_status": "known" | "unknown"
```

Keep `known_in_matrix` for backward compatibility.

Unknown changed files should imply `validate` even when the score is `0`.
Unknown is not the same as high risk, but it is not enough evidence for a quiet
handoff.

### 5. Recommendations Should Be Commands

The current `recommendations` list is helpful, but agents need command-shaped
items where possible.

Proposed:

```json
{
  "recommended_commands": [
    {
      "kind": "command",
      "command": "sentinel scan --force",
      "argv": ["sentinel", "scan", "--force"],
      "reason": "matrix is stale",
      "required": true
    }
  ]
}
```

Not every recommendation is machine-runnable. Manual recommendations should be
typed honestly:

```json
{
  "kind": "manual",
  "command": null,
  "argv": null,
  "label": "Run focused validation for unknown changed files",
  "reason": "one changed file is not represented in the matrix",
  "required": true
}
```

### 6. Risk Sorting Should Be Agent-Oriented

Changed-file risks should sort by action urgency:

1. unknown coverage
2. high risk
3. medium risk
4. low risk
5. quiet known files

Then sort by score descending, then path ascending.

For explicit `risk --file ...` queries, preserve input order or include
`input_order`/`rank` so humans and agents can map results back to the requested
file list without guessing.

### 7. Score Breakdown Should Be Auditable

Agents should know why a score exists without parsing reason text.

Add a `score_breakdown` object:

- `commits`
- `recent`
- `failure_like`
- `revert`
- `test_cochange`
- `churn`
- `total`

This keeps Sentinel honest and makes future tuning easier.

Add `schema_version` and `scoring_version` near the doctor/risk payload so
agents can compare packets over time without assuming a score means the same
thing forever.

## Gate Definitions

Define these in one pure derivation function and cover them with table-driven
tests:

- `matrix_fresh`: matrix has a `head_sha` and it matches the current repo head.
- `confidence_ok`: matrix confidence is not `low`.
- `unknown_files`: at least one changed or explicitly queried file has
  `coverage_status = unknown`.
- `high_risk_changed`: at least one changed file is high risk.
- `medium_risk_changed`: at least one changed file is medium risk.
- `dirty_now`: the working tree is dirty when the doctor command runs.

`dirty_now` should be informational by itself. Sentinel is expected to run while
the tree is dirty; dirtiness should influence the gate through stale, unknown,
medium-risk, or high-risk evidence, not because dirty files exist at all.

## Recommended Implementation Slice

Implement this in one small pass:

1. Add model fields:
   - `schema_version`
   - `scoring_version`
   - `coverage_status`
   - `score_breakdown`
   - `AgentGates`
   - `ActionLevel`
   - `RecommendedCommand`
   - `AgentDoctor`

2. Add one pure action derivation function:
   - input: matrix health, changed-file risks, warnings, and current dirty state
   - output: `gates`, `action_level`, and recommended actions
   - tests: stale+unknown, fresh+high-risk, fresh+unknown, clean/no-risk

3. Add `sentinel doctor --strict`.
   - strict JSON/text output remains normal output
   - exit code follows the gate
   - gate exits are not represented as Sentinel runtime errors

4. Make `doctor --format json` include:
   - `action_required`
   - `action_level`
   - `gates`
   - `recommended_commands`
   - sorted `changed_file_risks`

5. Make `risk --format json` include:
   - coverage status
   - score breakdown
   - schema/scoring version
   - either sorted risk order or explicit `input_order`/`rank`

6. Keep existing fields backward-compatible.

7. Update README/SPEC.

8. Add regression tests:
   - stale matrix produces `action_level = refresh`
   - unknown file produces `coverage_status = unknown`
   - unknown changed file produces `action_level = validate` when the matrix is fresh
   - medium-risk changed file produces `action_level = validate`
   - high risk produces `action_level = review`
   - strict mode exits nonzero when refresh/validate/review is needed while still printing a successful report

## Helix Role

Helix should:

- run the current Sentinel command suite before implementation review
- review the plan for agent-contract awkwardness
- run the implemented code after Bjarn or Helix lands the slice
- check JSON fields for agent usability
- verify strict exit codes
- report findings back to Bjarn in Switchboard and/or Meridian

For this planning pass, Helix's role is read-only review plus command
verification. If Mark opens the build pass, Helix can take the implementation
slice from this document and keep edits scoped to Sentinel only.

## Helix Review Receipt

Helix completed a read-only review on 2026-08-04 and ran the current Sentinel
commands before implementation:

- `cargo test --offline`: passed, 11/11.
- `sentinel doctor --format json`: current truth surface reports stale matrix,
  low confidence, dirty working tree, thin history warning, and the new plan
  file as unknown.
- `sentinel doctor`: text output is clear and agent-usable.
- `sentinel risk --file README.md --file DOES_NOT_EXIST.md --format json`:
  unknown files are exposed with `known_in_matrix = false`, and advice leads
  with stale/unverified matrix state.

Review changes folded into this plan:

- explicit `action_level` precedence
- strict mode reports successful gate output before nonzero gate exit
- `recommended_commands` include `kind`, `command`, and `argv`
- manual recommendations are typed separately from runnable commands
- gate definitions are normative and testable
- unknown changed files require validation
- schema/scoring versions are part of the JSON contract
- explicit risk queries preserve input mapping or expose rank/order

## Non-Goals For This Slice

- Do not ingest CI, witness, or test-failure evidence yet.
- Do not add a daemon.
- Do not make Sentinel a test runner.
- Do not add network dependencies.
- Do not remove text output.

## Done Criteria

- Plan reviewed by Helix.
- Latch task tracks Helix review/execution.
- Switchboard thread records start, findings, and handoff.
- Code pass, if approved, is committed.
- `cargo fmt --all --check` passes.
- `cargo test --offline` passes.
- `cargo build --offline` passes.
- `cargo clippy --offline --all-targets -- -D warnings` passes.
- `sentinel doctor --format json` smoke passes.
- `sentinel doctor --strict` exit behavior is verified.
