# PB8.9d1: Add another computer, the engine, the CLI and the app commands

Split from PB8.9d (plan `2026-11-12-pb8-9-one-assistant-many-computers.md`, step 2.2): this slice has no
screens; PB8.9d2 adds them on top of the app commands below. PB8.9c2 gave Voice a socket client for
Radio's link ops (`builder/api/src/radio.rs`), VO16.6a installs Radio on start, VO16.6b1 pinned Radio
`v0.3.0` (protocol 1.11). Radio's contract is `CONTRACT.md` at that tag (in the cargo git checkout of
`awna-ai/radio`, or `git -C ~/projects/radio show v0.3.0:CONTRACT.md`): read its rows for `fleet_init`,
`code_create`, `join`, `link_address_*`, `resign`, `delivery_folder_set` and the join code's format
before writing code.

**The flow.** On the computer that builds an assistant (Teo in the plan), the person sets this computer's
VPN address once, then asks for a join code. On a member computer (one that opened her from GitHub,
`save.json` `role: member`), the person sets that computer's VPN address, names the computer, pastes the
code, and joins. The join code already carries the building computer's address (Radio puts its link
addresses in the code), so the member never types it.

## Decisions (settled; do not re-decide)

1. **Where it lives.** A new module `builder/api/src/pairing.rs`, tests in `builder/api/src/pairing/tests.rs`.
   It calls only `radio.rs`'s `Radio` methods. Make PB8.9c2's fake Radio (the scratch-socket server in
   `builder/api/src/radio/tests.rs`) reusable by both test modules (a `#[cfg(test)] pub(crate)` helper
   module); don't copy it.
2. **Radio's refusal phrases.** `pairing` must match a few of Radio's fixed phrases (below). Add to
   `Radio` a crate-private way to get a refusal's raw phrase (for example an `ask_phrase` beside `ask`
   that answers `Err(Refused(phrase))` for an `ok: false` answer), so the existing public methods keep
   their behaviour and sentences exactly. Never show a raw phrase except through the existing
   `refusal()` rule.
3. **`resign` joins the link ops.** Add `"resign"` to `LINK_OPS` and a `Radio::resign_section(section:
   &str) -> Result<()>` that sends `resign` with `app: "voice"` and `section` (no `roles`). Use
   `radio_client`'s `Request::Resign` as the type defines it at `v0.3.0`.
