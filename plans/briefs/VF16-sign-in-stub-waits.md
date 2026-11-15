# VF16: the sign-in tests wait longer for a stub to start

Card `808a4rGb`. Spec: lane V's spec file § VF16. Test only.

## Why

`builder/api/tests/claude_sign_in.rs` waits at most 3 s for a shell stub to start before it asserts that
the stub started. On a loaded Mac (load average 10 to 27) the stub sometimes takes longer than 3 s to
start, and the test fails with `login did not start` (or `logout did not start`, `status did not start`)
on a change that never touched sign-in. Each wait is a poll that returns as soon as the file appears, so
a longer ceiling makes a passing run no slower.

## What to change

In `builder/api/tests/claude_sign_in.rs`, and nowhere else:

1. `Stub::wait_for_logout`: the ceiling `Duration::from_secs(3)` becomes `STUB_START`.
2. `Stub::wait_for_login`: the same.
3. The `status-started` poll in the test that waits for `status` to start (the `while
   !stub.0.join("status-started").exists() && begun.elapsed() < Duration::from_secs(3)` loop): the same.
4. Add one constant near the top of the file, after the `use` lines:

   ```rust
   /// How long a test waits for a stub it has started to show it is running. Only a ceiling: each poll
   /// returns as soon as the stub's file appears, so a passing run is no slower on a loaded machine.
   const STUB_START: Duration = Duration::from_secs(30);
   ```

## What not to change

- Any other `Duration`: the `< Duration::from_secs(1)` checks prove something returned quickly or did
  *not* happen, and `claude_sign_in_shutdown(Duration::from_secs(5))` is the code under test.
- Any assertion, test name, stub script or non-test file.

## Checks (all must pass)

```sh
cargo fmt --check
VOICE_REQUIRE_PARITY=1 VOICE_REQUIRE_SANDBOX=1 cargo test --locked -p voice-api --test claude_sign_in -- --test-threads=2
VOICE_REQUIRE_PARITY=1 VOICE_REQUIRE_SANDBOX=1 cargo test --locked -p voice-api --features claude-subscription --test claude_sign_in -- --test-threads=2
cargo clippy --all-targets --locked -- -D warnings
```

Testing: `~/.codex/skills/tdd-skill/SKILL.md`. This is a test-harness change with no new behaviour, so
there is no red test to write; the proof is the checks above and the diff touching only the three
ceilings and the constant. Do not commit; leave the change in the working tree.
