# `pls` - A Task Runner

`pls` runs tasks; kind of like Make, but more modern.

## Features

### Runner

* :heavy_check_mark: Define commands to run
* :heavy_check_mark: One-shot and daemon (start/stop)
* :heavy_check_mark: Dependencies - before and while
* :heavy_check_mark: Types: exec, docker, shell, cargo, ...
* :heavy_check_mark: Templates to be able to define one target, and then run it multiple ways?
* :heavy_check_mark: Arguments to allow passing extra args from cli
* :heavy_check_mark: Defaults for those arguments
* :heavy_check_mark: Env variables
* :heavy_check_mark: Return values, can be used by dependants, e.g. container name, port
* :heavy_check_mark: Status for daemons
* :soon: Logs for daemons
* :heavy_check_mark: List targets
* :heavy_check_mark: Descriptions
* :soon: Groups/tags

* :heavy_check_mark: Define artifacts to build, that are only rebuilt if needed
* :heavy_check_mark: Timestamp comparisons on files
* :heavy_check_mark: Timestamp comparison of last runtime when no file to check
* :heavy_check_mark: A way to force the build to happen

### Watcher

* :fast_forward: Standard watcher functionality
* :fast_forward: Daemon RPC
* :fast_forward: Enable/disable certain triggers
* :fast_forward: Ordering (same as dependencies?)

### UI

* :fast_forward: TUI
* :fast_forward: Log streams
* :fast_forward: Highlight failures
* :fast_forward: Parsing next/prev failure etc.

### CI Integration

* :fast_forward: Some way to flatten out to run the same commands in CI?

## Usage

Usage is driven by a file called `pls.toml`. Create this at the root of your project
(next to your `.git` directory), add add it to version control.

### Commands

The file describes a series of commands that you can run, like this:

```toml
[command.exec.hello]
command = "echo hello world"
```

This defines a command called `hello`, so we can run it with:

```console
$ pls run hello
[command.exec.hello] Running echo hello world
hello world
```

You can define a number of commands that do different things, and with different
arguments depending on your needs.

```toml
[command.exec.hello]
command = "echo hello world"

[command.exec.goodbye]
command = "echo goodbye"
```

#### Container commands

You can also specify commands that run inside containers using `podman`.

```toml
[command.container.hello]
image = 'docker.io/alpine:latest
command = echo hello
```

They can then be run in exactly the same way.

```console
$ pls run hello
[command.exec.hello] Running container using docker.io/alpine:latest
hello
```

This allows you to rely on specific versions of tools, or other cases
where using a container is preferable.

#### Arguments

#### Dependencies with `requires`

You can specify that one command needs to run after another by using
the `requires` configuration option.

```toml
[command.container.one]
image = 'docker.io/alpine:latest
command = echo one

[command.exec.two]
command = echo two
```

#### Reuse with `extends`

#### Variables

#### Long-running commands with daemons

#### Outputs

### Artifacts

#### Container Images

#### Timestamp comparisons

#### Last-run comparisons

#### Forcing a rebuild to happen

### Descriptions

## Detecting changes

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

* `updates_paths`
* `if_files_changed`
* `if_ran` use requires for this

If `updates_paths` is set then use the earliest timestamp of those
paths as the time to compare against.

If any file in `if_files_changed` is newer, or if any target in
`if_ran` was ran more recently, then run this target, else skip.

There could be more logic in the future, for e.g. docker images.

## Commands vs artifacts

Define commands that are things that you run, e.g.

```toml
[command.cargo]
command = "cargo {args}"

[container_command.cargo]
image = "{build-backend-image.tag}"
command = "cargo {args}"
requires = ["build-backend-image"]
mount = { "." = "/app", "~/.cargo/registry/index/" = "/usr/local/cargo/registry/index/", "~/.cargo/registry/cache/" = "/usr/local/cargo/registry/cache/", "~/.cargo/git/db/" = "/usr/local/cargo/git/" }
workdir = "/app"
```

Commands can extend others

```toml
[command.cargo_with_db]
extends = cargo
requires = ["dev-db-bound"]
env = ["DATABASE_URL=postgres://postgres:postgres@localhost:{dev-db-bound.output.port}/coach"]

[container_command.cargo_with_db]
extends = cargo
requires = ["dev-db"]
env = ["DATABASE_URL=postgres://postgres:postgres@{dev-db.output.name}:5432/coach"]
network = "{dev-db.output.network}"
```

Artifacts are things that you build, commands with arguments, one-shot only?

```toml
[artifact.test]
uses = "cargo_with_db"
args = "test --all"

[artifact.run]
uses = "cargo_with_db"
args = "run"
```

`pls run <COMMAND> [ARGS...]` to run a command or build an artifact
(with disambiguation). Provides the way to force a build to happen.

`pls build <ARTIFACT>` to build an artifact, taking in to account
whether deps have changed etc.
