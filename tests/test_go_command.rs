use assert_cmd::prelude::*;
use assert_fs::prelude::*;
use predicates::prelude::*;

mod common;

#[test_with::executable(go)]
#[test]
fn test_go_build_command() {
    let config_src = r#"
        [command.go.build]
        subcommand = "build"
        args = "."
    "#;

    let test_context = common::TestContext::new_with_git();
    test_context.write_config(config_src);

    // Create a minimal Go project
    test_context
        .workdir
        .child("go.mod")
        .write_str("module example.com/test\n\ngo 1.20\n")
        .unwrap();
    test_context
        .workdir
        .child("main.go")
        .write_str(
            r#"package main

import "fmt"

func main() {
    fmt.Println("hello")
}
"#,
        )
        .unwrap();

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("build");

    cmd.assert().success();
}

#[test_with::executable(go)]
#[test]
fn test_go_with_output() {
    let config_src = r#"
        [command.go.build-out]
        subcommand = "build"
        output = "./myapp"
        args = "."
    "#;

    let test_context = common::TestContext::new_with_git();
    test_context.write_config(config_src);

    // Create a minimal Go project
    test_context
        .workdir
        .child("go.mod")
        .write_str("module example.com/test\n\ngo 1.20\n")
        .unwrap();
    test_context
        .workdir
        .child("main.go")
        .write_str(
            r#"package main

func main() {}
"#,
        )
        .unwrap();

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("build-out");

    cmd.assert().success();
}

#[test_with::executable(go)]
#[test]
fn test_go_with_tags() {
    let config_src = r#"
        [command.go.build-tagged]
        subcommand = "build"
        tags = ["integration", "test"]
        args = "."
    "#;

    let test_context = common::TestContext::new_with_git();
    test_context.write_config(config_src);

    // Create a minimal Go project
    test_context
        .workdir
        .child("go.mod")
        .write_str("module example.com/test\n\ngo 1.20\n")
        .unwrap();
    test_context
        .workdir
        .child("main.go")
        .write_str(
            r#"package main

func main() {}
"#,
        )
        .unwrap();

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("build-tagged");

    cmd.assert().success();
}

#[test_with::executable(go)]
#[test]
fn test_go_extends() {
    let config_src = r#"
        [command.go.build]
        subcommand = "build"
        args = "."

        [command.go.build-verbose]
        extends = "build"
        verbose = true
    "#;

    let test_context = common::TestContext::new_with_git();
    test_context.write_config(config_src);

    // Create a minimal Go project
    test_context
        .workdir
        .child("go.mod")
        .write_str("module example.com/test\n\ngo 1.20\n")
        .unwrap();
    test_context
        .workdir
        .child("main.go")
        .write_str(
            r#"package main

func main() {}
"#,
        )
        .unwrap();

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("build-verbose");

    cmd.assert().success();
}

#[test_with::executable(go)]
#[test]
fn test_go_test_command() {
    let config_src = r#"
        [command.go.test]
        subcommand = "test"
        args = "./..."
        verbose = true
    "#;

    let test_context = common::TestContext::new_with_git();
    test_context.write_config(config_src);

    // Create a minimal Go project with a test
    test_context
        .workdir
        .child("go.mod")
        .write_str("module example.com/test\n\ngo 1.20\n")
        .unwrap();
    test_context
        .workdir
        .child("main.go")
        .write_str(
            r#"package main

func Add(a, b int) int {
    return a + b
}

func main() {}
"#,
        )
        .unwrap();
    test_context
        .workdir
        .child("main_test.go")
        .write_str(
            r#"package main

import "testing"

func TestAdd(t *testing.T) {
    if Add(2, 3) != 5 {
        t.Error("2 + 3 should equal 5")
    }
}
"#,
        )
        .unwrap();

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("test");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

#[test_with::executable(go)]
#[test]
fn test_go_test_with_run_pattern() {
    let config_src = r#"
        [command.go.test-specific]
        subcommand = "test"
        args = "./..."
        run_pattern = "TestAdd"
    "#;

    let test_context = common::TestContext::new_with_git();
    test_context.write_config(config_src);

    // Create a minimal Go project with tests
    test_context
        .workdir
        .child("go.mod")
        .write_str("module example.com/test\n\ngo 1.20\n")
        .unwrap();
    test_context
        .workdir
        .child("main.go")
        .write_str(
            r#"package main

func Add(a, b int) int { return a + b }
func main() {}
"#,
        )
        .unwrap();
    test_context
        .workdir
        .child("main_test.go")
        .write_str(
            r#"package main

import "testing"

func TestAdd(t *testing.T) {
    if Add(2, 3) != 5 {
        t.Error("failed")
    }
}
"#,
        )
        .unwrap();

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("test-specific");

    cmd.assert().success();
}

#[test_with::executable(go)]
#[test]
fn test_go_mod_tidy() {
    let config_src = r#"
        [command.go.tidy]
        subcommand = "mod"
        mod_operation = "tidy"
    "#;

    let test_context = common::TestContext::new_with_git();
    test_context.write_config(config_src);

    // Create a minimal Go project
    test_context
        .workdir
        .child("go.mod")
        .write_str("module example.com/test\n\ngo 1.20\n")
        .unwrap();
    test_context
        .workdir
        .child("main.go")
        .write_str(
            r#"package main

func main() {}
"#,
        )
        .unwrap();

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("tidy");

    cmd.assert().success();
}

#[test_with::executable(go)]
#[test]
fn test_go_artifact_smart_defaults() {
    let config_src = r#"
        [artifact.go.build]
        subcommand = "build"
        bin = "myapp"
        args = "."
    "#;

    let test_context = common::TestContext::new_with_git();
    test_context.write_config(config_src);

    // Create a minimal Go project
    test_context
        .workdir
        .child("go.mod")
        .write_str("module example.com/test\n\ngo 1.20\n")
        .unwrap();
    test_context
        .workdir
        .child("main.go")
        .write_str(
            r#"package main

func main() {}
"#,
        )
        .unwrap();

    // The artifact should auto-set:
    // - updates_paths to ["./myapp"]
    // - if_files_changed to ["**/*.go", "go.mod", "go.sum"]

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("build");

    cmd.assert().success();
}

#[test_with::executable(go)]
#[test]
fn test_go_artifact_with_output() {
    let config_src = r#"
        [artifact.go.build-bin]
        subcommand = "build"
        output = "./bin/myapp"
        args = "."
    "#;

    let test_context = common::TestContext::new_with_git();
    test_context.write_config(config_src);

    // Create a minimal Go project
    test_context
        .workdir
        .child("go.mod")
        .write_str("module example.com/test\n\ngo 1.20\n")
        .unwrap();
    test_context
        .workdir
        .child("main.go")
        .write_str(
            r#"package main

func main() {}
"#,
        )
        .unwrap();

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("build-bin");

    // Should auto-detect updates_paths as ["./bin/myapp"]
    cmd.assert().success();
}
