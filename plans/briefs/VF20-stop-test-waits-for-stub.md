# VF20: the stop test waits for its stub to start before it stops it

Card `x6cFnLqh`. Spec: lane V's spec file § VF20. Test only. Codex was at its usage limit (until
2026-11-18, re-probed this run), so V1 implemented this brief directly. Same family as VF16.

## Why

`builder/agent/tests/stop.rs` `stopped_call_kills_its_group_and_counts_the_budget` runs a shell stub
that writes its own pid to `parent`, starts `sleep 30 &`, writes that pid to `pid`, and waits. A
second thread sets the stop flag after a fixed 200 ms; the test then reads both files and checks both
processes are gone. Before its body, every stub runs the recording prelude in `tests/common/mod.rs`
(about ten commands: `ls`, `grep`, `pwd`, `env`, `sed`, `cat`). On a loaded machine that takes longer
than 200 ms, so the stop kills the stub before it writes either file and the test panics with
`NotFound` at `common/mod.rs:103`. That is how VF19's land stopped (2026-11-15 19:19Z, load1 ~18).
The call's 2 s timeout would hit a slow start the same way.

## What to change

Only `builder/agent/tests/stop.rs`, only that test:

1. The call's timeout goes from 2 s to 30 s, so a slow start never meets it.
2. The stop thread polls every 20 ms until `pid` reads as a number (at most 30 s), then sets the stop
   flag and returns the moment it did.
3. The speed check measures from the stop: the call returns within 5 s of it (it was "within 5 s of
   the call's start").
4. Everything else stays: the error is the stop's text, both pids are gone within 2 s, and the ledger
   line says `stopped`, cost from the budget, $5, no input tokens.

## Proof

- Red: the old test with `sleep 1` at the start of the stub's body fails the way the land did
  (`common/mod.rs:103`, `NotFound`).
- Green: the new test passes with that `sleep 1`, and without it.
- Mutation: `child.rs`'s stop check turned off (`if false && …`) must turn the new test red (the
  call then runs into its timeout and the error is the timeout's text).

## Gate

A test-only slice (V-Q19): `cargo fmt --check`, the touched test with the env and features the full
gate gives it, and `cargo clippy --all-targets --locked -- -D warnings`, landed with
`direct-to-main.sh land --check`.
