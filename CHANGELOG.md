# Changelog

All notable changes to this project will be documented in this file.

## [0.1.0] - 2025-01-29

### Added
- Initial release with task runner functionality
- Support for exec, shell, container, and cargo commands
- File-based configuration via pls.toml
- Watch mode for file-triggered task execution
- Daemon mode for long-running services (start/stop)
- Task dependency resolution with topological sort
- File timestamp-based build caching for artifacts
- Working directory support for commands and artifacts
- Environment variable support
- Template-based command reuse with `extends`
- Command arguments with defaults
- Full-text search in task list
- Container support via Podman
- Automated multi-platform releases (Linux, macOS)

[GitHub Release](https://github.com/james-w/pls/releases/tag/v0.1.0)
