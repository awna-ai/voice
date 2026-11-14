# VF5a brief: the Mac's Console sign-in takes a pasted API key (agent and CLI)

VF5 (V-Q15 answer 2, 2026-11-09): on a Mac, Claude Code's own `auth login --console` creates an API key
and can save it only in the Keychain, which Voice's sandbox keeps closed, so VF3 refused the Console
sign-in on a Mac (`claude::starts(SignIn::Console)` is false there; `sign_in` stops with
`CONSOLE_NOT_AVAILABLE`). VF5 replaces that refusal: the person pastes an Anthropic Console API key,
Voice keeps it in its own Keychain item (the store its GitHub sign-in already uses,
`builder/save/src/keychain.rs`), and passes it to the sandboxed Claude Code as `ANTHROPIC_API_KEY` on
each run. No sandbox change.

This brief (VF5a) is the agent crate and the CLI. A second brief (VF5b) does the API, the app and the
Connect Claude screen on top of it; don't touch `builder/api`, `builder/app` or `builder/ui` here,
except where a test there asserts a sentence this brief rewrites (§2.4).

Facts measured (don't re-measure): with a made-up key in `ANTHROPIC_API_KEY` and an empty
`CLAUDE_CONFIG_DIR`, Claude Code 2.1.311's `auth status --json` reads `loggedIn: true`,
`authMethod: "api_key"` (it doesn't check the key with the server); without it, `loggedIn: false`.
`auth logout` exits 0 when nobody is signed in. Probe 6 (V-Q15) showed the same inside the real sandbox
and a call reaching the API.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md`, `CONTRIBUTING.md`,
`builder/agent/src/claude.rs` (all of it), `builder/agent/src/sandbox.rs` (`Launch::command`, and how
`#[cfg(feature = "test-stubs")]` already swaps behaviour for tests), `builder/agent/src/text.rs`
(the first 30 lines, and `CONSOLE_NOT_AVAILABLE`, `sign_in_cli`, `sign_in_once`),
`builder/save/src/keychain.rs` and `builder/save/src/secret.rs`, `builder/cli/src/main.rs`
(`ClaudeCommand`, `claude_check`, `claude_sign_in`, `claude_sign_out`), the agent tests that use a stub
`claude` (`builder/agent/tests/common/mod.rs`, `builder/agent/tests/find_check.rs`,
`builder/agent/tests/profile.rs`), `builder/agent/tests/text.rs`, and `scripts/ci-local.sh` (its
`test-stubs` guard). Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/plain-language/SKILL.md`.

This is a public repo: invented names only, no real person, no fixed paths, and **never a real API
key anywhere** (tests use made-up keys such as `sk-ant-api03-` followed by `a` characters).
**Commit nothing; leave every change in the working tree.**

## 1. Decisions already made (do not change them)

1. **Where the key lives.** `builder/agent/Cargo.toml` gains `voice-save = { path = "../save" }`
   under `[dependencies]` (no new third-party crate). The agent reuses
   `voice_save::keychain::{Keychain, TokenStore}` and `voice_save::secret::Secret`; it never writes a
   second Keychain implementation. The Keychain item's service is `voice.claude-console`
   (`Keychain::with_service`), its account the existing `ACCOUNT`.
2. **A new module `builder/agent/src/console_key.rs`** (`pub mod console_key;` in `lib.rs`), with a
   module doc saying what it keeps and why (VF5, the sandbox keeps the Keychain closed to Claude Code):
   - `pub const SERVICE: &str = "voice.claude-console";`
   - `pub fn valid(key: &str) -> bool`: starts with `sk-ant-`, does **not** start with `sk-ant-admin`
     (an Admin key can manage the whole organisation and is never needed), 20 to 256 bytes long, and
     every byte an ASCII letter, digit, `-` or `_`. This is a shape check only; Claude Code doesn't
     check the key with the server either.
   - `pub(crate) fn load(home: &Path) -> Result<Option<Secret>>`, `pub(crate) fn save(home: &Path, key: &Secret) -> Result<()>`,
     `pub(crate) fn forget(home: &Path) -> Result<()>`, going through one private
     `fn store(home: &Path) -> impl TokenStore` (or a `Box<dyn TokenStore>`):
     - `#[cfg(not(feature = "test-stubs"))]`: `Keychain::with_service(SERVICE)` (`home` unused).
     - `#[cfg(feature = "test-stubs")]`: a small private `TokenStore` over the file
       `<home>/.voice-test-claude-console-key`, written 0600 (create with `OpenOptions` + `mode(0o600)`,
       write, `sync_all`), read only when it is a regular file (not a link), removed by `forget`
       (missing is fine). This keeps every test away from the real Keychain; `scripts/ci-local.sh`
       already fails when a shipped build of `voice-cli` or `voice-app` has `test-stubs`.
   - Errors from the store map to the agent's own sentences (§1.6), never `voice_save`'s GitHub ones.
   - A loaded value that fails `valid` is an error (`KEY_UNREADABLE`), never used.
3. **Which builds take a key.** In `claude.rs`, add
   `pub fn takes_key(sign_in: SignIn) -> bool { cfg!(target_os = "macos") && sign_in == SignIn::Console }`
   with a doc line. `starts` keeps its meaning (a browser sign-in can start) and doesn't change. Off a
   Mac nothing in this brief changes behaviour: `auth login --console` as today, no key loaded.
4. **The key reaches Claude Code.**
   - `Claude` gains a field `api_key: Option<Secret>` (`Secret`'s `Debug` already hides it; `Claude`
     keeps `#[derive(Debug)]`). `Claude::at` sets `None`.
   - `find_in_impl`, on macOS only, after `ensure_profile`: `console_key::load(home)` and keep the key
     in the returned `Claude`; a load error returns `Err(invalid(text::KEY_UNREADABLE))`.
   - One private helper builds the command for `ask` and `call`: `launch.command(args)`, then, when
     `self.api_key` is set, `.env("ANTHROPIC_API_KEY", key.expose())`. `sign_in_with` and the logout
     run never get the key. Nothing else sets, prints, logs or writes the key.
5. **New and changed `Claude` methods.**
   - `pub fn save_key(self, home: &Path, key: Secret) -> Result<Claude>`:
     1. Off a Mac: return `Err(invalid(text::FAILED))` (nothing calls it there).
     2. `!console_key::valid(key.expose())` → `Err(invalid(text::KEY_SHAPE))`, before any run or store.
     3. `let status = self.status(home)?;` If `status.signed_in`, return `Ok(self)` unchanged (the
        callers check this first and show the ready message). If `status.subscription_refused`, run
        the key-less logout (below) first, so a subscription login file and a key never mix.
     4. `console_key::save(home, &key)` → on error `Err(invalid(text::KEY_NOT_KEPT))`.
     5. `let claude = Claude { api_key: Some(key), ..self };` then `claude.status(home)`: when it
        reads `signed_in && !subscription`, return `Ok(claude)`. Otherwise (or on an error), call
        `console_key::forget(home)` (ignore its error) and return `Err(invalid(text::KEY_REFUSED))`
        (or the status error itself when `status` failed).
   - `sign_out` becomes: on macOS, `console_key::forget(home)` first (error →
     `Err(invalid(text::KEY_NOT_FORGOTTEN))`, and nothing else runs); then the existing `auth logout`
     run and `.credentials.json` check, run **without** the key (a private `logout` helper both
     `sign_out` and `save_key` use).
   - `sign_in_with`: replace the `CONSOLE_NOT_AVAILABLE` refusal with
     `if takes_key(sign_in) { return Err(invalid(text::CONSOLE_TAKES_KEY)); }` (keep `starts` for the
     same check otherwise, so behaviour off a Mac is unchanged). Update the VF3 comment above it to say
     the Mac's Console sign-in is a pasted key now (VF5).
   - Update the module doc at the top of `claude.rs`: it says "Voice never reads, sets or passes a
     token". Make it say Voice never reads Claude Code's own login, and that on a Mac the Console
     sign-in is a key the person pastes, kept in Voice's Keychain item and passed as
     `ANTHROPIC_API_KEY` (VF5).
6. **The sentences** (`builder/agent/src/text.rs`; plain words, written out whole, one literal per
   variant, the `#[cfg]` pattern the file already uses). Off a Mac every existing sentence stays exactly
   as it is. Delete `CONSOLE_NOT_AVAILABLE` (both variants) and update every use (the API allow-list
   in `builder/api/src/assistants.rs` `claude_message` swaps it for `CONSOLE_TAKES_KEY` and gains the
   new key sentences below; that is the only API edit here).
   - New, every build (only used on a Mac):
     - `KEY_HOW`: `Make a key at console.anthropic.com, under API keys. Voice keeps it in this Mac's Keychain and gives it only to Claude Code. What Claude Code does with it is billed to your Console account.`
     - `KEY_PROMPT`: `Paste your Anthropic Console API key, then press Return. It won't show on screen.`
     - `KEY_SHAPE`: `That doesn't look like an Anthropic Console API key. Copy it again from console.anthropic.com: it starts with sk-ant-.`
     - `KEY_NOT_KEPT`: `Voice couldn't keep your key in this Mac's Keychain, so it isn't signed in. If the Keychain is locked, unlock it, then try again.`
     - `KEY_UNREADABLE`: `Voice couldn't read your Anthropic Console API key from this Mac's Keychain. If the Keychain is locked, unlock it, then try again.`
     - `KEY_NOT_FORGOTTEN`: `Voice couldn't remove your Anthropic Console API key from this Mac's Keychain, so it's still signed in. If the Keychain is locked, unlock it, then sign out again.`
     - `KEY_REFUSED`: `Claude Code didn't accept that key, so Voice didn't keep it. Check you copied the whole key from console.anthropic.com, then try again.`
     - `CONSOLE_TAKES_KEY`: ``On a Mac, Voice signs in with an Anthropic Console API key: paste it on Connect Claude, or run `voice claude sign-in`.``
   - Changed, a Mac, public build (`all(target_os = "macos", not(feature = "claude-subscription"))`):
     - `NOT_INSTALLED`: today's shared start, then ` Voice signs it in with an Anthropic Console API key you paste, which is billed for what it uses (API billing).`
     - `SIGNED_OUT`: ``Claude Code isn't signed in for Voice. Paste your Anthropic Console API key on Connect Claude, or run `voice claude sign-in`, then try again.``
   - Changed, a Mac, both builds (`target_os = "macos"`):
     - `SUBSCRIPTION_REFUSED`: ``Claude Code is signed in for Voice with a Claude subscription, which this copy of Voice can't use. Paste an Anthropic Console API key on Connect Claude instead, or run `voice claude sign-in`.``
     - `SUBSCRIPTION_NOT_OFFERED`: `This copy of Voice signs in with an Anthropic Console API key only.`
   - The Mac testers' build keeps its subscription sentences unchanged.
   - `sign_in_once()`, its last branch (a Mac, public build): `, so sign in once with an Anthropic Console API key.`
   - `sign_in_cli()` now always has an answer: return `&'static str` instead of `Option`, with the last
     branch `Run voice claude sign-in to paste your Anthropic Console API key.`; update its callers.
7. **The CLI** (`builder/cli/src/main.rs`):
   - `ClaudeCommand::SignIn`'s doc: `Sign in with an Anthropic Console account (on a Mac, paste its API key).`
   - `claude_sign_in`: after the existing "already signed in" check, when `claude::takes_key(sign_in)`:
     read the key (below), `claude.save_key(&home, key)?`, then print `text::ready(&status.version)`
     and return. The browser path is unchanged otherwise (and `SIGN_IN_WAITING` is printed only for it).
   - The key is **never** an argument or an environment variable. A private `read_key(stop: &AtomicBool) -> Result<Secret>`:
     - When stdin is a terminal (`libc::isatty(0) == 1`): print `KEY_HOW` and then `KEY_PROMPT` to
       stderr; turn echo off with `libc::tcgetattr`/`tcsetattr` on fd 0 (clear `ECHO`), restoring the
       saved settings in a `Drop` guard so every return and an unwinding panic restores them; print a
       newline to stderr after the read.
     - Read with `std::io::Read::read` into a small buffer in a loop until `\n` or end of input,
       keeping at most 4096 bytes (more → `KEY_SHAPE`). On `ErrorKind::Interrupted`, if `stop` is set,
       return `Err(invalid(text::SIGN_IN_STOPPED))`; otherwise read again. (`read_line` retries on
       interrupts by itself, so Ctrl-C would hang there: don't use it.)
     - Trim ASCII white space, put the result in a `Secret`, and zeroize every buffer that held it
       (`zeroize` is already in the workspace; add `zeroize = "=1.9.1"` and `libc = "=0.2.191"` to
       `builder/cli/Cargo.toml` if missing, the versions the workspace already locks).
   - `claude_sign_out` is unchanged (the agent's `sign_out` forgets the key).

## 2. Tests, red first

Write each test, run it, and see it fail on its assertion before the code that makes it pass.

1. `console_key::valid`: accepts `sk-ant-api03-` + 40 `a`s, and one with `-` and `_` inside; rejects
   empty, `sk-ant-` alone (too short), no prefix, `sk-ant-admin01-` + 40 `a`s, a space or a quote
   inside, 257 bytes, a non-ASCII letter.
2. Agent, macOS, with the stub `claude` from `tests/common` (extend the stub so `auth status --json`
   prints `{"loggedIn":true,"authMethod":"api_key"}` only when `ANTHROPIC_API_KEY` equals the made-up
   test key, and records for each run whether the variable was set and matched, writing `match`,
   `mismatch` or `unset`, never the value):
   - `save_key` with a valid key ends signed in, the test store file exists with mode 0600, and a new
     `find_in` loads it: its `check` is ready, and a `call` runs with the key (`match`).
   - A run of `auth logout` and the browser sign-in never get the key (`unset`).
   - `save_key` whose status stays signed out → `KEY_REFUSED`, and the store file is gone.
   - `save_key` with a bad shape → `KEY_SHAPE`, no stub run, no store file.
   - `save_key` when the status reads a refused subscription → `auth logout` runs before the key is
     kept.
   - `sign_out` removes the store file, then runs `auth logout`; a later `find_in` has no key.
   - A store file holding a value that fails `valid` → `find_in` fails with `KEY_UNREADABLE`.
   - `sign_in_with(SignIn::Console)` on a Mac → `CONSOLE_TAKES_KEY` (replaces the
     `CONSOLE_NOT_AVAILABLE` assertion in `tests/find_check.rs`).
3. Texts: in `builder/agent/tests/text.rs`, extend VF14's test
   (`the_claude_code_messages_name_the_sign_in_this_build_can_start`) and VF6's to expect §1.6's
   sentences per build, and assert `sign_in_cli()` per build.
4. CLI, macOS (`builder/cli/tests/`, the existing stub pattern there): `voice claude sign-in` with the
   made-up key on stdin (not a terminal) ends with the ready line, and the key never appears in stdout
   or stderr; a bad key fails with `KEY_SHAPE`; `voice claude sign-out` then leaves no store file.
   Update any test in `builder/agent`, `builder/cli`, `builder/api` or `builder/panel-runner` that
   asserts one of the changed sentences as a literal (`grep -rn "Console account doesn't work on a Mac" builder`).
5. Green, from the repo root, both builds (two test threads, the computer is busy):
   `cargo test --locked -p voice-agent -p voice-api -p voice-cli -p voice-panel-runner -- --test-threads=2`
   and `cargo test --locked -p voice-agent -p voice-api -p voice-cli --features claude-subscription -- --test-threads=2`.
6. `cargo fmt --all`, `cargo clippy --all-targets --locked -- -D warnings`, and
   `cargo clippy --all-targets --locked -p voice-agent -p voice-api -p voice-cli --features claude-subscription -- -D warnings`.

## 3. Done when

- On a Mac, `voice claude sign-in` takes a pasted key, keeps it through `console_key`, and every
  later `ask` and `call` passes it as `ANTHROPIC_API_KEY`; sign-out forgets it first.
- Off a Mac, behaviour is unchanged.
- Every test above fails first and passes after, in both builds; fmt and clippy are clean.
- Report the files changed and the exact commands you ran with their results.
