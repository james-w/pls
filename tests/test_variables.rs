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
