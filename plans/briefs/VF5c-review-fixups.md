# VF5c brief: the security review's fix-ups to VF5a and VF5b

The security review of VF5a (`1bc5a21..ebb4fab`) found no HIGH findings. It found two MEDIUM ones and
some LOW ones; fix those listed here after VF5b is in the working tree. Same rules as VF5a's brief:
public repo, made-up keys only, red first, commit nothing.

## 1. Fixes

1. **M1: sign-out must stop runs already going from using the key.** `Claude::command`
   (`builder/agent/src/claude.rs`) passes the key held in memory since `find_in`, and a build or
   panel keeps one `Claude` for its whole run. So a sign-out from the app or another process leaves
   the key in every later call. Fix: in `Claude::command`, re-read `console_key::load(home)` before each
   run and pass the key only while it is still stored (a load error passes no key, and the run then
   reads as signed out). Red first: save a key, `find_in`, `forget` through another `find_in`'s
   `sign_out`, then a `call` on the first `Claude` runs with `unset`.
2. **M2: sign-out works even when Claude Code can't be found or the stored key is unreadable.** Add
   `pub fn forget_key(home: &Path) -> Result<()>` in `claude.rs` (macOS: `console_key::forget`; off a
   Mac: `Ok(())`). Call it first in the CLI's `claude_sign_out` and the API's `claude_sign_out_in`,
   before and independent of `find`/`find_in`; then run `auth logout` only when `find` succeeds, and
   report success when the key is forgotten and Claude Code isn't installed. Red first: a stored key
   with no `claude` installed is forgotten by sign-out; so is a store holding a bad-shaped value.
3. **L1: a failed save never leaves a key behind.** In `save_key`, a `console_key::save` error is
   followed by a best-effort `console_key::forget`; on the refusal path, if `forget` fails, return
   `KEY_NOT_FORGOTTEN` instead of `KEY_REFUSED`.
4. **L2: a stale login file and a key never mix.** In `save_key`, also run the key-less logout when
   `<profile>/.credentials.json` exists (any file type), not only when `subscription_refused`.
5. **L5: echo comes back after Ctrl-\, a hang-up or a kill.** In `stop_on_interrupt`, install the same
   handler for SIGTERM, SIGHUP and SIGQUIT, so `read_key` returns through its guard.
6. **A mistyped key reads as signed out at the first call.** Measured by V1 on Claude Code 2.1.311
   (2026-11-14, throwaway `CLAUDE_CONFIG_DIR`, a made-up well-shaped key): `claude auth status` checks
   nothing over the network and reads `"loggedIn": true, "authMethod": "api_key"`, so `save_key` keeps
   any well-shaped key. The first `-p` call then exits 1 with `"is_error": true`,
   `"api_error_status": 401` and `"result": "Invalid API key · Fix external API key"`, which
   `signed_out` (`claude.rs`) doesn't match, so the person sees `text::FAILED`. Fix: `signed_out` also
   matches a result containing `Invalid API key`, so that reply is judged `signed-out` with
   `text::SIGNED_OUT` (on a Mac without the subscription feature it already says to paste the key on
   Connect Claude). Red first: a stub reply with exactly that JSON is judged `signed-out`. Don't add a
   network check to `save_key`.

## 1b. Tests and checks

Red first for each fix, then green, from the repo root (two test threads, the computer is busy):
`cargo test --locked -p voice-agent -p voice-api -p voice-cli -- --test-threads=2`, the same with
`--features claude-subscription`, `cargo fmt --all`, and both clippy runs from VF5b's brief §2.5.
Your sandbox can't bind localhost: report the tests that fail for that, and run everything else.
Report the files changed and the exact commands you ran with their results.

## 2. Records to correct (V1 does these, not Codex)

`.ai/SECURITY.md`'s new *Console API key* row and `.ai/DECISIONS.md`'s VF5 entry (uncommitted in the
VF5 worktree): "every buffer zeroed" covers the CLI's input buffers only (the child's environment and
`security`'s output are copies that aren't zeroed: list it as a known limit); "tools off" applies to
calls only; check "the app's request type prints it as hidden and zeroes it on drop" and "a password
field cleared as the request is sent" against VF5b's code; "a mistyped key shows up as not signed in"
is unmeasured: measure Claude Code's reply to a made-up key on one `-p` call (outside the sandbox,
throwaway `CLAUDE_CONFIG_DIR`) and say what Voice shows; and check whether Claude Code writes anything
about an environment-supplied key into `CLAUDE_CONFIG_DIR` (list the throwaway profile after a run).
