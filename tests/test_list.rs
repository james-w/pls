use assert_cmd::prelude::*;
use predicates::prelude::*;

mod common;

#[test]
fn test_list() {
    let config_src = r#"
        [artifact.exec.copy]
        command = "cp hello world"

        [command.container.hello]
        image = "alpine"
        description = "Hello world"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("list");

    cmd.assert().success().stdout(predicate::eq(
        "artifact.exec.copy - \ncommand.container.hello - Hello world\n",
    ));
}

#[test]
fn test_list_alias_ls() {
    let config_src = r#"
        [artifact.exec.copy]
        command = "cp hello world"

        [command.container.hello]
        image = "alpine"
        description = "Hello world"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("ls");

    cmd.assert().success().stdout(predicate::eq(
        "artifact.exec.copy - \ncommand.container.hello - Hello world\n",
    ));
}

#[test]
fn test_list_search_by_name() {
    let config_src = r#"
        [artifact.exec.copy]
        command = "cp hello world"

        [command.container.hello]
        image = "alpine"
        description = "Hello world"

        [command.exec.build]
        command = "make"
        description = "Build project"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("list").arg("copy");

    cmd.assert()
        .success()
        .stdout(predicate::eq("artifact.exec.copy - \n"));
}

#[test]
fn test_list_search_by_description() {
    let config_src = r#"
        [artifact.exec.copy]
        command = "cp hello world"

        [command.container.hello]
        image = "alpine"
        description = "Hello world"

        [command.exec.build]
        command = "make"
        description = "Build project"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("list").arg("world");

    cmd.assert()
        .success()
        .stdout(predicate::eq("command.container.hello - Hello world\n"));
}

#[test]
fn test_list_search_case_insensitive() {
    let config_src = r#"
        [command.container.hello]
        image = "alpine"
        description = "Hello World"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("list").arg("HELLO");

    cmd.assert()
        .success()
        .stdout(predicate::eq("command.container.hello - Hello World\n"));
}

#[test]
fn test_list_search_no_results() {
    let config_src = r#"
        [artifact.exec.copy]
        command = "cp hello world"

        [command.container.hello]
        image = "alpine"
        description = "Hello world"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("list").arg("nonexistent");

    cmd.assert().success().stdout(predicate::eq(""));
}
