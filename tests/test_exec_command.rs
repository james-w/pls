use assert_cmd::prelude::*;
use assert_fs::prelude::*;
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

#[test]
fn test_env() {
    let config_src = r#"
        [command.exec.env]
        command = "env"
        env = ["HELLO=world"]
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("env");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("HELLO=world").trim());
}

#[test]
fn test_dir_option() {
    let config_src = r#"
        [command.exec.pwd_in_subdir]
        command = "pwd"
        dir = "subdir"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);
    test_context.workdir.child("subdir").create_dir_all().unwrap();

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("pwd_in_subdir");

    let expected_path = test_context.workdir.path().join("subdir");
    cmd.assert()
        .success()
        .stdout(predicate::eq(expected_path.to_str().unwrap()).trim());
}

#[test]
fn test_dir_with_variable() {
    let config_src = r#"
        [command.exec.pwd_in_var_dir]
        command = "pwd"
        dir = "{test_dir}"
        variables = { test_dir = "subdir" }
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);
    test_context.workdir.child("subdir").create_dir_all().unwrap();

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("pwd_in_var_dir");

    let expected_path = test_context.workdir.path().join("subdir");
    cmd.assert()
        .success()
        .stdout(predicate::eq(expected_path.to_str().unwrap()).trim());
}

#[test]
fn test_dir_default_is_cwd() {
    let config_src = r#"
        [command.exec.pwd_no_dir]
        command = "pwd"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("pwd_no_dir");

    let expected_path = test_context.workdir.path();
    cmd.assert()
        .success()
        .stdout(predicate::eq(expected_path.to_str().unwrap()).trim());
}
