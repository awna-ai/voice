# VF47 fix-up 1 brief: the Verifier's three should-fixes

Card: VF47 (Persona Builder board). Lane V, runner V1. You are Codex; implement this in the worktree you were
started in. This branch holds VF47 (`plans/briefs/VF47-keychain-test-own-keychain.md`, commit `fca1fcb`). Keep all
of it except what this brief changes. Only `builder/agent/tests/sandbox_profile.rs` changes; no behaviour change.

1. **One random helper.** Replace `fn random_service()` and the `/dev/urandom` read inside `TestKeychain::new`
   with one `fn random_hex() -> String` that reads 16 bytes from `/dev/urandom` and returns
   `format!("{:032x}", u128::from_ne_bytes(random))`. `TestKeychain::new` uses `random_hex()` as the password;
   the test's two services become `format!("voice-sandbox-test-{}", random_hex())`.
2. **The comment on the struct.** Delete the comment now above `let readable = …` in `the_keychain_stays_closed`,
   and put this comment directly above `struct TestKeychain`, word for word:

   ```rust
   // A Keychain file of the test's own avoids every person's Keychain and works where the login
   // Keychain is locked, as in an ssh session. The sandboxed security's default search list (the
   // login Keychain, no path given) is not exercised here; the Library/Keychains and *.keychain-db
   // canaries in denies_every_secret_in_both_letter_cases cover its files.
   ```

Run `cargo fmt --check` and `cargo clippy --locked -p voice-agent --all-targets -- -D warnings` and report both.
You cannot run the sandbox test inside your sandbox; Claude runs it.

**Do not commit and do not push**: leave the one changed file uncommitted and say so in your last message.
