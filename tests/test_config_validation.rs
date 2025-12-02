use assert_cmd::prelude::*;
use predicates::prelude::*;

mod common;

#[test]
fn test_empty_command_validation() {
    let test_context = common::TestContext::new();
    test_context.write_config(
        r#"
[command.exec.test]
command = ""
"#,
    );

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("test");

    let output = cmd.assert().failure();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr);

    println!("Stderr output:\n{}", stderr);

    // Check for the structured error format with target name and field path
    assert!(
        stderr.contains("Error in target 'command.exec.test':\n  command.exec.test.command:"),
        "Error should show structured format with FQN and field path, got: {}",
        stderr
    );

    // Check that we show the source line with line number
    assert!(
        stderr.contains("3 | command = \"\""),
        "Error should show line 3 with the empty command, got: {}",
        stderr
    );

    // Check for the help message
    assert!(
        stderr.contains(
            "The 'command' field cannot be an empty string.\n  Example: command = \"npm run dev\""
        ),
        "Error should show helpful example, got: {}",
        stderr
    );
}

#[test]
fn test_missing_command_validation() {
    let test_context = common::TestContext::new();
    test_context.write_config(
        r#"
[command.exec.test]
# command field omitted entirely
"#,
    );

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("test");

    let output = cmd.assert().failure();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr);

    println!("Stderr output:\n{}", stderr);

    // For omitted fields, we should still show helpful error with target info and span
    assert!(
        stderr.contains("Error in target 'command.exec.test':\n  command.exec.test.command:"),
        "Error should show structured format with FQN and field path, got: {}",
        stderr
    );

    // Should show the table header line where the field is missing
    assert!(
        stderr.contains("2 | [command.exec.test]"),
        "Error should show line 2 with the table header, got: {}",
        stderr
    );

    // Check for the help message
    assert!(
        stderr.contains(
            "The 'command' field cannot be an empty string.\n  Example: command = \"npm run dev\""
        ),
        "Error should show helpful example, got: {}",
        stderr
    );
}

#[test]
fn test_empty_env_validation() {
    let test_context = common::TestContext::new();
    test_context.write_config(
        r#"
[command.exec.test]
command = "echo hello"
env = ["FOO=bar", ""]
"#,
    );

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("test");

    let output = cmd.assert().failure();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr);

    println!("Stderr output:\n{}", stderr);

    // Check for structured error with target and env field
    assert!(
        stderr.contains("Error in target 'command.exec.test':\n  command.exec.test.env:"),
        "Error should show structured format with FQN and env field path, got: {}",
        stderr
    );

    // Check that we show the env line
    assert!(
        stderr.contains("4 | env = [\"FOO=bar\", \"\"]"),
        "Error should show line 4 with the env array, got: {}",
        stderr
    );

    // Check for the help message about env
    assert!(
        stderr.contains("Environment variable entries cannot be empty strings.\n  Example: env = [\"FOO=bar\", \"DEBUG=true\"]"),
        "Error should show helpful env example, got: {}",
        stderr
    );
}

#[test]
fn test_valid_config_loads() {
    let test_context = common::TestContext::new();
    test_context.write_config(
        r#"
[command.exec.hello]
command = "echo hello"
"#,
    );

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("hello");

    cmd.assert().success().stdout(predicate::eq("hello").trim());
}
