# pls TODO List

## High Priority Fixes

### Watch Command Issues (commit 704eaf6 - "has issues depending on targets")
* **IMPORTANT**: Propagate args through `and_then` dependency chain
  - Location: `src/cmd/watch.rs:119-126` - TODO comment, passes `vec![]`
  - Impact: Downstream targets in watch don't receive command-line arguments
* **UX**: Run target initially before watching starts
  - Currently only startable (daemon) targets run initially
  - Non-daemon targets require a file change before first run

## Commands to Implement

### Logs Command ✅ (Ready to implement - infrastructure exists)
* **Status**: Infrastructure complete - stdout/stderr already captured to `.pls/<target>/log`
* **What exists**:
  - Log files created at `.pls/<target-name>/log` (src/commands.rs:155-165)
  - Both exec and container commands capture output
  - Metadata directory structure in place
* **What to build**:
  - New command in `src/cmd/logs.rs`
  - Basic: read and display log file
  - Flag: `-n/--tail` for last N lines (default 10)
  - Flag: `-f/--follow` for tail -f behavior
  - Error handling: target not found, not a daemon, not started, empty log
* **Files to modify**:
  - NEW: `src/cmd/logs.rs`
  - MODIFY: `src/cmd/mod.rs` line 67 (replace TODO)
* **Estimate**: 2-4 hours, LOW-MEDIUM complexity
* **No blockers**: Can be implemented independently

## Feature Enhancements

### Daemon Management
* Don't error if daemon is already started when it's required as a dependency
* Is it possible to reparent daemons so that the stop command is more reliable?
* Stop getting ESRCH when trying to send signal

### Configuration & Target Types
* Serde flatten HashMap<String, Value> into Config to find other declared tables?
* Shell command/artifact types that runs a shell script like make
* Command artifact that runs a pls command to generate the artifact
* Smarter args handling for extends, e.g. cargo vs cargo test

### Output Management
* Outputs stored on the filesystem (for cross-invocation persistence)
* Capture stdout/stderr of some commands and only show on error, e.g. podman network/podman stop

## Technical Debt

### Cleanup Manager Pattern (Repeated in multiple locations)
* Cleanup manager and to_stop resolution
* Multiple TODOs in `src/target.rs` lines: 211, 215, 276, 279, 306, 310, 348, 352, 532, 536, 555, 668, 690, 694, 729, 733, 766
  - "TODO: use cleanup manager to handle the to_stop stuff?"
  - "TODO: add in errors to result"
* Other scattered TODOs: `src/context.rs:1180`, various in `src/targets/command/*.rs`
