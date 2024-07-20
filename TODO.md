* Cleanup manager and to_stop resolution
* Don't error if daemon is already started when it's required as a dependency
* Outputs stored on the filesystem
* Serde flatten HashMap<String, Value> into Config to find other declared tables?
* Capture stdout/stderr of some commands and only show on error, e.g. podman network/podman stop
* Shell command/artifact types that runs a shell script like make
* Command artifact that runs a pls command to generate the artifact
* Smarter args handling for extends, e.g. cargo vs cargo test
* Stop getting ESCRH when trying to send signal
* Is it possible to reparent daemons so that the stop command is more reliable?
