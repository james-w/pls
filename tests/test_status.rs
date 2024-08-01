use assert_cmd::prelude::*;
use predicates::prelude::*;

mod common;

#[test]
fn test_status_not_started() {
    let config_src = r#"
        [command.exec.do_stuff]
        command = "bash -c '$i=0; while $i<10; do i+=1; date; sleep 1; done'" 
        daemon = true
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("status").arg("do_stuff");

    cmd.assert().success().stdout(predicate::str::contains(
        "[command.exec.do_stuff] Not running",
    ));
}
