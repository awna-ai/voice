# VF7d brief: the permission text says how a subscription stops, version 3

Card `XwB1BQNw` (lane V). Decided: the project owner chose one wording for every build, a new
consent version 3, and everyone agrees once more. Do exactly this; change nothing else.

Skills: `~/.codex/skills/tdd-skill/SKILL.md` (red first), `~/.codex/skills/plain-language/SKILL.md`.
Read `AGENTS.md` first: no real people, no fixed paths.

## Why

`builder/core/src/consent.rs` tells the person "Voice stops for the day when it reaches a daily
limit". Since VF7b, a build signed in with a Claude subscription has no daily limit: it stops when
Claude Code says the plan is nearly used up. The permission text must say both.

## The change

1. **`builder/core/src/consent.rs`**, section "What goes to Claude", second line. Replace it with
   exactly this one string (one line, no line break inside it):

   ```
   This uses your own Claude plan's usage, and a build can use a lot of it. Voice stops for the day when it reaches your daily limit or, signed in with a Claude subscription, when Claude Code says your plan is nearly used up. You can stop it at any time.
   ```

2. Same file: `pub const VERSION: u32 = 3;` and set `WORDING_SHA1` to the SHA-1 hex of the new
   `plain_text()` (compute it by running the existing test and reading the failure, or with
   `printf` of the exact text through `shasum`; the test is the judge). Leave every other line,
   heading and the title alone.

3. **Test first.** Before step 1, in `builder/core/src/consent.rs`'s test module add
   `fn wording_names_the_subscription_stop()` asserting that `plain_text()` contains the new line
   above, does not contain `when it reaches a daily limit, and you can stop`, and that `VERSION == 3`.
   Run `cargo test --locked -p voice-core consent` and see it fail on its assertion. Then make
   steps 1 and 2 and see it pass along with `wording_matches_pinned_sha1_and_contains_every_line`.

4. **An older yes is asked again.** In `builder/api/tests/assistants.rs`, rename
   `version_one_consent_is_no_permission_given` to `older_consent_versions_are_no_permission_given`
   and make it loop over versions `1` and `VERSION - 1` (use `voice_core::consent::VERSION`, never a
   bare `2`), writing `{"version":<v>,"agreed_on":"2026-10-25"}` for each and asserting the same
   `Consent { agreed: false, agreed_on: None }`. No production code changes for this: `read_consent`
   already compares with `consent::VERSION`.

5. **`.ai/DECISIONS.md`**: append a new dated section at the end, in the file's style:

   ```
   ## 2026-11-15: the permission text says how a subscription stops, so it is version 3 (VF7d)

   - **One wording for every build:** Voice stops for the day at the person's daily limit or, signed
     in with a Claude subscription, when Claude Code says the plan is nearly used up (VF7b's stop).
   - **The wording changed, so it is version 3:** a yes given to version 1 or 2 is asked again, once.
   ```

## Leave alone

- `builder/ui/src/test-support.ts` `consentText`: an old version-1 fixture the UI tests feed in;
  the app reads the real text from the API. Don't touch it.
- `plans/briefs/PB7.2-*.md` and `.ai/handoffs/*`: dated records.

## Done when

- `cargo fmt --check`, `cargo test --locked -p voice-core`, `cargo test --locked -p voice-api --test assistants`
  and `cargo clippy --all-targets --locked -- -D warnings` pass (use `-j 2` and
  `RUST_TEST_THREADS=2`: the machine is loaded).
- Do not commit. Report the files changed, the red run's failing assertion and the green runs.
