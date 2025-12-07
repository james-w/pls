use assert_cmd::prelude::*;
use std::time::Instant;

mod common;

#[test]
fn test_start() {
    let config_src = format!(
        r#"
        [command.exec.do_stuff]
        command = "{}"
        daemon = true
    "#,
        common::daemon_loop_command()
    );

    let test_context = common::TestContext::new();
    test_context.write_config(&config_src);

    eprintln!("Starting daemon...");
    let start = Instant::now();
    let mut cmd = test_context.get_command();
    cmd.arg("start").arg("do_stuff");
    cmd.assert().success();
    eprintln!("Start took {:?}", start.elapsed());

    eprintln!("Stopping daemon...");
    let start = Instant::now();
    let mut cmd = test_context.get_command();
    cmd.arg("stop").arg("do_stuff");
    cmd.assert().success();
    eprintln!("Stop took {:?}", start.elapsed());

    eprintln!("Test complete, dropping TestContext...");
    let start = Instant::now();
    drop(test_context);
    eprintln!("Drop took {:?}", start.elapsed());
}
