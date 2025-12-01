use assert_cmd::prelude::*;
use assert_fs::prelude::*;

mod common;

fn create_minimal_rust_project(test_context: &common::TestContext) {
    test_context
        .workdir
        .child("Cargo.toml")
        .write_str(
            r#"[package]
name = "test"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();
    test_context
        .workdir
        .child("src/main.rs")
        .write_str("fn main() {}\n")
        .unwrap();
}

#[test_with::executable(cargo)]
#[test]
fn test_cargo_command() {
    let config_src = r#"
        [command.cargo.check]
        subcommand = "check"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);
    create_minimal_rust_project(&test_context);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("check");

    cmd.assert().success();
}

#[test_with::executable(cargo)]
#[test]
fn test_cargo_with_features() {
    let config_src = r#"
        [command.cargo.check-all]
        subcommand = "check"
        all_features = true
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);
    create_minimal_rust_project(&test_context);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("check-all");

    cmd.assert().success();
}

#[test_with::executable(cargo)]
#[test]
fn test_cargo_extends() {
    let config_src = r#"
        [command.cargo.check]
        subcommand = "check"

        [command.cargo.check-all]
        extends = "check"
        all_features = true
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);
    create_minimal_rust_project(&test_context);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("check-all");

    cmd.assert().success();
}

#[test_with::executable(cargo)]
#[test]
fn test_cargo_artifact_smart_defaults() {
    let config_src = r#"
        [artifact.cargo.build]
        subcommand = "build"
        bin = "test"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);
    create_minimal_rust_project(&test_context);

    // The artifact should auto-set:
    // - updates_paths to ["target/debug/test"]
    // - if_files_changed to ["src/**/*.rs", "Cargo.toml", "Cargo.lock"]

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("build");

    // This should work even though we didn't explicitly set updates_paths
    cmd.assert().success();
}

#[test_with::executable(cargo)]
#[test]
fn test_cargo_artifact_release() {
    let config_src = r#"
        [artifact.cargo.build-release]
        subcommand = "build"
        bin = "test"
        release = true
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);
    create_minimal_rust_project(&test_context);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("build-release");

    // Binary path should auto-detect as target/release/test (not debug)
    cmd.assert().success();
}

#[test_with::executable(cargo)]
#[test]
fn test_cargo_with_package() {
    let config_src = r#"
        [command.cargo.check-test]
        subcommand = "check"
        package = "test"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);
    create_minimal_rust_project(&test_context);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("check-test");

    cmd.assert().success();
}
