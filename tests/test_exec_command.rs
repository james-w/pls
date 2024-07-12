use assert_cmd::prelude::*;
use predicates::prelude::*;

mod common;

#[test]
fn test_exec_command() {
    let config_src = r#"
        [command.exec.hello]
        command = "echo hello"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("hello");

    cmd.assert().success().stdout(predicate::eq("hello").trim());
}

#[test]
fn test_with_args() {
    let config_src = r#"
        [command.exec.hello]
        command = "echo {args} hello"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("hello").arg("world");

    cmd.assert()
        .success()
        .stdout(predicate::eq("world hello").trim());
}

#[test]
fn test_extends() {
    let config_src = r#"
        [command.exec.hello]
        command = "echo hello"

        [command.exec.world]
        extends = "hello"
        command = "echo world"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("world");

    cmd.assert().success().stdout(predicate::eq("world").trim());
}
