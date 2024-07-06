# Task Runner

## Runner

**** Define commands to run
* One-shot and daemon (start/stop)
* Dependencies - before and while
* Stamp files? Timestamp comparisons?
* Types: shell, docker, cargo, ...
* Templates to be able to define one target, and then run it multiple ways?
* Arguments to allow passing extra args from cli
* Defaults for those arguments
* Env variables
* Return values, can be used by dependants, e.g. container name, port
* Status for daemons
* Logs for daemons
* List targets
* Descriptions
* Groups/tags

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


# Detecting changes

When in watch mode you want a set of paths to trigger changes, with some excludes.

When writes are detected for those paths then trigger the relevant targets.

This should then cascade to other targets.

Therefore there are paths to trigger changes, and othe jobs to trigger changes.

The jobs ones may just be the required ones?

When not in watch mode you want to decide whether to re-run a task by detecting
if something has changed. Make does this with checking file timestamps.

Knowing the output file, and the paths that should trigger changes you can
detect changes.

If there is no output file you can use the last run time.

You can also run if another target has been run since.

Are these the same dependencies as the watch mode? I think so.

So, targets should have:
  * updates_paths:
  * if_files_changed:
  * if_ran: use requires for this

If updates_paths is set then use the earliest timestamp of those
paths as the time to compare against.

If any file in if_files_changed is newer, or if any target in
if_ran was ran more recently, then run this target, else skip.

There could be more logic in the future, for e.g. docker images.
