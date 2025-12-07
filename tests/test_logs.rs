use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::thread;
use std::time::Duration;

mod common;

#[test]
fn test_logs_for_daemon() {
    let config_src = format!(
        r#"
        [command.exec.log_daemon]
        command = "{}"
        daemon = true
    "#,
        common::echo_lines_and_sleep(&["line1", "line2", "line3"], 1)
    );

    let test_context = common::TestContext::new();
    test_context.write_config(&config_src);

    // Start the daemon
    let mut cmd = test_context.get_command();
    cmd.arg("start").arg("log_daemon");
    cmd.assert().success();

    // Give it time to write logs
    thread::sleep(Duration::from_millis(200));

    // View logs
    let mut cmd = test_context.get_command();
    cmd.arg("logs").arg("log_daemon");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("line1"))
        .stdout(predicate::str::contains("line2"))
        .stdout(predicate::str::contains("line3"));

    // Stop the daemon
    let mut cmd = test_context.get_command();
    cmd.arg("stop").arg("log_daemon");
    cmd.assert().success();
}

#[test]
fn test_logs_with_tail_flag() {
    let config_src = format!(
        r#"
        [command.exec.log_daemon]
        command = "{}"
        daemon = true
    "#,
        common::echo_loop_and_sleep(20, 1)
    );

    let test_context = common::TestContext::new();
    test_context.write_config(&config_src);

    // Start the daemon
    let mut cmd = test_context.get_command();
    cmd.arg("start").arg("log_daemon");
    cmd.assert().success();

    // Give it time to write logs
    thread::sleep(Duration::from_millis(200));

    // View last 5 lines
    let mut cmd = test_context.get_command();
    cmd.arg("logs").arg("-n").arg("5").arg("log_daemon");
    let output = cmd.assert().success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);

    // Should contain last 5 lines
    assert!(stdout.contains("line16"));
    assert!(stdout.contains("line17"));
    assert!(stdout.contains("line18"));
    assert!(stdout.contains("line19"));
    assert!(stdout.contains("line20"));

    // Should not contain early lines
    assert!(!stdout.contains("line1\n"));
    assert!(!stdout.contains("line2\n"));

    // Stop the daemon
    let mut cmd = test_context.get_command();
    cmd.arg("stop").arg("log_daemon");
    cmd.assert().success();
}

#[test]
fn test_logs_error_target_not_found() {
    let config_src = r#"
        [command.exec.some_command]
        command = "echo hello"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("logs").arg("nonexistent");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_logs_error_not_a_daemon() {
    let config_src = r#"
        [command.exec.not_daemon]
        command = "echo hello"
        daemon = false
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("logs").arg("not_daemon");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("not a daemon"));
}

#[test]
fn test_logs_error_not_started() {
    let config_src = r#"
        [command.exec.not_started]
        command = "echo hello"
        daemon = true
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("logs").arg("not_started");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("No log file found"));
}
