use assert_cmd::prelude::*;
use predicates::prelude::*;

mod common;

#[test]
fn test_suggests_run_when_target_used_as_command() {
    let config_src = r#"
        [command.exec.test]
        command = "echo test"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("test");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand 'test'"))
        .stderr(predicate::str::contains(
            "Hint: Did you mean 'pls run test'?",
        ))
        .stderr(predicate::str::contains(
            "Target 'test' exists but must be run with the 'run' command.",
        ));
}

#[test]
fn test_no_hint_for_invalid_command() {
    let config_src = r#"
        [command.exec.test]
        command = "echo test"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("invalidcommand");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"))
        .stderr(predicate::str::contains("Hint:").not());
}
