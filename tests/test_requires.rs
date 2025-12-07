use assert_cmd::prelude::*;
use predicates::prelude::*;

mod common;

#[test]
fn test_requires() {
    let config_src = r#"
        [command.exec.hello]
        command = "echo hello"

        [command.exec.world]
        command = "echo world"
        requires = ["hello"]
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("world");

    // Verify both commands ran and in the correct order (hello before world)
    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let hello_pos = stdout.find("hello").expect("hello should appear in output");
    let world_pos = stdout.find("world").expect("world should appear in output");
    assert!(
        hello_pos < world_pos,
        "hello should appear before world in output"
    );
}

#[test]
fn test_diamond_dependency() {
    // Test that shared dependency D is only executed once
    // A requires [B, C]; B requires [D]; C requires [D]
    let config_src = r#"
        [command.exec.d]
        command = "echo d"

        [command.exec.b]
        command = "echo b"
        requires = ["d"]

        [command.exec.c]
        command = "echo c"
        requires = ["d"]

        [command.exec.a]
        command = "echo a"
        requires = ["b", "c"]
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("a");

    // d should only appear once in output despite being required by both b and c
    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Count occurrences of 'd'
    let d_count = stdout.lines().filter(|line| line.trim() == "d").count();
    assert_eq!(
        d_count, 1,
        "Dependency 'd' should only be executed once, but was executed {} times",
        d_count
    );

    // Verify all commands ran
    assert!(stdout.contains("d"));
    assert!(stdout.contains("b"));
    assert!(stdout.contains("c"));
    assert!(stdout.contains("a"));
}

#[test]
fn test_circular_dependency_direct() {
    // Test direct circular dependency: A requires B, B requires A
    let config_src = r#"
        [command.exec.a]
        command = "echo a"
        requires = ["b"]

        [command.exec.b]
        command = "echo b"
        requires = ["a"]
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("a");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Circular dependency"));
}

#[test]
fn test_circular_dependency_indirect() {
    // Test indirect circular dependency: A -> B -> C -> A
    let config_src = r#"
        [command.exec.a]
        command = "echo a"
        requires = ["b"]

        [command.exec.b]
        command = "echo b"
        requires = ["c"]

        [command.exec.c]
        command = "echo c"
        requires = ["a"]
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("a");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Circular dependency"));
}

#[test]
fn test_self_dependency() {
    // Test that a target cannot require itself
    let config_src = r#"
        [command.exec.a]
        command = "echo a"
        requires = ["a"]
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("a");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Circular dependency"));
}
