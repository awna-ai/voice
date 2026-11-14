# VF5f brief: the LOW findings from VF5c and VF5d's security review

The security review of VF5c and VF5d (`2907d9b..9e645d9`) found no HIGH or MEDIUM findings and six
LOW ones. Fix the five below; the sixth (each run's Keychain read has no timeout, the same as the
GitHub token's) is recorded as a known limit by V1. Same rules as the VF5 briefs: public repo,
made-up keys only (`sk-ant-api03-` followed by `a` characters), red first, **commit nothing**, never
touch `.ai/`.

Read first: `plans/briefs/VF5c-review-fixups.md`, `plans/briefs/VF5d-review-lows.md`,
`builder/agent/src/claude.rs` (`save_key`, `judge`, `signed_out`), `builder/agent/src/console_key.rs`,
`builder/agent/src/text.rs`, `builder/api/src/assistants.rs` (`claude_save_key_in`,
`claude_sign_out_in`, `claude_message`), `builder/cli/src/main.rs` (`claude_sign_in`,
`claude_sign_out`). Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`.

## 1. Fixes (decisions made; do not change them)

1. **A badly shaped paste never costs the old key.** In `claude_save_key_in`, before the
   `KEY_UNREADABLE` branch calls `claude::forget_key`, return the `text::KEY_SHAPE` failure the same
   way `save_key` reports it when `!voice_agent::console_key::valid(key)` (make `valid` reachable
   from the API if it isn't: `pub` on the module item is fine). Red first: an unreadable stored value
   plus a pasted `not-a-key` leaves the stored value in place and answers `KEY_SHAPE`.
2. **A failed save that can't clean up says so.** In `Claude::save_key`, on the
   `console_key::save` error path, if the follow-up `console_key::forget(home)` also fails, return
   `text::KEY_NOT_FORGOTTEN` instead of `KEY_NOT_KEPT`. Red first, by the sabotage pattern of
   `failed_save_removes_the_store_and_refused_cleanup_failure_is_reported` in
   `builder/agent/tests/console_key.rs` (make the store unremovable during the first `auth status`).
3. **A not-installed sign-out still checks Claude Code's login file.** In both sign-outs (CLI and
   API), the `NOT_INSTALLED` branch reports success only when
   `fs::symlink_metadata(claude::profile_folder(home).join(".credentials.json"))` fails with
   `NotFound`; if it exists (any file type) or can't be checked, fail with
   `text::SIGN_OUT_FILE_LEFT`. Red first in `builder/cli/tests/agent.rs` and
   `builder/api/tests/claude_sign_in.rs`: no `claude` installed, a `.credentials.json` in the
   profile folder, a stored key: the key is gone and the answer is `SIGN_OUT_FILE_LEFT`, no success
   line. VF5c's not-installed tests (no login file) stay green.
4. **A key Claude Code rejects tells the person how out.** Claude Code's `auth status` reads any
   well-shaped key as signed in, so the key field never shows again, yet `SIGNED_OUT` says to paste
   the key. Add to `text.rs` (one definition, every build):
   `pub const KEY_INVALID: &str = "Claude Code says your Anthropic Console API key doesn't work. Sign out on Connect Claude, or run \`voice claude sign-out\`, then paste a new key.";`
   In `judge`, an error reply whose `result` contains `Invalid API key` gives
   `failed("signed-out", text::KEY_INVALID)`; the other `signed_out` matches keep `text::SIGNED_OUT`
   (take `Invalid API key` back out of `signed_out` and test it first, before that branch). Add
   `KEY_INVALID` to `claude_message`'s allow-list. Update VF5c's
   `invalid_api_key_reply_is_signed_out` to expect `KEY_INVALID` (outcome stays `signed-out`), and add
   a test that a `/login` reply still gives `SIGNED_OUT`.
5. **A kill during `voice claude sign-in` stops the save.** In the CLI's `claude_sign_in`, right after
   `read_key(stop)?` returns, if `stop` is set return `text::SIGN_IN_STOPPED` without saving; and
   after `claude.save_key` succeeds, if `stop` is set, call `claude::forget_key(&home)?` and return
   `SIGN_IN_STOPPED`. Red first in `builder/cli/tests/agent.rs`: with the key on stdin and the stub's
   `auth status` sending the CLI `SIGTERM` (`kill -TERM $PPID` from the stub) before it answers,
   the command fails with `SIGN_IN_STOPPED`, prints no ready line, and no key is stored. If that
   signal timing can't be made reliable, say so and test the two `stop` checks with whichever seam is
   reliable instead; never a sleep-based test.

## 2. Tests and checks

From the repo root, two test threads and `CARGO_BUILD_JOBS=2` (the computer is busy):
`cargo test --locked -p voice-agent -p voice-api -p voice-cli -- --test-threads=2`, the same with
`--features claude-subscription`, `cargo fmt --all`,
`cargo clippy --all-targets --locked -- -D warnings`, and
`cargo clippy --all-targets --locked -p voice-agent -p voice-api -p voice-cli -p voice-app --features claude-subscription -- -D warnings`.
Your sandbox can't bind localhost: report the tests that fail for that, and run everything else.
Report the files changed and the exact commands with their results.
