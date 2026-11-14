# VF5d brief: the two LOW findings from VF5b's security review

The security review of VF5b (`1c13b54`) found no HIGH or MEDIUM findings and two LOW ones. Fix both
after VF5c is in the working tree (VF5c adds `claude::forget_key`, used below). Same rules as VF5b's
brief: public repo, made-up keys only (`sk-ant-api03-` followed by `a` characters), red first,
**commit nothing**, and never touch `.ai/`.

Read first: `plans/briefs/VF5b-console-key-app.md`, `plans/briefs/VF5c-review-fixups.md`,
`builder/api/src/assistants.rs` (`claude_check_in`, `claude_save_key_in`, `unavailable_status`),
`builder/agent/src/console_key.rs`, `builder/ui/src/screens/ConnectClaude.vue` and its test.
Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/frontend/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`.

## 1. Fixes (decisions made; do not change them)

1. **A key typed but not saved never stays in the screen's state.** In `ConnectClaude.vue`, set
   `key.value = ''` at the start of `check()` and of `signIn()`, and in an `onUnmounted` hook (the
   screen already imports from `vue`; add `onUnmounted` to that import if it isn't there). Red
   first (vitest): type a key, click *Check again* whose reply still offers the key field, and the
   field is empty; the same after the subscription button in a testers' status
   (`canSignInWithSubscription: true`).
2. **An unreadable stored key doesn't lock the person out of Connect Claude** (a Mac only).
   - `claude_check_in`: when `claude::find_in` fails with exactly `text::KEY_UNREADABLE` (compare the
     error's message, as `claude_message` does), return
     `dto::ClaudeStatus { subscription: false, ready: false, message: text::KEY_UNREADABLE.into(), note: None, can_sign_in: true, can_sign_in_with_subscription: claude::SUBSCRIPTION_OFFERED, console_takes_key: true }`
     instead of `unavailable_status`, so the key field shows. Every other error stays as it is.
   - `claude_save_key_in`: when `find_in` fails with exactly `text::KEY_UNREADABLE`, call
     `claude::forget_key(home)`; if that fails, return the same status as above (still retryable);
     if it succeeds, call `find_in` once more and carry on as today with its result. The key the
     person just pasted is never dropped before `save_key` runs except on these early returns.
   - Off a Mac nothing changes (`find_in` never loads a key there).
   - Red first (API tests, the stub `claude` pattern of `builder/api/tests/claude_sign_in.rs`; with
     the agent's `test-stubs` the stored key is a file in the test's home, so write a bad-shaped value
     such as `not-a-key` into that file the way `builder/agent/tests/console_key.rs`
     `invalid_and_linked_stores_are_unreadable` does): a check with a bad-shaped stored value is not
     ready, says `KEY_UNREADABLE`, and has `can_sign_in` and `console_takes_key` true; saving a valid
     made-up key then replaces it and reads ready; a later check is ready.

## 2. Tests and checks

From the repo root, two test threads (the computer is busy):
`cargo test --locked -p voice-agent -p voice-api -p voice-cli -- --test-threads=2`, the same with
`--features claude-subscription`, `cargo fmt --all`,
`cargo clippy --all-targets --locked -- -D warnings`, and
`cargo clippy --all-targets --locked -p voice-agent -p voice-api -p voice-cli -p voice-app --features claude-subscription -- -D warnings`;
in `builder/ui`: `pnpm test` and `pnpm typecheck` (dependencies are installed; if the registry is
unreachable, say so and stop there). Your sandbox can't bind localhost: report the tests that fail
for that, and run everything else. Report the files changed and the exact commands with results.
