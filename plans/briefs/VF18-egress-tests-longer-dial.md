# VF18: the egress tests give the proxy longer to reach its test server

Card `WIWsUKY7`. Spec: lane V's spec file § VF18. Test only.

## Why

The egress proxy's unit tests in `builder/agent/src/egress.rs` build their rules with the test helper
`fn rules(port)`, which gives the proxy only 500 ms (`dial_time`) to connect to its local test server. On
a heavily loaded Mac (load average about 33) that ran out, and a change that never touched the proxy went
red on three tests: `allowed_tunnels_round_trip` and `header_size_limit` got `502 Bad Gateway` instead of
`200 Connection Established`, and `idle_tunnel_closes` took longer than its 1.3 s ceiling for an 800 ms
idle close. A healthy local connect returns at once, so a longer `dial_time` makes a passing run no slower.

## What to change

In `builder/agent/src/egress.rs`, inside `mod tests`, and nowhere else:

1. In `fn rules(port: u16) -> Rules`, `dial_time: Duration::from_millis(500)` becomes
   `dial_time: Duration::from_secs(5)`. Leave `header_time` and `idle` at 800 ms.
2. In `fn idle_tunnel_closes`:
   - the second read timeout, `socket.set_read_timeout(Some(Duration::from_secs(2)))`, becomes
     `Duration::from_secs(10)`;
   - the last line, `assert!(started.elapsed() < Duration::from_millis(1300));`, becomes
     `assert!(started.elapsed() < Duration::from_secs(5));`;
   - add one comment line just above that assertion:
     `// \`eof\` reads a clean end of stream, so the proxy closed the idle tunnel; a read timeout would fail it.`

   The 400 ms first read still proves the tunnel is open before it goes idle, and `eof` (which asserts the
   read returns `Ok(0)`) still proves the close came from the proxy, never from the test's read timeout.

## What not to change

- Any test that sets its own `dial_time` (the `short.dial_time = Duration::from_millis(100)` tests and
  their `< dial_time + Duration::from_secs(1)` checks): they time the dial deadline on purpose.
- Any other `Duration`, assertion, test name, helper, or any non-test code (the real `Rules::claude_code()`
  keeps its 10 s `dial_time`, checked by its own test).
- No new dependency, no new file.

## Checks (all must pass)

```sh
cargo fmt --check
VOICE_REQUIRE_PARITY=1 VOICE_REQUIRE_SANDBOX=1 cargo test --locked -p voice-agent --features claude-subscription --lib egress -- --test-threads=2
cargo clippy --all-targets --locked -- -D warnings
```

Testing: `~/.codex/skills/tdd-skill/SKILL.md`; code standards: `~/.codex/skills/code-quality/SKILL.md`.
This is a test-harness change with no new behaviour, so there is no red test to write; Claude runs the
mutation check afterwards. Do not commit; leave the change in the working tree.

## Fix-up 1: three tests leaned on the old 500 ms

With `rules()` at 5 s, two tests fail every run, because each test client gives up reading after 2 s
(`fn client`): `resolution_is_checked_and_bounded` (its `slow` resolver now runs out at 5 s) and
`resolver_result_is_pinned` (its first address, `127.0.0.2`, doesn't answer on a Mac and takes half the
dial time, now 2.5 s). `resolution_rejects_without_dialling` passes but now waits 5 s on its `slow` case.
In `builder/agent/src/egress.rs`, `mod tests`, and nowhere else:

1. `fn resolution_rejects_without_dialling`: after `short.public = public;` add
   `short.dial_time = Duration::from_millis(500);`.
2. `fn resolution_is_checked_and_bounded`: after `short.public = public;` add the same line.
3. `fn resolver_result_is_pinned`: after `short.resolve = once;` add
   `short.dial_time = Duration::from_millis(1500);` and, just above it, the comment line
   `// 127.0.0.2 never answers on a Mac and takes half of this; the rest must reach 127.0.0.1 inside the client's 2 s read.`

No other change. Run only `cargo fmt --check` (this sandbox can't open sockets, so Claude runs the tests).
Do not commit.
