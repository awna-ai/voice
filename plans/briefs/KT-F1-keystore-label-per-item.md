# KT-F1 brief: each Secret Service item gets its own label

Card: KT-F1. Lane V, runner V1. You are Codex; implement this in the worktree you were started in, on top of
`HEAD`. Read `AGENTS.md` first. Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`. Leave every change
uncommitted (your sandbox can't write git metadata); Claude commits. Scratch only under a fresh `mktemp -d`,
removed when done. The host is load-throttled: at most 2 heavy processes at a time, `--test-threads=2`.

## The bug

`builder/keystore/src/lib.rs` `System::open` on Linux sets the fixed modifier `("label", "Voice: GitHub sign-in")`,
and `System::entry` passes the same modifiers for every service. So the Claude Console key
(`builder/agent/src/console_key.rs`, service `voice.claude-console`) shows in the person's password manager as
"Voice: GitHub sign-in". The label is display text only: the Secret Service store finds an item by its
attributes (service and user), never by its label, so changing the label never loses a stored item.

## The fix (decided; do exactly this)

1. In `builder/keystore/src/lib.rs`, add a public function:

   ```rust
   /// What the person's password manager shows for an item Voice stores under `service`.
   pub fn label(service: &str) -> String
   ```

   - `"voice.github"` → `"Voice: GitHub sign-in"` (unchanged from today).
   - `"voice.claude-console"` → `"Voice: Claude Console key"`.
   - any other service → `format!("Voice: {service}")`.

   No other table entries. Doc comment says new services should add their line here.
2. `System` stops carrying the label in its fixed `modifiers`. Add a field `labelled: bool`: `true` from the Linux
   `open()`, `false` from the Windows `open()`, the non-Windows/non-Linux `open()` (which errors anyway) and
   `over()`. The Linux `open()`'s fixed `modifiers` become `&[]`; Windows keeps `("persistence", "Local")`
   unchanged.
3. Split the modifier building out of `entry` into a private method
   `fn modifiers(&self, service: &str) -> HashMap<&'static str, String>` (or an equivalent owned shape that
   compiles cleanly with `keyring_core`'s `build(service, user, Option<&HashMap<&str, &str>>)` signature — read
   `keyring_core`'s `CredentialStore::build` in `~/.cargo/registry` to get the exact type, and borrow from the
   owned map when calling it). It returns the fixed modifiers plus, only when `labelled`, `"label"` →
   `label(service)`. `entry` calls it. `over()` must still pass no modifiers at all (the mock store refuses
   them), so an empty map must still reach `build` as it does today (check whether today's code passes
   `Some(&empty)`; keep exactly that behaviour).
4. Change nothing else: no other crate, no service names, no Windows behaviour, no new dependency.

## Tests first (red on `HEAD` on their assertions; a compile error is not a red: add the `label` fn and the
`labelled` field as stubs first if you need the test to compile, then make it fail on its assertion)

In `builder/keystore/src/lib.rs`, a `#[cfg(test)] mod tests` (these must run on a Mac, so they build `System`
directly with the mock store, never `open()`):

- `label` returns the three cases above, plus a second unknown service, to prove the fallback.
- A `System` with `labelled: true` over `keyring_core::mock::Store::new().unwrap()` and empty fixed modifiers:
  `modifiers("voice.github")` holds exactly `label` → `"Voice: GitHub sign-in"`, and
  `modifiers("voice.claude-console")` holds exactly `label` → `"Voice: Claude Console key"` (the bug: two
  services, two labels).
- A `System` with `labelled: false` (as `over()` builds it): `modifiers(...)` is empty for both services.
- A `System` with `labelled: true` and fixed modifiers `[("persistence", "Local")]`: both keys present (the
  label is added beside the fixed ones, never replacing them).

The existing `builder/keystore/tests/system.rs` must stay green unchanged.

Mutations (scratch copy, `cp` back, `cmp`; never `git checkout --`), each must turn a test red:
`label` returns `"Voice: GitHub sign-in"` for every service; `modifiers` ignores `labelled` (always adds the
label); `modifiers` drops the fixed modifiers when it adds the label.

## Run

`cargo fmt --check`; `cargo test --locked -p voice-keystore -- --test-threads=2`;
`cargo test --locked -p voice-save -- --test-threads=2`; for each target `scripts/ci-local.sh`'s `keystore_steps`
names, `cargo check --locked -p voice-keystore --all-targets --target <target>` (only where that target's std is
installed: `rustup target list --installed`; say which ran); `cargo clippy --locked -p voice-keystore --all-targets
-- -D warnings`. End with a short report: red reasons, mutation results, totals.
