use assert_cmd::prelude::*;
use assert_fs::prelude::*;
use predicates::prelude::*;

mod common;

/// On Windows, paths from `cd` command may use 8.3 short names (e.g., RUNNER~1 vs runneradmin).
/// This helper compares paths by canonicalizing both sides.
#[cfg(windows)]
fn paths_equal(output: &str, expected: &std::path::Path) -> bool {
    let output_path = std::path::Path::new(output.trim());
    match (output_path.canonicalize(), expected.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

#[cfg(unix)]
fn paths_equal(output: &str, expected: &std::path::Path) -> bool {
    // On macOS, /var is a symlink to /private/var, so we need to canonicalize both paths
    let output_path = std::path::Path::new(output.trim());
    match (output_path.canonicalize(), expected.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => output.trim() == expected.to_str().unwrap(),
    }
}

#[test]
fn test_exec_command() {
    let config_src = r#"
        [command.exec.hello]
        command = "echo hello"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("hello");

    cmd.assert().success().stdout(predicate::eq("hello").trim());
}

#[test]
fn test_with_args() {
    let config_src = r#"
        [command.exec.hello]
        command = "echo {args} hello"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("hello").arg("world");

    cmd.assert()
        .success()
        .stdout(predicate::eq("world hello").trim());
}

#[test]
fn test_extends() {
    let config_src = r#"
        [command.exec.hello]
        command = "echo hello"

        [command.exec.world]
        extends = "hello"
        command = "echo world"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("world");

    cmd.assert().success().stdout(predicate::eq("world").trim());
}

#[test]
fn test_env() {
    let config_src = format!(
        r#"
        [command.exec.env]
        command = "{}"
        env = ["HELLO=world"]
    "#,
        common::env_command()
    );

    let test_context = common::TestContext::new();
    test_context.write_config(&config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("env");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("HELLO=world").trim());
}

#[test]
fn test_dir_option() {
    let config_src = format!(
        r#"
        [command.exec.pwd_in_subdir]
        command = "{}"
        dir = "subdir"
    "#,
        common::pwd_command()
    );

    let test_context = common::TestContext::new();
    test_context.write_config(&config_src);
    test_context
        .workdir
        .child("subdir")
        .create_dir_all()
        .unwrap();

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("pwd_in_subdir");

    let output = cmd.output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected_path = test_context.workdir.path().join("subdir");
    assert!(
        paths_equal(&stdout, &expected_path),
        "Expected path {:?}, got {:?}",
        expected_path,
        stdout.trim()
    );
}

#[test]
fn test_dir_with_variable() {
    let config_src = format!(
        r#"
        [command.exec.pwd_in_var_dir]
        command = "{}"
        dir = "{{test_dir}}"
        variables = {{ test_dir = "subdir" }}
    "#,
        common::pwd_command()
    );

    let test_context = common::TestContext::new();
    test_context.write_config(&config_src);
    test_context
        .workdir
        .child("subdir")
        .create_dir_all()
        .unwrap();

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("pwd_in_var_dir");

    let output = cmd.output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected_path = test_context.workdir.path().join("subdir");
    assert!(
        paths_equal(&stdout, &expected_path),
        "Expected path {:?}, got {:?}",
        expected_path,
        stdout.trim()
    );
}

#[test]
fn test_dir_default_is_cwd() {
    let config_src = format!(
        r#"
        [command.exec.pwd_no_dir]
        command = "{}"
    "#,
        common::pwd_command()
    );

    let test_context = common::TestContext::new();
    test_context.write_config(&config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("pwd_no_dir");

    let output = cmd.output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected_path = test_context.workdir.path();
    assert!(
        paths_equal(&stdout, expected_path),
        "Expected path {:?}, got {:?}",
        expected_path,
        stdout.trim()
    );
}

#[test]
fn test_dir_relative_to_project_root_not_cwd() {
    let config_src = format!(
        r#"
        [command.exec.pwd_in_target_dir]
        command = "{}"
        dir = "target_dir"
    "#,
        common::pwd_command()
    );

    let test_context = common::TestContext::new();
    test_context.write_config(&config_src);

    // Create both a subdirectory to run from and the target directory at the project root
    test_context
        .workdir
        .child("run_from_here")
        .create_dir_all()
        .unwrap();
    test_context
        .workdir
        .child("target_dir")
        .create_dir_all()
        .unwrap();

    // Run pls from the subdirectory, but with -C pointing to project root
    // The dir should resolve to project_root/target_dir, NOT run_from_here/target_dir
    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("pwd_in_target_dir");

    let output = cmd.output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected_path = test_context.workdir.path().join("target_dir");
    assert!(
        paths_equal(&stdout, &expected_path),
        "Expected path {:?}, got {:?}",
        expected_path,
        stdout.trim()
    );
}
