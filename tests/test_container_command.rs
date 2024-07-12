use assert_cmd::prelude::*;
use predicates::prelude::*;

mod common;

#[test]
fn test_container_command() {
    let config_src = r#"
        [command.container.hello]
        image = "docker.io/alpine:latest"
        command = "echo hello"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("hello");

    cmd.assert().success().stdout(predicate::eq("hello").trim());
}

#[test]
fn test_extends() {
    let config_src = r#"
        [command.container.hello]
        image = "docker.io/alpine:latest"

        [command.container.world]
        extends = "hello"
        command = "echo world"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("world");

    cmd.assert().success().stdout(predicate::eq("world").trim());
}