4. **This computer's VPN address** (`pairing::link_address(home)` and `pairing::set_link_address(home,
   address)`):
   - `link_address` answers Radio's `port` and `addresses` (`link_address_list`).
   - `set_link_address` parses the text as `std::net::IpAddr` first (trimmed; anything else, a port or a
     name included, is refused with `NOT_AN_ADDRESS` before Radio is asked). If Radio already holds a
     different address of the same family, Voice removes that one, then adds the new one (setting
     replaces; the same address again changes nothing). Answers the list after the change.
5. **Make a join code** (`pairing::invite(home, slug) -> dto::Invite`), in this order, stopping at the first
   refusal:
   1. The assistant exists (its `member.json`); its private state folder is
      `voice_core::member::private_state_dir` (refuse as other commands do when it isn't
      `~/.claude/state/<slug>`).
   2. **Building computer only:** when `voice_save::record::read_record(state)` is `Github { role: Member,
      .. }`, refuse with `voice_save::text::member_computer`'s sentence pattern, worded for this action:
      `Add another computer from <builder>, the computer that builds <Name>.` (or `from the computer
      that builds <Name>` with no builder). No save record, `Local`, `Existing` and a building `Github`
      record may invite.
   3. **One assistant takes this computer's deliveries.** Radio keeps one delivery folder per app, and
      setting another moves Radio to it. Refuse when any other assistant Voice lists on this computer
      (the same list the Assistants screen uses) has a `deliveries` entry in its private state folder:
      `<Other> already takes the history your other computers send to this one. Voice does this for one
      assistant on each computer for now.`
   4. **Radio answers.** `hello`; when it fails with `text::NOT_RUNNING`, run `radio::install(home)` once,
      then `hello` again (the plan's "install at first pairing"; VO16.6a's start-up install usually got
      there first). Any other failure stops with its sentence.
   5. **An address to be found at.** `link_address_list` empty: refuse with `NEEDS_ADDRESS`.
   6. `register` with Voice's member list. Voice has no Radio members yet, so this is `[]`; a comment says
      VO16.6b2 (panels on Radio) owns the list and must keep calling it with every member.
   7. **The fleet.** `fleet_init("voice")`. The phrase `already in a fleet` means this computer is linked
      already: carry on with no recovery file. On success keep `recovery_file` for the answer.
   8. **The delivery folder.** `delivery_folder_set(<private state folder>/deliveries)`. Radio makes it
      (0700) and refuses a path that already exists unless it made it for Voice; on the phrase `the folder
      already exists; name a new one`, refuse with `<path> is already there, and Radio didn't make it.
      Move it somewhere else, then try again.` (the path shown with `~` for the home folder).
   9. `code_create(None, None)` (Radio's 30-minute default).
   Answer `dto::Invite { code, expires_at, port, addresses, recovery_file: Option<String> }`. The DTO's
   `Debug` hides `code` (as `JoinCode` does), and nothing logs it.
6. **Join** (`pairing::join(home, slug, code: &str, name: &str) -> dto::Joined`), stopping at the first
   refusal, and **writing nothing anywhere before Radio accepts the code**:
   1. The assistant exists. **Member computer only:** when the record is not `Github { role: Member, .. }`,
      refuse: `This computer builds <Name>. Make a join code here, then paste it on the other computer.`
   2. `name` passes `voice_save::computer::valid_computer_name`, else `NOT_A_NAME`.
   3. The code's shape, checked locally: one line, trimmed, at most 2,048 bytes, starting `rj1.`; else
      `NOT_A_CODE`. Radio checks the rest.
   4. Radio answers (5.4), has an address (5.5), `register` (5.6).
   5. `join(code)`, mapping Radio's phrases (anything else keeps PB8.9c2's sentence):
      - `not a join code` → `NOT_A_CODE`
      - `the join was refused` → `CODE_REFUSED`
      - `the computer could not be reached`, `the computer did not answer` → `NOT_REACHED`
      - `the join code has no address to dial` → `CODE_NO_ADDRESS`
      - `already in a fleet` → `ALREADY_LINKED`
   6. `resign_section` with `{"computer_name": "<name>"}` (serde_json, one object, that key only). If it
      fails, answer an error whose sentence is `This computer joined, but Voice couldn't save its name: <the
      error's sentence> Try again with: voice radio name "<name>"` (no doubled full stop).
   Answer `dto::Joined { fleet: <fleet name>, computer_name: <name> }`.
7. **Name this computer** (`pairing::name(home, name)`): `valid_computer_name`, Radio answers (5.4), then
   `resign_section` as in 6.6. Used by `voice radio name` and by the app after a join whose naming failed.
8. **Sentences.** New constants in `radio::text` (plain language, no jargon beyond "VPN address" and
   "join code"):
   - `NOT_AN_ADDRESS`: `That isn't a VPN address. Type the numbers only, like 10.0.0.5, with no port.`
   - `NEEDS_ADDRESS`: `Add this computer's VPN address first, so your other computers can find it.`
   - `NOT_A_NAME`: `Give this computer a name of 1 to 64 letters, numbers or spaces, such as Office Mac.`
   - `NOT_A_CODE`: `That isn't a join code. Copy the whole code from the computer that builds your assistant, then try again.`
   - `CODE_REFUSED`: `The other computer turned that code down. It may be used or out of date: make a new one there, then try again.`
   - `NOT_REACHED`: `Voice couldn't reach the other computer. Check that both are on the VPN and Voice is open there, then try again.`
   - `CODE_NO_ADDRESS`: `That code doesn't say where the other computer is. Add its VPN address there, make a new code, then try again.`
   - `ALREADY_LINKED`: `This computer is already linked to your other computers.`
   - The recovery sentence (a function of the path): `Radio saved a recovery key for your computers in <path>. Move it into your password manager, then delete that file.`
9. **The CLI** (`builder/cli/src/radio.rs`, new `RadioCommand` variants):
   - `voice radio address` prints `This computer's VPN address: <a>, <b> (port <port>).` or `This
     computer has no VPN address yet.`; `voice radio address <ip>` sets it and prints the same line.
   - `voice radio invite <slug>` prints, one per line: the address line, `Join code: <code>`, `It works
     once, until <local time HH:MM>.`, and the recovery sentence when there is one. The code goes only
     to standard output.
   - `voice radio join <slug> --name <name>` reads the code from standard input (one line, at most 4 KiB
     read). When standard input is a terminal it first writes `Paste the join code, then press Return:`
     to standard error and turns terminal echo off while reading (libc `tcgetattr`/`tcsetattr`, echo
     restored on every path). Never a code argument. Prints `This computer joined as <name>.`
   - `voice radio name <name>` prints `This computer is now called <name>.`
   - Errors print their sentence and exit 1, as the other `voice radio` commands do. Slugs are checked
     the way `voice open --update` checks them.
