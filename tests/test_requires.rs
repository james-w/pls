use assert_cmd::prelude::*;
use predicates::prelude::*;

mod common;

#[test]
fn test_requires() {
    let config_src = r#"
        [command.exec.hello]
        command = "echo hello"

        [command.exec.world]
        command = "echo world"
        requires = ["hello"]
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("run").arg("world");

    cmd.assert()
        .success()
        .stdout(predicate::eq("hello\nworld").trim());
}
