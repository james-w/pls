use std::thread;
use std::time::Duration;

use assert_cmd::prelude::*;
use assert_fs::prelude::*;
use predicates::prelude::*;

mod common;

#[test]
fn test_build() {
    let config_src = r#"
        [artifact.exec.copy]
        command = "cp hello world"
        updates_paths = ["world"]
        if_files_changes = ["hello"]
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    test_context.workdir.child("hello").touch().unwrap();

    let starting_timestamp = test_context
        .workdir
        .child("hello")
        .metadata()
        .unwrap()
        .modified()
        .unwrap();

    let mut cmd = test_context.get_command();
    cmd.arg("build").arg("copy");

    cmd.assert().success();

    test_context
        .workdir
        .child("world")
        .assert(predicate::path::exists());

    let ending_timestamp = test_context
        .workdir
        .child("world")
        .metadata()
        .unwrap()
        .modified()
        .unwrap();

    assert!(ending_timestamp >= starting_timestamp);
}

#[test]
fn test_build_doesnt_rebuild() {
    let config_src = r#"
        [artifact.exec.copy]
        command = "cp hello world"
        updates_paths = ["world"]
        if_files_changed = ["hello"]
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    test_context.workdir.child("hello").touch().unwrap();

    let mut cmd = test_context.get_command();
    cmd.arg("build").arg("copy");

    cmd.assert().success();

    eprintln!(
        "{}",
        String::from_utf8(cmd.output().unwrap().stderr).unwrap()
    );

    test_context
        .workdir
        .child("world")
        .assert(predicate::path::exists());

    let middle_timestamp = test_context
        .workdir
        .child("world")
        .metadata()
        .unwrap()
        .modified()
        .unwrap();

    cmd.assert().success();

    eprintln!(
        "{}",
        String::from_utf8(cmd.output().unwrap().stderr).unwrap()
    );

    let ending_timestamp = test_context
        .workdir
        .child("world")
        .metadata()
        .unwrap()
        .modified()
        .unwrap();

    assert_eq!(ending_timestamp, middle_timestamp);
}

#[test]
fn test_build_rebuilds_if_file_changes() {
    let config_src = r#"
        [artifact.exec.copy]
        command = "cp hello world"
        updates_paths = ["world"]
        if_files_changed = ["hello"]
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    test_context.workdir.child("hello").touch().unwrap();

    let mut cmd = test_context.get_command();
    cmd.arg("build").arg("copy");

    cmd.assert().success();

    eprintln!(
        "{}",
        String::from_utf8(cmd.output().unwrap().stderr).unwrap()
    );

    test_context
        .workdir
        .child("world")
        .assert(predicate::path::exists());

    let middle_timestamp = test_context
        .workdir
        .child("world")
        .metadata()
        .unwrap()
        .modified()
        .unwrap();

    cmd.assert().success();

    test_context.workdir.child("hello").touch().unwrap();

    // Tiny sleep to make sure the timestamp changes
    thread::sleep(Duration::from_nanos(500));

    eprintln!(
        "{}",
        String::from_utf8(cmd.output().unwrap().stderr).unwrap()
    );

    let ending_timestamp = test_context
        .workdir
        .child("world")
        .metadata()
        .unwrap()
        .modified()
        .unwrap();

    assert!(ending_timestamp > middle_timestamp);
}

#[test]
fn test_error_when_is_not_an_arfifact() {
    let config_src = r#"
        [command.exec.copy]
        command = "cp hello world"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("build").arg("copy");

    cmd.assert().failure();

    cmd.assert().stderr(predicate::str::contains(
        "Target <copy> is not buildable, use the run command instead",
    ));
}

#[test]
fn test_error_when_does_not_exist() {
    let config_src = r#"
        [command.exec.copy]
        command = "cp hello world"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("build").arg("non_existent");

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
    cmd.arg("build").arg("copy");

    cmd.assert().failure();

    cmd.assert().stderr(predicate::str::contains(
        "Target <copy> is ambiguous, possible values are <artifact.container_image.copy, artifact.exec.copy>",
    ));
}
