# VF47 brief: the sandbox's Keychain test makes its own throwaway Keychain

Card: VF47 (Persona Builder board). Lane V, runner V1. You are Codex; implement this in the worktree you were
started in. Read `AGENTS.md` first. Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`.

## What is wrong (measured by Claude, 2026-11-21)

Voice's code lands now run their gate on Teo, over ssh. In an ssh session the login Keychain is locked, so the
setup step of `the_keychain_stays_closed` in `builder/agent/tests/sandbox_profile.rs` fails before the sandbox is
ever tested:

```
security: SecKeychainItemCreateFromContent (<default>): User interaction is not allowed.   (exit 36)
thread 'the_keychain_stays_closed' panicked at builder/agent/tests/sandbox_profile.rs:844:5:
couldn't create the test Keychain item
```

It is the only failing test in the workspace on Teo. A Keychain file the test makes itself works in that ssh
session: Claude ran, on Mimi and on Teo over ssh, `security create-keychain -p <random> <dir>/t.keychain-db`,
`unlock-keychain -p <same> <file>`, `add-generic-password -a voice-test -s probe -w unused <file>`,
`find-generic-password -s probe <file>` and `delete-keychain <file>`; every one exited 0, and
`security list-keychains -d user` was the same before, during and after.

## The fix (decided; do exactly this)

Only `builder/agent/tests/sandbox_profile.rs` changes. Not the profile (`builder/agent/sandbox/claude-code.sb`),
not `tests/common`, nothing else.

1. Replace `struct KeychainCleanup` (its `impl` and its `Drop`) with `struct TestKeychain`, holding a `Scratch`
   (from `common`, `Scratch::new("keychain")`) and the path of the file `test.keychain-db` inside it.
   - `TestKeychain::new()`: a random 32-hex-digit password read from `/dev/urandom` the way `KeychainCleanup::new`
     reads its random service name today; run `/usr/bin/security create-keychain -p <password> <path>` and then
     `/usr/bin/security unlock-keychain -p <password> <path>`, asserting each succeeds with the messages
     `"couldn't create the test Keychain"` and `"couldn't unlock the test Keychain"`. The password goes only into
     those two argv lists; never print it.
   - `Drop`: run `/usr/bin/security delete-keychain <path>`; when not `std::thread::panicking()`, assert it
     succeeded (`"couldn't delete the test Keychain"`). The `Scratch` removes the folder after that, as it does for
     the other scratch folders.
   - Keep the random service-name helper the test needs: a small `fn random_service() -> String` giving
     `voice-sandbox-test-<32 hex digits>` exactly as `KeychainCleanup::new` builds it today.
2. In `the_keychain_stays_closed`, keep the `available()` check and `Sandbox::new()` first, then:
   - `let keychain = TestKeychain::new();`
   - the readable item: `security add-generic-password -a voice-test -s <readable> -w unused <keychain path>`,
     run outside the sandbox, asserting success with the existing message `"couldn't create the test Keychain item"`;
   - the sandboxed add, as today but with the keychain path as its last argument and a second random service;
   - the sandboxed find, as today but `find-generic-password -s <readable> <keychain path>`;
   - the same two asserts: `"the sandbox wrote a Keychain item"` and `"the sandbox found the test Keychain item"`;
   - then, outside the sandbox, `security find-generic-password -s <readable> <keychain path>` must succeed and
     `find-generic-password -s <the second service> <keychain path>` must fail (assert messages
     `"the test Keychain item went missing"` and `"the sandbox's item is in the test Keychain"`). These replace what
     `KeychainCleanup`'s drop-time `assert_eq!(found, self.1, …)` checked.
3. Keep the comment above the readable item, reworded to say why: a Keychain file of the test's own avoids every
   person's Keychain and works where the login Keychain is locked, as in an ssh session.

## Test it

`sandbox-exec` does not run inside your sandbox, so you cannot run this test: `available()` returns false there,
and with `VOICE_REQUIRE_SANDBOX=1` it panics. Run only `cargo fmt --check` and
`cargo clippy --locked -p voice-agent --all-targets -- -D warnings`, and report both. Claude runs the test, its
mutation check and the gate herself.

## Commit

**Do not commit and do not push**: your sandbox cannot write this worktree's git index. Leave the one changed file
uncommitted and say in your last message that nothing was committed. No real person's name, handle or path
anywhere.
