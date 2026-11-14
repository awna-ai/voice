# VF5g brief: the two LOW findings from VF5f's security review

The security review of VF5f (`9b1dd60..45b9ce7`) found no HIGH or MEDIUM findings and two LOW ones.
Fix both. Same rules as the VF5 briefs: public repo, made-up keys only (`sk-ant-api03-` followed by
`a` characters), red first, **commit nothing**, never touch `.ai/`.

Read first: `plans/briefs/VF5f-review-lows.md`, `builder/agent/src/text.rs` (`SIGN_OUT_FILE_LEFT`,
`KEY_INVALID`), `builder/panel-runner/src/turns.rs` (`turn_error`), `builder/api/src/assistants.rs`
(`claude_sign_out_in`, `claude_message`), `builder/cli/src/main.rs` (`claude_sign_out_in` and its
`sign_out_tests`). Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md`.

## 1. Fixes (decisions made; do not change them)

1. **A panel turn shows the rejected-key guidance.** VF5f made `judge` answer `text::KEY_INVALID`
   for an `Invalid API key` reply, but the panel runner's `turn_error` allow-list in
   `builder/panel-runner/src/turns.rs` doesn't name it, so a panel turn now shows `FAILED` where it
   used to show `SIGNED_OUT`. Add `agent_text::KEY_INVALID` to that list, next to
   `agent_text::SIGNED_OUT`. Red first: a unit test in `turns.rs` (a `#[cfg(test)] mod` at the end of
   the file if there is none) that `turn_error(Error::Invalid(KEY_INVALID.into()))` returns
   `KEY_INVALID` and `turn_error(Error::Invalid(SIGNED_OUT.into()))` still returns `SIGNED_OUT`.
2. **A not-installed sign-out that finds a login file says what to do.** `SIGN_OUT_FILE_LEFT` says
   "Claude Code signed out … Try signing out again", which is untrue when Claude Code isn't
   installed, and trying again fails forever. Add to `text.rs` (one definition, every build), next to
   `SIGN_OUT_FILE_LEFT`:
   `pub const SIGN_OUT_NOT_INSTALLED_FILE_LEFT: &str = "Voice forgot your key, but Claude Code isn't installed, so its sign-in file is still in Voice's folder. Install Claude Code, then sign out again.";`
   In both not-installed branches (the API's `claude_sign_out_in` and the CLI's `claude_sign_out_in`),
   return it instead of `SIGN_OUT_FILE_LEFT`. Every other use of `SIGN_OUT_FILE_LEFT` stays. Add the
   new constant to `claude_message`'s allow-list in `assistants.rs`, next to `SIGN_OUT_FILE_LEFT`.
   Update VF5f's two `not_installed_sign_out_refuses_a_remaining_login_file` tests (API
   `builder/api/tests/claude_sign_in.rs`, CLI `builder/cli/src/main.rs` `sign_out_tests`) to expect
   the new constant (red first: change the expectation, watch it fail, then change the code), and add
   `SIGN_OUT_NOT_INSTALLED_FILE_LEFT` to any test in `builder/api` that lists every allowed message
   (search for `SIGN_OUT_FILE_LEFT` in `builder/api/src/assistants.rs` tests and
   `builder/ui/src/wording.test.ts`; if the UI's wording test lists the agent's sentences, add it
   there the same way and run it with `pnpm --dir builder/ui test -- wording`).

## 2. Tests and checks

From the repo root, two test threads and `CARGO_BUILD_JOBS=2` (the computer is busy):
`cargo test --locked -p voice-agent -p voice-api -p voice-cli -p voice-panel-runner -- --test-threads=2`,
the same with `--features claude-subscription` minus `-p voice-panel-runner` if it lacks that
feature, `cargo fmt --all`, `cargo clippy --all-targets --locked -- -D warnings`, and
`cargo clippy --all-targets --locked -p voice-agent -p voice-api -p voice-cli -p voice-app --features claude-subscription -- -D warnings`.
Your sandbox can't bind localhost: report the tests that fail for that, and run everything else.
Report the files changed and the exact commands with their results.
