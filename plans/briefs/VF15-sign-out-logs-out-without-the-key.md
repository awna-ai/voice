# VF15 brief: sign-out still logs out when the Console key can't be forgotten or read

On a Mac, Voice keeps a pasted Console key in its own Keychain item (VF5). Both sign-outs,
`builder/cli/src/main.rs` `claude_sign_out_in` and `builder/api/src/assistants.rs`
`claude_sign_out_in`, first call `claude::forget_key` and then `claude::find_in` to run Claude
Code's `auth logout`. `find_in` ends with `console_key::load(home)?` (in `find_in_impl`,
`builder/agent/src/claude.rs`), so when the Keychain can neither delete nor read the item (it is
locked, say), `find_in` fails with `KEY_UNREADABLE`, the logout never runs, and the person gets
`KEY_NOT_FORGOTTEN` while Claude Code's own login file stays. Make sign-out find Claude Code without
reading the key, so the logout always runs; the key error still wins the reply.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `CONTRIBUTING.md`, `builder/agent/src/claude.rs`
(`forget_key`, `find_in`, `find_in_with_tmutil`, `find_in_impl`, `Claude::sign_out`, `logout`),
`builder/agent/src/console_key.rs` (the `test-stubs` store is the file
`<home>/.voice-test-claude-console-key`; when that path is a directory, `forget` gives
`KEY_NOT_FORGOTTEN` and `load` gives `KEY_UNREADABLE`, which is how a test stands in for a locked
Keychain), both `claude_sign_out_in`s, and the tests named below.
Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`.

This is a public repo: invented values only, no real person, no fixed paths. **Commit nothing;
leave every change in the working tree.**

## 1. Decisions already made (do not change them)

1. **A new finder in `builder/agent/src/claude.rs`:**
   ```rust
   /// `find_in` without reading Voice's Console key: sign-out runs Claude Code's logout even when
   /// the key can't be read (VF15).
   pub fn find_for_sign_out_in(home: &Path, path: Option<&OsStr>, fallbacks: &[PathBuf]) -> Result<Claude>
   ```
   Give `find_in_impl` one more parameter, `read_key: bool`, placed after `fallbacks`. Its
   `console_key::load(home)?` runs only when `read_key` is true (and on macOS, as today). `find_in`
   and `find_in_with_tmutil` pass `true` (their behaviour is unchanged); `find_for_sign_out_in`
   passes `false` and uses `/usr/bin/tmutil` on macOS exactly as `find_in` does. Program lookup and
   `ensure_profile` stay first and unchanged, so a profile failure still fails sign-out.
2. **Both `claude_sign_out_in`s call `find_for_sign_out_in`** instead of `find_in`, and nothing
   else in them changes: `forget_key` still runs first, the `NOT_INSTALLED` arm is unchanged, and
   `forgotten.and(logout)` still makes the key error win. No other caller changes: every call
   that passes the key to Claude Code keeps `find_in`.
3. **`Claude::sign_out` and `logout` stay as they are.** `logout` already launches without the key
   (`launch.command`, not `self.command`).
4. **No new text.** The replies are the existing `text::KEY_NOT_FORGOTTEN`, `SIGN_OUT_SUCCESS`
   and so on.

## 2. Tests, red first

Write each test, run it and watch it fail on its assertion before changing `src/`.

1. **CLI**, `builder/cli/tests/agent.rs`, `#[cfg(target_os = "macos")]`, next to
   `sign_out_reports_profile_failure_after_forgetting_the_key`:
   `sign_out_still_logs_out_when_the_key_cannot_be_forgotten_or_read`. `Fixture::new()`, a stub
   like `claude_sign_out_prints_success_after_claude_logs_out`'s (prints the version, `touch`es
   `$here/logged-out` on `auth logout`), and `fs::create_dir(f.home.join(".voice-test-claude-console-key"))`.
   Run `claude sign-out`. Assert: not success; stderr is exactly
   `format!("voice: {}\n", voice_agent::text::KEY_NOT_FORGOTTEN)`; stdout doesn't contain
   `SIGN_OUT_SUCCESS`; `f.bin.join("logged-out")` exists; the store directory still exists.
2. **API**, `builder/api/tests/claude_sign_in.rs`, `#[cfg(target_os = "macos")]`, next to its
   `sign_out_reports_profile_failure_after_forgetting_the_key`:
   `sign_out_still_logs_out_when_the_key_cannot_be_forgotten_or_read`. `Stub::new("exit 1")`
   (its script records each run, read with `stub.runs()`), `fs::create_dir` the store, call
   `stub.sign_out(&ClaudeSignIns::new())`. Assert: the error's `message` is
   `text::KEY_NOT_FORGOTTEN`; `stub.runs()` contains `auth logout`; the store directory still
   exists.
3. **CLI, test only:** in `console_sign_in_bad_paste_preserves_an_unreadable_stored_key`, make the
   stub record that it ran (for example `touch "${0%/*}/ran"; exit 1`) and assert that
   `f.bin.join("ran")` does not exist after the bad paste. This one is expected to pass already
   (VF5h); confirm it does, and that it fails if you temporarily make the stub record its run before
   the paste is read (do not keep that temporary change).

Every existing test must still pass, especially `sign_out_forgets_malformed_keys_before_finding_claude`,
`sign_out_reports_profile_failure_after_forgetting_the_key` (both crates),
`sign_out_forgets_valid_or_malformed_keys_without_claude_installed` and
`not_installed_sign_out_refuses_a_remaining_login_file`.

## 3. Check before you stop

From the repo root, run each and fix anything red:

- `cargo fmt --check`
- `cargo test --locked -p voice-agent -p voice-api -p voice-cli`
- `cargo test --locked -p voice-agent -p voice-api -p voice-cli --features claude-subscription`
- `cargo clippy --all-targets --locked -- -D warnings`

If a test that needs the macOS sandbox or a socket can't run inside your own sandbox, say exactly
which ones and why; don't change them.

Report: the files changed, each test's red output (one line each), the green run, and anything you
couldn't run.