10. **App commands** (no screens): `radio_link_address`, `radio_set_link_address`, `radio_invite`,
    `radio_join`, `radio_name`, wired exactly as `open_saved` and `take_newest` are
    (`builder/app/src/commands.rs`, `command_names.rs`, `voice_api` entry points with DTOs in
    `builder/api/src/dto.rs`, `builder/api/src/parity.rs` mapping each to its CLI command, the UI's
    `builder/ui/src/types` and `backend.ts` / `tauri-backend.ts` request and answer types). Each runs off
    the UI thread as the existing long commands do (a join waits up to 120 s for Radio). `radio_join`'s
    request carries the code; its `Debug` hides it.
11. **Records in the repo, same commit series:**
    - `.ai/SECURITY.md`: extend the row *Voice installing Radio and talking to it* (or add the row after
      it) with: the join code is shown to the person (screen, `voice radio invite`'s standard output) and
      read only from standard input or the app command, never an argument, file or log; join writes
      nothing before Radio accepts the code; the computer's name is the one thing Voice puts in its
      section of the computer's signed record, which every computer in the fleet holds (not secret, so
      only the name the person typed); the delivery folder is Radio-made, 0700, inside the assistant's
      private state folder, for one assistant per computer. Tests column names the new tests.
    - `.ai/DECISIONS.md`: a dated entry `2026-11-17: adding another computer (PB8.9d1)` with decisions
      4, 5.3, 5.7 and 6.6 (the fleet's name is `voice`; a computer's name travels in Voice's Radio
      section; one assistant per computer takes deliveries; setting an address replaces the one of the
      same family).

## Tests (red first, each failing on its assertion before the code)

`builder/api/src/pairing/tests.rs` against the fake Radio on a scratch socket and a scratch home with an
invented assistant (`wren`, `Wren`) and hand-written `save.json` records:
- invite: the request lines in order (`hello`, `link_address_list`, `register` with `members: []`,
  `fleet_init` `voice`, `delivery_folder_set` with the state folder's `deliveries`, `code_create`); the
  answer's fields; `already in a fleet` carries on with no recovery file; a member record is refused
  before any request; another assistant's `deliveries` refuses before any request; no address refuses
  after `link_address_list`; the folder-exists phrase gives its sentence; `Debug` of `dto::Invite`
  hides the code.
- join: a building record, a bad name and a bad code shape are refused **with no request sent**; each
  mapped phrase gives its sentence and sends no `resign`; success sends `resign` with exactly
  `{"computer_name": "<name>"}`; a failing `resign` gives the joined-but-not-named sentence; nothing is
  written under the scratch home on any refused join (compare a file listing before and after).
- address: a port, a name and an empty string are refused before any request; a different same-family
  address is removed then the new one added; the same address sends only the add (Radio's no-op).
- `builder/cli/tests/radio.rs`: `voice radio join` with no Radio in a scratch home reads the code from
  stdin (never an argument; `--code` does not exist) and prints a sentence, not the code; `voice radio
  address 10.0.0.5:7341` prints `NOT_AN_ADDRESS` and exits 1.

Mutation check for the reviewer to repeat: dropping the member-role check, the local code-shape check,
or the before-join-writes-nothing ordering must each turn a test red.

## Rules

- Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
  `~/.codex/skills/code-quality/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md`,
  `~/.codex/skills/source-driven-development/SKILL.md`. Read the repo's `AGENTS.md` first.
- Public repo: invented names only, no real person, computer or address beyond documentation ranges
  (`10.0.0.5`, `192.0.2.x`), no fixed paths.
- No new dependency. Stable Rust, edition 2024. Match the surrounding code's density and idiom.
- Never log, print to standard error, or `{:?}` a join code; never put one on a command line.
- Check before you stop: `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`,
  `cargo test --locked -p voice-api -p voice-cli -p voice-app` (2 test threads), and the UI's
  `pnpm --dir builder/ui test` and type check if you touched its types. Commit nothing; leave the changes
  in the worktree and list every file you changed.
