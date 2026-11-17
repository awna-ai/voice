# PB8.9e1 brief: send this computer's history to the build computer (the engine, the CLI, the app commands)

Lane V, runner V1, 2026-11-17. Card `EqEi2E8T` (split from PB8.9e). Codex implements; Claude reviews.

Read first: `AGENTS.md` (no real person anywhere; invented data only; raw history never leaves
`~/.claude/state/<slug>/` except as this slice allows), `.ai/PROJECT.md`, `.ai/DECISIONS.md`, `.ai/SECURITY.md`,
then `plans/briefs/PB8.9d1-add-another-computer-engine.md` and `builder/api/src/pairing.rs` (the slice this one
follows, and the shape to copy: engine functions taking `home` and a `Radio`, fixed sentences, a fake Radio on a
scratch socket in `builder/api/src/pairing/tests.rs`). Radio's contract at the pinned tag is
`~/.cargo/git/checkouts/radio-df939653001766a2/09f499a/CONTRACT.md` (`send`, `deliveries`, `computer_list`).
Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md` (every sentence the person sees).

## Why

The plan (`2026-11-12-pb8-9-one-assistant-many-computers.md`, step 2.3; decision D-1) has each member computer send
its own read Claude Code history to the computer that builds the assistant, over Radio's link only, and the build
computer fold each delivery into `decisions-from-<peer>.jsonl`, merged by record id and never shrunk. This slice is
the engine, the CLI and the app commands; PB8.9e2 adds the screens. The live log is PB8.9f's, not this slice's.

What already exists (don't rebuild it):
- `voice_api::radio::Radio::{computer_list, send, deliveries}` (`builder/api/src/radio.rs`), all in `LINK_OPS`.
- PB8.9d1 made the build computer's delivery folder `<state>/deliveries/` (Radio-made, 0700) at `invite`.
- `history::read_claude_code_in` writes `<state>/decisions-claude-code.jsonl` (filtered, with record `id`s:
  `builder/ingest/src/mine.rs` `record_id`).
- `corpus::discover` reads any `decisions-*.jsonl` other than `decisions-interview*`, `decisions-live.jsonl` and
  `decisions-round-*` as History, so `merge_histories_in` already takes `decisions-from-<peer>.jsonl` in, and
  `corpus::merge` re-runs the personal and secret filters and drops duplicate ids.
- `voice_ingest::private::write_private` / `write_private_at` (`builder/ingest/src/private.rs`) write 0600 files in
  the state folder safely: use them for every file this slice writes.

## Decisions (made at pickup; reversible; add each to `.ai/DECISIONS.md`)

1. **The build computer is the fleet's founder.** Only the build computer invites (`pairing::invite` refuses on a
   member), and its first invite runs `fleet_init`, so from a member computer the build computer is the one entry of
   `computer_list` whose `state` is `accepted` and which has no `invited_by`. None, or more than one: refuse with
   `CANT_FIND_BUILDER` (below). Send to it by its `id`, never its name.
2. **What travels:** this computer's `decisions-claude-code.jsonl` as it is (already filtered here), nothing else.
3. **One pending send.** Before sending, copy it to `<state>/pending-send.jsonl` (0600, replacing any older pending
   copy); remove that file only after Radio answers the send with its size. A failed send keeps it, so a later run
   (PB8.9g's helper) knows a send is owed.
4. **The file's name** is `history-<unix nanoseconds>-<this process's id>.jsonl`, both in decimal (unique, not
   secret: no random source needed; Radio's name rule is `A-Z a-z 0-9 . _ -`, at most 128 bytes, no leading dot).
   The build computer takes only names matching `^history-[0-9]{1,20}-[0-9]{1,10}\.jsonl$`. Radio's refusal `a file
   with that name is already there` maps to `BUSY_BUILDER`.
5. **The peer** is the delivery's authenticated `key` (hex), never its name: the build computer's file is
   `decisions-from-<first 16 hex of key>.jsonl`. A computer's display name for status is the delivery's `computer`
   field, shown only through `{{ }}`-safe text later; store it as given, but never use it in a path.
6. **Merge by id, never shrink.** Read the existing `decisions-from-<peer>.jsonl` (if any) and the delivered file
   with `corpus::read_records` (its shape checks); keep every existing record; add each delivered record whose `id`
   is new; drop a delivered record with no string `id`. Run the member's filters on the delivered records with the
   same function `corpus::merge` uses for each record (find it; don't copy its logic). Sort by (`timestamp`, `id`)
   as `corpus::merge` does. Write 0600 with `write_private`.
7. **Taking deliveries.** On the build computer, `take` reads `<state>/deliveries-taken.json` (`{"after": <last
   Radio delivery id taken>, "computers": {<key hex>: {"name": <string>, "at": <unix seconds>, "records": <count in
   that computer's file>}}}`, 0600; missing means after 0), then pages `deliveries(after, 500)` until empty. For each
   delivery: the file is `<state>/deliveries/<name>`; take it only when the name matches decision 4's pattern, the
   path is a regular file opened without following a link (`O_NOFOLLOW`, then `fstat`), and it is at most 64 MiB;
   fold it (decision 6), delete it, then write the new `after` and that computer's entry. A file that fails a check
   or doesn't parse is deleted, counted as `refused`, and `after` still moves past it (it can't be taken later).
   A delivery whose file is gone is skipped. Taking twice is harmless (merge by id).
8. **Status files**, both 0600 in the state folder: `last-sent.json` on a member (`{"at": <unix seconds>, "to":
   <the build computer's name from computer_list, or its id>}`), written after each successful send; and
   `deliveries-taken.json` on the build computer (decision 7).
9. **Forget** removes all of these: check `use_it.rs` `forget_in`; if it removes the whole state folder, say so in a
   test; if it lists files, add `pending-send.jsonl`, `last-sent.json`, `deliveries-taken.json`, every
   `decisions-from-*.jsonl` and the `deliveries/` folder's files, and test it.
10. **DECISIONS.md** amends the raw-history rule: raw history stays in the assistant's state folder, and travels only
    from a member computer to the build computer over Radio's link (plan D-1), landing in the build computer's state
    folder; nothing else of it ever leaves the computer.

## What to build

New module `builder/api/src/sending.rs` (with `sending/tests.rs`), registered in `lib.rs` like `pairing`:

- `pub fn send_history(home: &Path, slug: &str) -> Result<dto::Sent>` and `fn send_with(home, slug, radio)`:
  refuse a computer that builds the assistant (`format!("This computer builds {name}. Your other computers send
  their history here.")`); refuse when `decisions-claude-code.jsonl` is missing (`format!("Read this computer's
  Claude Code history for {name} first, on the Your history step.")`); `pairing::ready` + `register` (make them
  `pub(crate)`, don't copy them); find the build computer (decision 1); write the pending copy; `radio.send`; on
  success remove the pending copy, write `last-sent.json`, answer `Sent { to: <name or id>, at, bytes }`. Map Radio's
  `send` refusals as `pairing::join_with` maps `join`'s: `the computer could not be reached` / `the computer did not
  answer` → `NOT_REACHED_BUILDER` (below); `that app takes no files on that computer` → `BUILDER_NOT_READY`;
  `that computer is busy; try again` → `BUSY_BUILDER`; anything else through `phrase_error`. A failure leaves the
  pending copy.
- `pub fn take_deliveries(home: &Path, slug: &str) -> Result<dto::Taken>` and `fn take_with(...)`: refuse on a
  member computer (`format!("This computer sends its history to the computer that builds {name}; take deliveries
  there.")`); `ready` (no `register` needed beyond what `ready` gives; check whether `deliveries` needs
  registration, per the contract, and call `register` if so); decision 7; answer `Taken { taken, refused,
  computers: Vec<SentFrom { name, at, records }> }`.
- `pub fn history_status(home: &Path, slug: &str) -> Result<dto::HistoryStatus>`: no Radio call; reads the state
  files: `{ pending: bool, lastSent: Option<Sent>, computers: Vec<SentFrom> }`.
- Sentences in `radio::text` beside PB8.9d1's: `CANT_FIND_BUILDER` = "Voice can't tell which of your computers
  builds this assistant. Link this computer again with a new join code from that computer.";
  `NOT_REACHED_BUILDER` = "Voice couldn't reach the computer that builds this assistant. Check that both are on the
  VPN and Voice is open there. This computer sends it again next time."; `BUILDER_NOT_READY` = "The computer that
  builds this assistant isn't ready to take history yet. Make a join code there once, then try again.";
  `BUSY_BUILDER` = "The computer that builds this assistant is busy. Try again in a moment."
- DTOs in `dto.rs` (ts-rs exported, camelCase, as the neighbours): `Sent { to: String, at: f64, bytes: u64 }`,
  `SentFrom { name: String, at: f64, records: u64 }`, `Taken { taken: u32, refused: u32, computers: Vec<SentFrom> }`,
  `HistoryStatus { pending: bool, last_sent: Option<Sent>, computers: Vec<SentFrom> }`.
- App commands, exactly as PB8.9d1 added its five (`command_names.rs`, `commands.rs`, `main.rs`, capabilities,
  `permissions/autogenerated/*.toml` the build writes, `tests/lockdown.rs`, `parity.rs`, `backend.ts`,
  `app/tauri-backend.ts` and its test, `test-support.ts` fakes): `radio_send_history`, `radio_take_deliveries`,
  `radio_history_status`, each taking `SlugRequest`.
- CLI (`builder/cli/src/radio.rs` and its parser): `voice radio send <slug>` prints `Sent this computer's history to
  <to>.`; `voice radio take <slug>` prints `Took <n> deliveries.` plus one line per computer `<name>: <records>
  records, last <local HH:MM or date>`, and `Left out <refused> files that weren't Voice's history.` when any;
  `voice radio history <slug>` prints the status. Parity rows for all three. Never print a record or a path inside
  a delivery.
- `.ai/DECISIONS.md` (decisions 1 to 10, briefly) and `.ai/SECURITY.md`: the Radio row says raw history now travels
  member → build computer over the link, is at rest only in the state folders (`pending-send.jsonl`,
  `deliveries/<name>` until taken, `decisions-from-*.jsonl`, all 0600 in 0700 folders), that a delivery's contents
  are untrusted even from an accepted computer (name pattern, no link, regular file, 64 MiB cap, shape checks,
  filters re-run, ids), and the tests that show it.

## Tests (red first: each fails on today's code for the right reason)

In `sending/tests.rs`, with the fake Radio from `radio/test_support.rs` and a scratch home holding an invented
assistant (member and builder save records as `pairing/tests.rs` makes them) and invented records:
- send: the request order (`hello`, `register`, `computer_list`, `send`) and the `send` line's `computer` is the
  founder's id, `name` matches the pattern, `file` is the pending copy; success removes the pending copy and writes
  `last-sent.json` 0600; each mapped refusal gives its sentence and keeps the pending copy (0600); no founder and two
  founders refuse with `CANT_FIND_BUILDER` and send nothing; a removed computer with no inviter is not the founder;
  the builder role and a missing history file refuse before any Radio call.
- take: two deliveries from two keys fold into two `decisions-from-*.jsonl` files 0600; a second delivery from the
  same key with fewer records (a shorter read) adds the new ids and keeps every old one; a duplicate id is kept once;
  a record a member filter drops is not written; a bad name, a symlink, a non-regular file, an oversized file and a
  file that doesn't parse are each deleted, counted as refused, and move `after`; `after` is passed on the next call
  and pages until empty; the member role refuses.
- status: reads both files; no files gives `pending: false`, no last send, no computers.
- Forget removes every new file (decision 9).
- CLI tests in `builder/cli/tests/radio.rs` for the three commands' lines; `tauri-backend.test.ts` for the three
  invoke names; the lockdown and parity tests cover the three commands.

Mutation-check each claim (`cp` backup, `cmp`; never `git checkout --`): at least the founder rule, the
never-shrink merge, the symlink refusal, the size cap, the filters re-run, the pending copy kept on failure, and
`after` moving past a refused file.

## Never

- No real person's name, handle or text; invented records only.
- Never log, print or show a record, a delivered file's contents, a raw path inside `deliveries/`, or a key beyond
  what decision 5 puts in a file name.
- No new dependency.
- Never write outside the assistant's state folder; never follow a link in `deliveries/`.

## Gate

`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p voice-api --lib --
sending pairing radio`, `cargo test -p voice-cli --test radio`, `cargo test -p voice-app --test lockdown`,
`pnpm -C builder/ui test`, `pnpm -C builder/ui run typecheck`. Your sandbox can't open sockets: run what you can and
say which you couldn't. Don't commit; Claude reviews and commits.
