mod common;
use common::*;
use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use voice_agent::guard::Guard;
use voice_core::ceilings::Ceilings;
const STOPPED: &str = "Voice stopped Claude Code, as you asked.";
#[test]
fn stopped_call_kills_its_group_and_counts_the_budget() {
    let home = Scratch::new("stop");
    let stub = Stub::new("echo $$ > \"$here/parent\"\nsleep 30 &\necho $! > \"$here/pid\"\nwait");
    let session = session(&home.0);
    let stop = Arc::new(AtomicBool::new(false));
    let claude = stub
        .claude()
        .with_timeout(Duration::from_secs(30))
        .with_stop(stop.clone());
    let (error, stopped, returned) = thread::scope(|scope| {
        // Stop the call only once the stub has written both pids: a loaded machine can take
        // seconds to start it.
        let stopper = scope.spawn(|| {
            let started = Instant::now();
            while !fs::read_to_string(stub.folder.0.join("pid"))
                .is_ok_and(|pid| pid.trim().parse::<i32>().is_ok())
                && started.elapsed() < Duration::from_secs(30)
            {
                thread::sleep(Duration::from_millis(20));
            }
            stop.store(true, Ordering::SeqCst);
            Instant::now()
        });
        let error = Guard::new(vec![&claude], Ceilings::defaults())
            .call(&session, &call("records", &schema()))
            .unwrap_err();
        let returned = Instant::now();
        (error, stopper.join().unwrap(), returned)
    });
    assert_eq!(error.to_string(), STOPPED);
    assert!(returned.duration_since(stopped) < Duration::from_secs(5));
    for name in ["parent", "pid"] {
        let pid: i32 = stub.file(name).trim().parse().unwrap();
        let started = Instant::now();
        while unsafe { libc::kill(pid, 0) } == 0 && started.elapsed() < Duration::from_secs(2) {
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1, "{name} {pid} survived");
    }
    let line = &ledger_lines(&home.0)[0];
    assert_eq!(line["outcome"], "stopped");
    assert_eq!(line["cost_from"], "budget");
    assert_eq!(line["cost_usd"], 5.0);
    assert_eq!(line["input_tokens"], 0);
}
#[test]
fn stopped_before_a_call_never_runs_or_writes_a_ledger_line() {
    let home = Scratch::new("already-stop");
    let stub = Stub::replying(&ok_reply(&serde_json::json!({})), 0);
    let session = session(&home.0);
    let error = Guard::new(
        vec![&stub.claude().with_stop(Arc::new(AtomicBool::new(true)))],
        Ceilings::defaults(),
    )
    .call(&session, &call("records", &schema()))
    .unwrap_err();
    assert_eq!(error.to_string(), STOPPED);
    assert!(!stub.ran());
    assert!(ledger(&home.0).is_none());
}
