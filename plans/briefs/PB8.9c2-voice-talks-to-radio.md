# PB8.9c2: Voice talks to its Radio

Built Claude-direct under the fallback rule (Codex is at its usage limit until 2026-11-18); this brief
records the decisions it was built from. Split from PB8.9c (plan
`2026-11-12-pb8-9-one-assistant-many-computers.md`, step 2.1); PB8.9c1 put the `radio` binary in
Voice.app, and PB8.9d is the first screen that uses what this slice adds.

Radio `v0.2.0` (commit `a686dc51d175472562d511911a8b7e642b8d4511`) carries `crates/radio-client`: the
socket contract's request and answer types (protocol 1.7) and `request`, which sends one line, reads one
answer and gives up after a deadline (5 s; 150 s for `send`; 120 s for `join`). `crates/radio-panels` is
byte-identical between `v0.1.0` and `v0.2.0` (`git diff --stat v0.1.0 v0.2.0 -- crates/radio-panels` is
empty). `radio-client` pins `serde =1.0.229` and `serde_json =1.0.151`, the versions voice's lock already has.

## Decisions

1. **The dependency.** `builder/api/Cargo.toml` gains
   `radio-client = { git = "https://github.com/awna-ai/radio", tag = "v0.2.0" }`, and both
   `radio-panels` lines (api, panel-runner) move to `tag = "v0.2.0"`, so one Radio commit is in the lock.
   The lock change is only those two packages and the dependents' lists.
2. **One module, `builder/api/src/radio.rs`** (tests in `src/radio/tests.rs`). Voice's app name in
   Radio is `voice` (`radio::APP`).
3. **Finding the copy.** `shipped_radio()` is `voice_core::cli_command::shipped_cli()`'s folder joined
   with `radio`: the copy beside Voice.app's `voice` (PB8.9c1). A missing copy, or one that is a link or
   not a regular file, is refused with one sentence before anything runs.
4. **The install.** `install(radio, handler, app_version)` runs
   `radio install --app voice --app-version <Voice's version> --handler <Voice's voice>` with stdin,
   stdout and stderr closed, and waits at most 60 s (then kills it). Every exit code gets its own fixed
   sentence: 0 installed; 4 installed (Voice brings no older rooms, so the missed rooms don't matter);
   1 failed; 2 Radio refused how it was asked (a bug in Voice); 3 nobody is signed in at the computer's
   screen; anything else, or a signal, stopped before it finished. Radio's own output is never shown.
   Nothing calls it yet: PB8.9d runs it at first pairing, VO16.6 at app start.
5. **The socket client** is `Radio`, one per socket (`Radio::for_home` uses Radio's own
   `socket_path`). It speaks only the link ops: `hello`, `register`, `fleet_init`, `code_create`,
   `join`, `link_address_add|list|remove`, `computer_list`, `send`, `delivery_folder_set`,
   `deliveries`. No generic request is public. Each call is one `radio_client::request`.
   - Transport failures become Voice's own sentences (Radio isn't running; Radio didn't answer).
   - A refusal shows Radio's fixed phrase (`Radio said: <phrase>.`) only when it is one printable line
     of at most 200 characters; anything else gets a generic sentence.
   - An `ok` answer missing a field the op promises is treated as no answer.
   - `hello` refuses a daemon whose major isn't 1, whose minor is below 7, or that doesn't list every
     link op.
   - The join code goes to Radio only through the socket, never a command line, file or log; the
     made code is held in `JoinCode`, whose `Debug` hides it.
6. **`voice radio status`** prints three lines: where Voice's copy is (or that it's missing), whether
   Radio answers (its version and protocol), and the computers this one is linked to (or none yet). It
   exits 0 when Radio answers, else 1. It changes nothing.

## Tests (red first)

`builder/api/src/radio/tests.rs`, all against stand-ins: a fake Radio on a scratch socket in a temporary
folder (one thread answering canned lines and keeping each request line), and a fake `radio` script
recording its arguments and exiting with a chosen code. Covered: each exit code's sentence and the
argument list; a missing, linked or non-file copy refused before running; the deadline; each op's request
line (`"v":1`, the op and its fields, nothing else) and its answer; a refusal's phrase, and an unsafe phrase
replaced; a missing field; not running; `hello`'s three refusals; `JoinCode`'s `Debug` hiding the code;
the status lines with and without Radio. The CLI's `voice radio status` is checked end to end with no
Radio in a scratch home.

## After the reviews (V1, 2026-11-16)

The security review (one medium) and the Verifier (Fable, FAIL: three must-fixes) changed decisions 3 to 5:
`install(home)` takes Voice's `voice` from a place that lasts (`voice_core::cli_command::shipped_lasting_cli`,
the rule turning an assistant on uses) and the `radio` beside it, by absolute path only, with Voice's own version;
the child runs in its own process group, killed as a group after 180 s (Radio's install lock waits 120 s); exit 1
and 2 have sentences that match what Radio means by them; a refusal phrase shows only as printable ASCII; every
status line shows hidden characters as `?`; another major is named as such; a path that isn't plain text is
refused; dropping a `JoinCode` zeroes it.
