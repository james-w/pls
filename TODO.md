# pls TODO List

## High Priority Fixes

### Watch Command Issues (commit 704eaf6 - "has issues depending on targets")
* **UX**: Run target initially before watching starts
  - Currently only startable (daemon) targets run initially
  - Non-daemon targets require a file change before first run

## Feature Enhancements

### Daemon Management
* Is it possible to reparent daemons so that the stop command is more reliable?
* Stop getting ESRCH when trying to send signal
* **Bug**: Pre-existing daemons get stopped when used as dependencies (need to track "who started this daemon")
* **Bug**: Non-daemon dependencies stop their daemon deps too early (nested cleanup issue - each run() calls run_cleanups())
* Consider: Should start_if_needed verify/start daemon dependencies even if daemon is already running?
* Consider: Inconsistency when daemon requires non-daemon which requires daemon - should transitive daemon deps stay running?

### Configuration & Target Types
* Serde flatten HashMap<String, Value> into Config to find other declared tables?
* Shell command/artifact types that runs a shell script like make
* Command artifact that runs a pls command to generate the artifact
* Smarter args handling for extends, e.g. cargo vs cargo test

### Output Management
* Outputs stored on the filesystem (for cross-invocation persistence)
* Capture stdout/stderr of some commands and only show on error, e.g. podman network/podman stop

## Technical Debt

* Scattered TODOs in various files (default_args, cwd, etc.) - see `src/context.rs:1180`, `src/targets/command/*.rs`
