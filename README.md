# sentinel

`sentinel` is a regression risk watcher for agent workflows. It reads git
history, builds a fragility matrix, and warns when the files an agent is about
to touch have historically behaved like risky files.

It answers:

```text
Before I edit or commit this file, how nervous should I be?
```

## Quickstart

```sh
cargo build

# Build the matrix from recent git history.
cargo run -- scan --force

# Check current changed files.
cargo run -- risk

# Check explicit files.
cargo run -- risk --file src/main.rs

# Machine-readable output.
cargo run -- risk --file src/main.rs --format json

# One-shot agent preflight.
cargo run -- doctor --format json
```

After installation, replace `cargo run --` with `sentinel`.

## Commands

### scan

```sh
sentinel scan
sentinel scan --force
sentinel scan --limit 500 --force
```

Builds `.agent-sentinel/matrix.json` from git history as a local cache.
When Sentinel writes the matrix, it idempotently adds `.agent-sentinel/` to the
target repo's `.git/info/exclude` so generated matrix files stay out of
project commits.
The stored matrix records the git `HEAD` it was built from and whether the
worktree had changed files at scan time.

### risk

```sh
sentinel risk
sentinel risk --changed
sentinel risk --file src/lib.rs --file tests/lib.rs
```

Reports risk for explicit files. With no `--file`, it inspects files changed
relative to `HEAD`, including untracked files.
JSON output includes `matrix_health` so agents can tell whether the matrix is
fresh, stale, low-confidence, or generated from sparse history.

### matrix

```sh
sentinel matrix
sentinel matrix --top 50
sentinel matrix --format json
```

Shows the highest-risk files in the stored matrix.
If the matrix is stale or low-confidence, text output prints warnings before the
file list.

### tests

```sh
sentinel tests src/lib.rs
```

Shows tests historically co-changed with a source file.

### status

```sh
sentinel status
```

Shows the storage path and data sources.
Also reports matrix confidence, staleness, commits scanned, tracked files, and
current changed-file count when a matrix is available.

### doctor

```sh
sentinel doctor
sentinel doctor --format json
```

Runs an agent-facing preflight: matrix health, changed-file risk, advice, and
recommended next commands.

## Signals

The MVP scores files from signals that git can prove locally:

- commit frequency
- recent commit frequency
- failure-like commit subjects such as `fix`, `regression`, `panic`, `flake`
- revert/rollback commit subjects
- line churn from `git diff --numstat`
- source files co-changed with tests

`sentinel` does not claim to know real test failures unless future evidence
sources provide them. The current matrix is a historically grounded risk hint,
not an oracle.

## Local Cache Policy

`.agent-sentinel/matrix.json` is generated local cache. It should not be
committed as project evidence. Sentinel keeps it out of normal git status by
maintaining `.agent-sentinel/` in the repo-local `.git/info/exclude` file.

Durable coordination evidence belongs in Switchboard/Latch history, or in a
future explicit Sentinel export/snapshot command that marks the artifact as
intentional.

## Trust Signals

Sentinel now reports matrix health alongside risk data:

- `stale` when the repo `HEAD` differs from the scan `HEAD`
- `confidence` lowered for stale or thin-history matrices
- `warnings` for stale matrices, very small history windows, or dirty scans
- `dirty_now` when the current worktree has changed or untracked files
- `known_in_matrix: false` for files with no historical signal

Unknown files are not "proven safe." They are files Sentinel has not seen in the
scanned local history.

## Typical Agent Flow

```sh
probe doctor
sentinel doctor
sentinel scan --force
sentinel risk
sieve analyze
cargo test
rivet check --intent "finish the current change"
```

Use `sentinel` before and after editing: first to know which paths deserve extra
care, then to make sure risky files received appropriate validation.
