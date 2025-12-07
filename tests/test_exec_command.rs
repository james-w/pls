use assert_cmd::prelude::*;
use assert_fs::prelude::*;
use predicates::prelude::*;

mod common;

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

    let expected_path = test_context
        .workdir
        .path()
        .join("subdir")
        .canonicalize()
        .unwrap();
    // On Windows, canonicalize adds \\?\ prefix, so we need to handle that
    #[cfg(windows)]
    let expected_str = expected_path
        .to_str()
        .unwrap()
        .strip_prefix(r"\\?\")
        .unwrap_or(expected_path.to_str().unwrap());
    #[cfg(unix)]
    let expected_str = expected_path.to_str().unwrap();
    cmd.assert()
        .success()
        .stdout(predicate::eq(expected_str).trim());
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

    let expected_path = test_context
        .workdir
        .path()
        .join("subdir")
        .canonicalize()
        .unwrap();
    // On Windows, canonicalize adds \\?\ prefix, so we need to handle that
    #[cfg(windows)]
    let expected_str = expected_path
        .to_str()
        .unwrap()
        .strip_prefix(r"\\?\")
        .unwrap_or(expected_path.to_str().unwrap());
    #[cfg(unix)]
    let expected_str = expected_path.to_str().unwrap();
    cmd.assert()
        .success()
        .stdout(predicate::eq(expected_str).trim());
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

    let expected_path = test_context.workdir.path().canonicalize().unwrap();
    // On Windows, canonicalize adds \\?\ prefix, so we need to handle that
    #[cfg(windows)]
    let expected_str = expected_path
        .to_str()
        .unwrap()
        .strip_prefix(r"\\?\")
        .unwrap_or(expected_path.to_str().unwrap());
    #[cfg(unix)]
    let expected_str = expected_path.to_str().unwrap();
    cmd.assert()
        .success()
        .stdout(predicate::eq(expected_str).trim());
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

    let expected_path = test_context
        .workdir
        .path()
        .join("target_dir")
        .canonicalize()
        .unwrap();
    // On Windows, canonicalize adds \\?\ prefix, so we need to handle that
    #[cfg(windows)]
    let expected_str = expected_path
        .to_str()
        .unwrap()
        .strip_prefix(r"\\?\")
        .unwrap_or(expected_path.to_str().unwrap());
    #[cfg(unix)]
    let expected_str = expected_path.to_str().unwrap();
    cmd.assert()
        .success()
        .stdout(predicate::eq(expected_str).trim());
}
