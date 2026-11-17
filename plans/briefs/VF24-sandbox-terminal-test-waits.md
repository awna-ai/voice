# VF24: the sandbox's terminal test waits long enough on a busy computer

Card `jFdgqJCo`. Spec: lane V's spec file § VF24. Test only. Codex was at its usage limit (until
2026-11-18), so V1 implemented this brief directly. Same family as VF4, VF16 and VF20.

## Why

`builder/agent/tests/sandbox_profile.rs` `refuses_the_persons_terminals_and_system_logs` runs a
sandboxed `/bin/cat` (or `/bin/ls` for a folder) on `/dev/console`, a few system log paths and every
`/dev/ttys*`, and expects each to exit with code 1, the sandbox's refusal. It gives each one a fixed
2 s, then kills it and panics `sandbox read timed out`. On a loaded machine `sandbox-exec` can take
longer than that just to start. That is how VF22's full gate stopped (2026-11-17 03:50Z, load1
about 20, on `/dev/ttyse`). Run alone right after, the test passed 3 times in 3.

## What to change

Only that test, only its deadline: 2 s becomes 30 s, with a comment saying why. A refused read
exits at once; a read the sandbox allowed on a terminal blocks, so it still fails at the deadline.
Everything else stays: the paths, the exit-code-1 assertion, the kill and wait, the count line.

## Proof

- Red (stand-in for a slow start): the old loop with a 20 ms deadline fails the way the land did
  (`sandbox read timed out`).
- Green: the new test passes and checks every readable path.
- Mutation: `(regex #"^/dev/ttys")` added to the profile's `allow file-read*` list must turn the new
  test red (the read blocks, so the deadline fires).

## Gate

A test-only slice (V-Q19): `cargo fmt --check`, the touched test with the env and features the full
gate gives it, and `cargo clippy --all-targets --locked -- -D warnings`, landed with
`direct-to-main.sh land --check`. Verifier: not required by the spec.
