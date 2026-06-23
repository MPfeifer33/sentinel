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
```

After installation, replace `cargo run --` with `sentinel`.

## Commands

### scan

```sh
sentinel scan
sentinel scan --force
sentinel scan --limit 500 --force
```

Builds `.agent-sentinel/matrix.json` from git history.

### risk

```sh
sentinel risk
sentinel risk --changed
sentinel risk --file src/lib.rs --file tests/lib.rs
```

Reports risk for explicit files. With no `--file`, it inspects files changed
relative to `HEAD`, including untracked files.

### matrix

```sh
sentinel matrix
sentinel matrix --top 50
sentinel matrix --format json
```

Shows the highest-risk files in the stored matrix.

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

## Typical Agent Flow

```sh
probe doctor
sentinel scan --force
sentinel risk
sieve analyze
cargo test
rivet check --intent "finish the current change"
```

Use `sentinel` before and after editing: first to know which paths deserve extra
care, then to make sure risky files received appropriate validation.
