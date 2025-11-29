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

#[test]
fn test_run_alias_r() {
    let config_src = r#"
        [command.exec.echo]
        command = "echo hello"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("r").arg("echo");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("hello"));
}

#[test]
fn test_run_suggests_similar_targets_on_typo() {
    let config_src = r#"
        [command.exec.build]
        command = "echo build"

        [command.exec.test]
        command = "echo test"

        [command.exec.deploy]
        command = "echo deploy"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("buidl");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Target <buidl> not found"))
        .stderr(predicate::str::contains("Did you mean one of these?"))
        .stderr(predicate::str::contains("build"));
}

#[test]
fn test_run_no_suggestions_when_no_similar_targets() {
    let config_src = r#"
        [command.exec.build]
        command = "echo build"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("completely_different");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains(
            "Target <completely_different> not found",
        ))
        .stderr(predicate::str::contains("Did you mean").not());
}

#[test]
fn test_run_suggests_substring_matches() {
    let config_src = r#"
        [command.exec.build-all]
        command = "echo build-all"

        [command.exec.build-docker]
        command = "echo build-docker"

        [command.exec.test]
        command = "echo test"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("build");

    // "build" doesn't exist, but should suggest build-all and build-docker as substring matches
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Did you mean one of these?"))
        .stderr(predicate::str::contains("build-all"))
        .stderr(predicate::str::contains("build-docker"));
}

#[test]
fn test_run_suggests_by_description() {
    let config_src = r#"
        [command.exec.make]
        command = "make"
        description = "Compile the project"

        [command.exec.test]
        command = "npm test"
        description = "Run tests"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("compile");

    // Should suggest "make" because "compile" appears in its description
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Did you mean one of these?"))
        .stderr(predicate::str::contains("make"));
}
