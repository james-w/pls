use assert_cmd::prelude::*;
use predicates::prelude::*;

mod common;

#[test]
fn test_list() {
    let config_src = r#"
        [artifact.exec.copy]
        command = "cp hello world"

        [command.container.hello]
        image = "alpine"
        description = "Hello world"
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("list");

    cmd.assert().success().stdout(predicate::eq(
        "artifact.exec.copy - \ncommand.container.hello - Hello world\n",
    ));
}
