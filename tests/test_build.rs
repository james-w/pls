use std::thread;
use std::time::Duration;

use assert_cmd::prelude::*;
use assert_fs::prelude::*;
use predicates::prelude::*;

mod common;

#[test]
fn test_build() {
    let config_src = format!(
        r#"
        [artifact.exec.copy]
        command = "{}"
        updates_paths = ["world"]
        if_files_changes = ["hello"]
    "#,
        common::copy_command("hello", "world")
    );

    let test_context = common::TestContext::new();
    test_context.write_config(&config_src);

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
    let config_src = format!(
        r#"
        [artifact.exec.copy]
        command = "{}"
        updates_paths = ["world"]
        if_files_changed = ["hello"]
    "#,
        common::copy_command("hello", "world")
    );

    let test_context = common::TestContext::new();
    test_context.write_config(&config_src);

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
    let config_src = format!(
        r#"
        [artifact.exec.copy]
        command = "{}"
        updates_paths = ["world"]
        if_files_changed = ["hello"]
    "#,
        common::copy_command("hello", "world")
    );

    let test_context = common::TestContext::new();
    test_context.write_config(&config_src);

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

    // Sleep before modifying to ensure we're in a new timestamp window
    // Windows filesystem has ~100ms timestamp resolution, Unix is nanosecond
    #[cfg(windows)]
    thread::sleep(Duration::from_millis(150));
    #[cfg(unix)]
    thread::sleep(Duration::from_nanos(500));

    // Write to the file to update its modification time
    // touch() alone may not update mtime on Windows
    test_context
        .workdir
        .child("hello")
        .write_str("modified")
        .unwrap();

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
    let config_src = format!(
        r#"
        [command.exec.copy]
        command = "{}"
    "#,
        common::copy_command("hello", "world")
    );

    let test_context = common::TestContext::new();
    test_context.write_config(&config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("build").arg("copy");

    cmd.assert().failure();

    cmd.assert().stderr(predicate::str::contains(
        "Target <copy> is not buildable, use the run command instead",
    ));
}

#[test]
fn test_error_when_does_not_exist() {
    let config_src = format!(
        r#"
        [command.exec.copy]
        command = "{}"
    "#,
        common::copy_command("hello", "world")
    );

    let test_context = common::TestContext::new();
    test_context.write_config(&config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("build").arg("non_existent");

    cmd.assert().failure();

    cmd.assert().stderr(predicate::str::contains(
        "Target <non_existent> not found in config file <",
    ));
}

#[test]
fn test_error_when_ambiguous() {
    let config_src = format!(
        r#"
        [artifact.exec.copy]
        command = "{}"

        [artifact.container_image.copy]
        context = "."
        tag = "latest"
    "#,
        common::copy_command("hello", "world")
    );

    let test_context = common::TestContext::new();
    test_context.write_config(&config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("build").arg("copy");

    cmd.assert().failure();

    cmd.assert().stderr(predicate::str::contains(
        "Target <copy> is ambiguous, possible values are <artifact.container_image.copy, artifact.exec.copy>",
    ));
}

#[test]
fn test_artifact_with_dir_option() {
    let config_src = format!(
        r#"
        [artifact.exec.create_file_in_subdir]
        command = "{}"
        dir = "subdir"
        updates_paths = ["subdir/output.txt"]
    "#,
        common::pwd_to_file_command("output.txt")
    );

    let test_context = common::TestContext::new();
    test_context.write_config(&config_src);
    test_context
        .workdir
        .child("subdir")
        .create_dir_all()
        .unwrap();

    let mut cmd = test_context.get_command();
    cmd.arg("build").arg("create_file_in_subdir");

    cmd.assert().success();

    let output_file = test_context.workdir.child("subdir").child("output.txt");
    output_file.assert(predicate::path::exists());

    let expected_path = test_context
        .workdir
        .path()
        .join("subdir")
        .canonicalize()
        .unwrap();
    let contents = std::fs::read_to_string(output_file.path()).unwrap();
    // On Windows, canonicalize adds \\?\ prefix, so we need to handle that
    #[cfg(windows)]
    let expected_str = expected_path
        .to_str()
        .unwrap()
        .strip_prefix(r"\\?\")
        .unwrap_or(expected_path.to_str().unwrap());
    #[cfg(unix)]
    let expected_str = expected_path.to_str().unwrap();
    assert_eq!(contents.trim(), expected_str);
}

#[test]
fn test_artifact_dir_with_variable() {
    let config_src = format!(
        r#"
        [artifact.exec.create_file_var_dir]
        command = "{}"
        dir = "{{build_dir}}"
        variables = {{ build_dir = "subdir" }}
        updates_paths = ["subdir/output.txt"]
    "#,
        common::pwd_to_file_command("output.txt")
    );

    let test_context = common::TestContext::new();
    test_context.write_config(&config_src);
    test_context
        .workdir
        .child("subdir")
        .create_dir_all()
        .unwrap();

    let mut cmd = test_context.get_command();
    cmd.arg("build").arg("create_file_var_dir");

    cmd.assert().success();

    let output_file = test_context.workdir.child("subdir").child("output.txt");
    output_file.assert(predicate::path::exists());

    let expected_path = test_context
        .workdir
        .path()
        .join("subdir")
        .canonicalize()
        .unwrap();
    let contents = std::fs::read_to_string(output_file.path()).unwrap();
    // On Windows, canonicalize adds \\?\ prefix, so we need to handle that
    #[cfg(windows)]
    let expected_str = expected_path
        .to_str()
        .unwrap()
        .strip_prefix(r"\\?\")
        .unwrap_or(expected_path.to_str().unwrap());
    #[cfg(unix)]
    let expected_str = expected_path.to_str().unwrap();
    assert_eq!(contents.trim(), expected_str);
}

#[test]
fn test_build_suggests_similar_targets_on_typo() {
    let config_src = r#"
        [artifact.exec.compile]
        command = "echo compile"
        updates_paths = ["output"]

        [artifact.exec.package]
        command = "echo package"
        updates_paths = ["package.tar"]
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("build").arg("compil");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Target <compil> not found"))
        .stderr(predicate::str::contains("Did you mean one of these?"))
        .stderr(predicate::str::contains("compile"));
}
