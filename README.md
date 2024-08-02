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

* :arrow_forward: Standard watcher functionality
* :fast_forward: Daemon RPC
* :fast_forward: Enable/disable certain triggers
* :arrow_forward: Ordering (same as dependencies?)

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

A command can take arguments passed from the command line.

```toml
[command.exec.echo_something]
command = "echo {args}"
```

```console
$ pls run echo_something hello
[command.exec.hello] Running echo hello
hello
```

If you don't specify `{args}` in the command then they will be
appended to the command.

#### Dependencies with `requires`

You can specify that one command needs to run after another by using
the `requires` configuration option.

```toml
[command.container.one]
image = "docker.io/alpine:latest"
command = "echo one"

[command.exec.two]
command = "echo two"
requires = "one"
```

This will run the required command first.

```console
$ pls run two
[command.container.one] Running with image docker.io/alpine:latest
[command.container.two] Running command "echo two"
two
```

#### Reuse with `extends`

There are many cases where you want to have similar commands with slight differences. This is supported with the `extends` option:

```toml
[command.exec.cargo]
command = "cargo"

[command.exec.test]
extends = "cargo"
args = "test"
```

```console
$ pls run test
[command.exec.test] Running command "cargo test"
...
```

#### Variables

There are times when you want to avoid repeating something in the configuration, for instance the path to a file. For that there
are variables that are substituted into commands and more.

```toml
[command.exec.write_config]
command = ./write_base_config {config_file}
variables = { "config_file" = "base_config.yaml" }
```

```console
$ pls run write_config
[command.exec.write_config] Running command "./write_base_config base_config.yaml"
...
```

You can then also refer to these variables from other targets:

```toml
[command.exec.show_config]
command = cat {write_config.config_file}
```

You can also specify global variables for things that don't belong to a single target:

```toml
[globals]
project_name = "foo"

[command.exec.show_project_name]
command = echo {globals.project_name}
```

#### Long-running commands with daemons

Sometimes the commands that you want to run are long-running, and are run in the
background while doing other things, for instance dev servers. You define those
commands as normal, but set the `daemon` option:

```toml
[command.exec.dev]
command = npm run dev
daemon = true
```

You can choose to run these commands as normal, and they will run in the foreground:

```console
$ pls run dev
[command.exec.dev] Running command "npm run dev"
...
^C
```

However, when a command is defined as a daemon then you can also use the `start`
and `stop` commands to run it in the backgound:

```console
$ pls start dev
[command.exec.dev] Starting ...
$ pls stop dev
[command.exec.dev] Stopping ...
```

In addition, when one command `requires` a command that is defined as a daemon
then it will be started as a pre-requisite, and then stopped after.

```console
$ pls run integration_tests
[command.exec.db] Starting ...
[command.exec.integration_tests] Running command npm test
...
[command.exec.db] Stopping ...
```

#### Outputs

Certain commands produce `outputs`. These are similar to variables, but are defined
at runtime depending on what the command does. Currently the only supported outputs
are for containers:

| Output  | Description                                  |
|---------|----------------------------------------------|
| name    | The name of the container                    |
| network | The name of the network if one was requested |

```toml
[command.container.db]
image = "postgres"
create_network = true

[command.container.integration_tests]
image = "myimage"
network = "{db.outputs.network}"
```

This will allow the second container to run in the same network as the first while
still allowing that network to be dynamic.

### Artifacts

One of the most useful features of `make` is to avoid re-running commands if there's no need to.
`pls` supports a similar concept based around "artifacts." These are very similar to commands,
but produce a defined output. This allows for checks to be done to avoid the commands being re-run.
There are some limitations though, for instance args aren't supported, as they would change
the artifact that was produced, and so invalidate the comparisons.

Artifacts are built with the `build` command, or can also be ran with the `run` command to force
it to be rebuilt.

#### Exec artifacts

There is an `exec` artifact type, much like the `exec` command type.

```toml
[artifact.exec.build]
command = "./build"
```

```console
$ pls build
[artifact.exec.build] Building with command ./build
...
$ pls build
[artifact.exec.build] Up to date
```

#### Container Images

Another artifact type is a container image. This allows for a container image to be built using
`podman`.

```toml
[artifact.container_image.foo]
context = "./container"
tag = "myimage"
```

#### Timestamp comparisons

Timestamp comparisons are similar to `make`. A target defines `if_files_changed` as an array of paths
that should force a rebuild if they have changed.

```toml
[artifact.exec.build]
...
if_files_changed = ["src/*"]
```

A target can also define `updates_paths`, which are the paths that the command updates. If these
are defined then they will be used in the comparison to see if the artifact should be rebuilt.
If `updates_paths` is not defined then the last run time of the artifact will be compared
with the files in `if_files_changed`.

#### Last-run comparisons

Sometimes there aren't files that can be tracked, or it's a lot of effort to do so. In that case
you can fall back to comparison of the last-run time of targets. If you define an artifact
without `if_files_changed` then the timestamps of the dependencies specified in `requires` will
be used to decide whether to rebuild instead.

#### Forcing a rebuild to happen

Sometimes you want to rebuild an artifact, even if the dependencies haven't changed. In those
cases just use the `run` command to build the artifact. Note that this only forces a rebuild
of the specified target, any artifacts in the dependency chain will still be checked to see
if they should be rebuilt.

### Descriptions

Each target can have a description provided. This can help with remembering the purpose of a target,
or to provide more information about how to use it. This is supported by all target types.

```toml
[artifact.container_image.base_image]
...
description = "Build the base image used for all other images in the project"
```

## Watch Mode

When in a core development loop it's useful to have a "watch" running that triggers actions
as needed based on the changes that you are making. This avoids you having to remember which
commands to run in each case, and can also provider faster feedback to keep you in the flow
state.

There are many commands available that offer this functionality, but `pls` can provide
that functionality without having to repeat dependencies and mappings of which files
should cause each target to be re-run.

To use this run

```console
$ pls watch target1
```

This will run in the foreground and trigger targets to run as files change.

In order for this to work well you need to set up the `if_files_changed` and
`requires` dependencies for your project. The benefit is that by getting these
right once you get the watch functionality and much more, and it works for
any target.
