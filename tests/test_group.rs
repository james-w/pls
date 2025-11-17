use assert_cmd::prelude::*;
use predicates::prelude::*;

mod common;

#[test]
fn test_group_simple() {
    // Test a simple group with one dependency
    let config_src = r#"
        [command.exec.hello]
        command = "echo hello"

        [group.greet]
        description = "Run greeting"
        requires = ["hello"]
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("greet");

    cmd.assert().success().stdout(predicate::eq("hello").trim());
}

#[test]
fn test_group_multiple_deps() {
    // Test a group with multiple dependencies
    let config_src = r#"
        [command.exec.a]
        command = "echo a"

        [command.exec.b]
        command = "echo b"

        [command.exec.c]
        command = "echo c"

        [group.all]
        description = "Run all commands"
        requires = ["a", "b", "c"]
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("all");

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Verify all commands ran
    assert!(stdout.contains("a"));
    assert!(stdout.contains("b"));
    assert!(stdout.contains("c"));
}

#[test]
fn test_group_nested() {
    // Test groups with nested dependencies (group depends on group)
    let config_src = r#"
        [command.exec.a]
        command = "echo a"

        [command.exec.b]
        command = "echo b"

        [group.first]
        requires = ["a"]

        [group.second]
        requires = ["first", "b"]
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("second");

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Verify all commands ran
    assert!(stdout.contains("a"));
    assert!(stdout.contains("b"));
}

#[test]
fn test_group_cannot_build() {
    // Test that groups cannot be built
    let config_src = r#"
        [command.exec.hello]
        command = "echo hello"

        [group.greet]
        requires = ["hello"]
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("build").arg("greet");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("is not buildable"));
}

#[test]
fn test_group_cannot_start() {
    // Test that groups cannot be started
    let config_src = r#"
        [command.exec.hello]
        command = "echo hello"

        [group.greet]
        requires = ["hello"]
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("start").arg("greet");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("is not startable"));
}

#[test]
fn test_group_with_description() {
    // Test that group description shows in list
    let config_src = r#"
        [command.exec.hello]
        command = "echo hello"

        [group.greet]
        description = "A friendly greeting group"
        requires = ["hello"]
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("list");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("greet"))
        .stdout(predicate::str::contains("A friendly greeting group"));
}

#[test]
fn test_group_with_diamond_dependency() {
    // Test that shared deps in group are only run once
    let config_src = r#"
        [command.exec.shared]
        command = "echo shared"

        [command.exec.a]
        command = "echo a"
        requires = ["shared"]

        [command.exec.b]
        command = "echo b"
        requires = ["shared"]

        [group.all]
        requires = ["a", "b"]
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("all");

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // shared should only appear once despite being required by both a and b
    let shared_count = stdout
        .lines()
        .filter(|line| line.trim() == "shared")
        .count();
    assert_eq!(
        shared_count, 1,
        "Shared dependency should only execute once, but executed {} times",
        shared_count
    );
}
