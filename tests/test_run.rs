use assert_cmd::prelude::*;
use predicates::prelude::*;

mod common;

#[test]
fn test_error_when_does_not_exist() {
    let config_src = r#"
        [command.exec.copy]
        command = "cp hello world"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("non_existent");

    cmd.assert().failure();

    cmd.assert().stderr(predicate::str::contains(
        "Target <non_existent> not found in config file <",
    ));
}

#[test]
fn test_error_when_ambiguous() {
    let config_src = r#"
        [artifact.exec.copy]
        command = "cp hello world"

        [artifact.container_image.copy]
        context = "."
        tag = "latest"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("copy");

    cmd.assert().failure();

    cmd.assert().stderr(predicate::str::contains(
        "Target <copy> is ambiguous, possible values are <artifact.container_image.copy, artifact.exec.copy>",
    ));
}
