use assert_cmd::prelude::*;

mod common;

#[test]
fn test_start() {
    let config_src = r#"
        [command.exec.do_stuff]
        command = "bash -c '$i=0; while $i<10; do i+=1; date; sleep 1; done'" 
        daemon = true
    "#;

    let test_context = common::TestContext::new();
    test_context.write_config(config_src);

    let mut cmd = test_context.get_command();
    cmd.arg("start").arg("do_stuff");

    cmd.assert().success();

    let mut cmd = test_context.get_command();
    cmd.arg("stop").arg("do_stuff");

    cmd.assert().success();
}
