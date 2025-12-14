use assert_cmd::prelude::*;
use predicates::prelude::*;

mod common;

#[test]
fn test_variables() {
    let config_src = r#"
        [command.exec.hello]
        command = "echo hello {place}"
        variables = { place = "world" }
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("hello");

    // Windows echo may add trailing spaces and uses CRLF
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("hello world"));
}

#[test]
fn test_globals() {
    let config_src = r#"
        [globals]
        place = "world"

        [command.exec.hello]
        command = "echo hello {globals.place}"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("hello");

    // Windows echo may add trailing spaces and uses CRLF
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("hello world"));
}

#[test]
fn test_globals_platform_override() {
    let config_src = r#"
        [globals]
        tool = "./tool"

        [globals.platform.windows]
        tool = "./tool-windows"

        [globals.platform.linux]
        tool = "./tool-linux"

        [globals.platform.macos]
        tool = "./tool-macos"

        [command.exec.run]
        command = "echo {globals.tool}"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("run");

    #[cfg(windows)]
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("./tool-windows"));

    #[cfg(target_os = "linux")]
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("./tool-linux"));

    #[cfg(target_os = "macos")]
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("./tool-macos"));
}

#[test]
fn test_target_variables_platform_override() {
    let config_src = r#"
        [command.exec.run]
        command = "echo {binary}"

        [command.exec.run.variables]
        binary = "./app"

        [command.exec.run.variables.platform.windows]
        binary = "./app-windows"

        [command.exec.run.variables.platform.linux]
        binary = "./app-linux"

        [command.exec.run.variables.platform.macos]
        binary = "./app-macos"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("run");

    #[cfg(windows)]
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("./app-windows"));

    #[cfg(target_os = "linux")]
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("./app-linux"));

    #[cfg(target_os = "macos")]
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("./app-macos"));
}

#[test]
fn test_platform_reserved_word_error() {
    let config_src = r#"
        [globals]
        platform = "production"

        [command.exec.test]
        command = "echo {globals.platform}"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("test");

    // The error comes from serde during parsing, not from validation
    cmd.assert()
        .failure()
        .stderr(
            predicate::str::contains("platform").and(
                predicate::str::contains("invalid type")
                    .or(predicate::str::contains("expected struct")),
            ),
        );
}

#[test]
fn test_unknown_platform_error() {
    let config_src = r#"
        [globals]
        tool = "./tool"

        [globals.platform.freebsd]
        tool = "./tool-freebsd"

        [command.exec.test]
        command = "echo {globals.tool}"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("test");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("unknown field").or(predicate::str::contains("freebsd")));
}

#[test]
fn test_platform_override_empty_key_validation() {
    let config_src = r#"
        [globals]
        tool = "./tool"

        [globals.platform.windows]
        "" = "invalid"

        [command.exec.test]
        command = "echo {globals.tool}"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("test");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("key cannot be empty"));
}

#[test]
fn test_platform_override_empty_value_allowed() {
    // Test that empty values are allowed in platform overrides
    // This allows clearing a variable on specific platforms
    let config_src = r#"
        [globals]
        tool_args = "--verbose"

        [globals.platform.linux]
        tool_args = ""

        [command.exec.test]
        command = "echo {globals.tool_args}"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("test");

    // Should succeed - empty values are allowed
    cmd.assert().success();
}

#[test]
fn test_target_platform_override_empty_key_validation() {
    let config_src = r#"
        [command.exec.test]
        command = "echo {tool}"

        [command.exec.test.variables]
        tool = "./tool"

        [command.exec.test.variables.platform.macos]
        "" = "invalid"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("test");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("key cannot be empty"));
}
