# Task Runner

## Runner

**** Define commands to run
* One-shot and daemon (start/stop)
* Dependencies - before and while
* Stamp files? Timestamp comparisons?
* Types: shell, docker, cargo, ...
* Arguments, defaults, env variables
* Return values, can be used by dependants, e.g. container name, port

## Watcher

* Standard watcher functionality
* Daemon RPC
* Enable/disable certain triggers
* Ordering (same as dependencies?)

## UI

* TUI
* Log streams
* Highlight failures
* Parsing next/prev failure etc.

## CI Integration

* Some way to flatten out to run the same commands in CI?
