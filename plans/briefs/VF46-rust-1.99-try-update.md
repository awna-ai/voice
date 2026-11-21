# VF46 brief: voice builds clean on Rust 1.99

Card: VF46 (Persona Builder board). Lane V, runner V1. You are Codex; implement this in the worktree you were
started in. Read `AGENTS.md` first. Skills: `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/source-driven-development/SKILL.md`.

## What is wrong (measured by Claude, 2026-11-21)

Voice's code lands run their gate on Teo first, and Teo now has Rust 1.99.1 (this Mac has 1.98.0). On 1.99,
`AtomicUsize::fetch_update` is deprecated: "renamed to `try_update` for consistency". The gate's
`cargo clippy --all-targets --locked -- -D warnings` turns that into an error:

```
error: use of deprecated method `std::sync::atomic::Atomic::<usize>::fetch_update`: renamed to `try_update` for consistency
   --> builder/agent/src/egress.rs:406:10
```

So every code land goes red at its first clippy step, whatever it changes. It is the only `fetch_update` in
`builder/` (`grep -rn fetch_update --include='*.rs' builder`).

`try_update` is stable since Rust 1.95.0 (`#[stable(feature = "atomic_try_update", since = "1.95.0")]` in this
Mac's 1.98 `core/src/sync/atomic.rs`), with the same arguments in the same order (`set_order`, `fetch_order`, a
closure returning `Option`) and the same `Result<usize, usize>`. It compiles with `-D warnings` on 1.98.

## The fix (decided; do exactly this)

In `builder/agent/src/egress.rs`, function `dial`, change `.fetch_update(` to `.try_update(`. Nothing else:
same arguments, same closure, same `.map_err(|_| 502u16)?`. No `#[allow(deprecated)]`, no other file, no
`rust-version` change, no new dependency.

## Check

There is no behaviour to test: the two names are the same method. Run, on this Mac:

- `cargo fmt --check`
- `cargo clippy --locked -p voice-agent --all-targets -- -D warnings`
- `VOICE_REQUIRE_PARITY=1 VOICE_REQUIRE_SANDBOX=1 cargo test --locked -p voice-agent egress -- --test-threads=2`

Report each one's result. The machine is loaded: name any test that times out and passes when run alone. Do not
run `scripts/ci-local.sh` (Claude runs the gate, and probes Teo's 1.99 herself).

## Commit

**Do not commit and do not push**: your sandbox cannot write this worktree's git index. Leave the one changed file
uncommitted and say in your last message that nothing was committed. Claude commits this brief and your change.
No real person's name, handle or path anywhere.
