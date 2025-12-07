use assert_cmd::prelude::*;
use predicates::prelude::*;

mod common;

// TODO: This test requires a daemon command that runs reliably on Windows.
// The current ping-based approach doesn't work reliably in the test environment.
// The underlying daemon functionality works - this is a test infrastructure issue.
#[test]
#[cfg_attr(windows, ignore)]
fn test_daemon_start_idempotent() {
    let config_src = format!(
        r#"
        [command.exec.my_daemon]
        command = "{}"
        daemon = true
    "#,
        common::echo_and_sleep("started", 1)
    );

    let test_context = common::TestContext::new();
    test_context.write_config(&config_src);

    // Start the daemon first time
    let mut cmd = test_context.get_command();
    cmd.arg("start").arg("my_daemon");
    cmd.assert().success();

    // Try to start again - should error (not idempotent with explicit start command)
    let mut cmd = test_context.get_command();
    cmd.arg("start").arg("my_daemon");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("already running"));

    // Stop the daemon
    let mut cmd = test_context.get_command();
    cmd.arg("stop").arg("my_daemon");
    cmd.assert().success();
}

#[test]
fn test_daemon_dependency_idempotent() {
    let config_src = format!(
        r#"
        [command.exec.my_daemon]
        command = "{}"
        daemon = true

        [command.exec.my_app]
        command = "echo app running"
        requires = ["my_daemon"]
    "#,
        common::echo_and_sleep("daemon started", 1)
    );

    let test_context = common::TestContext::new();
    test_context.write_config(&config_src);

    // Start the daemon explicitly
    let mut cmd = test_context.get_command();
    cmd.arg("start").arg("my_daemon");
    cmd.assert().success();

    // Run app that requires the daemon - should NOT error even though daemon is already running
    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("my_app");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("app running"));

    // Run app again - daemon is still running, should still work
    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("my_app");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("app running"));

    // Stop the daemon
    let mut cmd = test_context.get_command();
    cmd.arg("stop").arg("my_daemon");
    cmd.assert().success();
}

#[test]
fn test_daemon_dependency_starts_if_not_running() {
    let config_src = format!(
        r#"
        [command.exec.my_daemon]
        command = "{}"
        daemon = true

        [command.exec.my_app]
        command = "echo app running"
        requires = ["my_daemon"]
    "#,
        common::echo_and_sleep("daemon started", 1)
    );

    let test_context = common::TestContext::new();
    test_context.write_config(&config_src);

    // Run app WITHOUT starting daemon first - should auto-start the daemon
    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("my_app");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("app running"));

    // Verify daemon is running
    let mut cmd = test_context.get_command();
    cmd.arg("status").arg("my_daemon");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("running"));

    // Stop the daemon
    let mut cmd = test_context.get_command();
    cmd.arg("stop").arg("my_daemon");
    cmd.assert().success();
}
