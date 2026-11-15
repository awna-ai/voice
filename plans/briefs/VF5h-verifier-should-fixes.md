# VF5h brief: the Verifier's should-fixes on VF5

The Verifier passed VF5 (`1bc5a21..1957c27`, no must-fix) with four should-fixes and one change to VF5g's
wording. Fix all five. Same rules as the VF5 briefs: public repo, made-up keys only (`sk-ant-api03-`
followed by `a` characters), red first, **commit nothing**, never touch `.ai/`.

Read first: `plans/briefs/VF5f-review-lows.md`, `plans/briefs/VF5g-review-lows.md`,
`builder/agent/src/claude.rs` (`judge`, `signed_out`, `Claude::sign_out`, `forget_key`),
`builder/agent/src/text.rs`, `builder/api/src/assistants.rs` (`claude_save_key_in`,
`claude_sign_out_in`), `builder/cli/src/main.rs` (`claude_sign_in`, `claude_sign_out_in`). Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md`.

## 1. Fixes (decisions made; do not change them)

1. **Only the external-key rejection asks for a new key.** Claude Code 2.1.311 has two replies that
   start `Invalid API key`: `Invalid API key · Fix external API key` (a key passed in the
   environment, Voice's pasted key) and `Invalid API key · Please run /login` (its own login). VF5f
   sends both to `KEY_INVALID`, which tells a person with no pasted key to paste one. In `judge`,
   give `failed("signed-out", text::KEY_INVALID)` only when the error reply's `result` contains both
   `Invalid API key` and `Fix external API key`. Put `result.contains("Invalid API key")` back into
   `signed_out`, so any other `Invalid API key` reply is `SIGNED_OUT`. Red first: unit tests next to
   VF5f's `invalid_api_key_reply_is_signed_out` that `Invalid API key · Please run /login` and a bare
   `Invalid API key` both give outcome `signed-out` with `SIGNED_OUT`; VF5f's external-key test keeps
   `KEY_INVALID`.
2. **A badly shaped paste starts no child in the app.** In `claude_save_key_in`, right after
   `let _guard = ClaudeSignInGuard(sign_ins);` and before `claude::find_in`, return the same
   `KEY_SHAPE` reply the function's `KEY_UNREADABLE` branch returns today when
   `!voice_agent::console_key::valid(key.expose())`, and delete that branch's now-unneeded check
   (one shape check, before anything runs). If the existing tests expect a bad-shape paste on the
   normal path to come back differently (an `Err` rather than a status), keep what they expect for
   that path and move only the check earlier; say which you did. Red first in
   `builder/api/tests/claude_sign_in.rs`: with a working stub Claude Code and nothing stored, a
   `not-a-key` paste answers `KEY_SHAPE`, stores nothing and `stub.runs()` is empty. VF5f's
   `badly_shaped_paste_preserves_an_unreadable_stored_key` stays green.
3. **`voice claude sign-in` replaces an unreadable stored key, as Connect Claude does.** Today the
   CLI stops at `claude::find` with `KEY_UNREADABLE` and never offers a way out. In the CLI's
   `claude_sign_in`, when `claude::find` fails with `KEY_UNREADABLE` and `claude::takes_key(sign_in)`:
   read the key first (`read_key(stop)?`), return `SIGN_IN_STOPPED` if `stop` is set, return
   `KEY_SHAPE` (as an `Error::Invalid`) without touching the store if the key isn't
   `console_key::valid`, otherwise `claude::forget_key(&home)?`, then call `claude::find` again and
   carry on as today, using the key already read instead of reading a second one (the ready line,
   the `stop` checks and `save_key` exactly as now). Any other `find` error, and `KEY_UNREADABLE` for
   a sign-in that doesn't take a key, is returned as today. Red first in `builder/cli/tests/agent.rs`
   (macOS only, like the other key tests): an unreadable stored value (`old-unreadable-value` in
   `.voice-test-claude-console-key`) plus a good key on stdin prints the ready line and leaves the
   new key stored; the same with `not-a-key` on stdin fails with `KEY_SHAPE` and leaves
   `old-unreadable-value` in place.
4. **Sign-out always tries Claude Code's own logout, and says it worked only when it did.**
   - In `Claude::sign_out`, the API's `claude_sign_out_in` and the CLI's `claude_sign_out_in`, a
     failed `forget_key` no longer returns before the logout: remember the error, run the rest
     exactly as today (find, then `sign_out`'s logout or the not-installed login-file check), and
     then return the forget error if there was one (it wins over any later error, since the key is
     what matters most). `Claude::sign_out` itself becomes: forget, logout, then the forget error
     first, else the logout result.
   - In the API's `claude_sign_out_in`, set `status.note = Some(text::SIGN_OUT_SUCCESS)` only when the
     fresh `claude_check_in` status has `ready == false`; when it reads ready (signed in again by
     another process), return the status with no note.
   - Red first: an agent test in `builder/agent/tests/console_key.rs` where the store can't be removed
     (the sabotage pattern of `failed_save_reports_when_cleanup_also_fails`) and the stub records its
     runs: `sign_out` answers `KEY_NOT_FORGOTTEN` and the stub saw `auth logout`. An API unit or
     integration test that a sign-out whose fresh status reads ready carries no `SIGN_OUT_SUCCESS`
     note (a stub whose `auth status` always answers signed in).
5. **VF5g's sentence is true in every build.** Off a Mac Voice keeps no key, so "Voice forgot your
   key" is untrue there. Change `SIGN_OUT_NOT_INSTALLED_FILE_LEFT` in `text.rs` to exactly:
   `"Claude Code isn't installed, so its sign-in file is still in Voice's folder. Install Claude Code, then sign out again."`
   Tests compare the constant, so none should need to change.

## 2. Tests and checks

From the repo root, two test threads and `CARGO_BUILD_JOBS=2` (the computer is busy):
`cargo test --locked -p voice-agent -p voice-api -p voice-cli -p voice-panel-runner -- --test-threads=2`,
the same with `--features claude-subscription` on `-p voice-agent -p voice-api -p voice-cli`,
`cargo fmt --all`, `cargo clippy --all-targets --locked -- -D warnings`, and
`cargo clippy --all-targets --locked -p voice-agent -p voice-api -p voice-cli -p voice-app --features claude-subscription -- -D warnings`.
Your sandbox can't bind localhost: report the tests that fail for that, and run everything else.
Report the files changed and the exact commands with their results.
