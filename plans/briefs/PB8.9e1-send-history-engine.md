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

## Fix-up 1 (Claude's review of the first build, 2026-11-17)

The build is in the branch's last commit. Change only what follows; keep everything else as it is. Same skills,
same Never list.

1. **`fold` must require what `corpus::merge` requires** (`builder/ingest/src/corpus.rs` `merge`: a string `id`, a
   string `kind`, a string `timestamp`). Today a delivered record with no `kind` is stored in
   `decisions-from-<peer>.jsonl`, and the build computer's next `corpus::merge` then fails on that file, so one bad
   delivery stops every build. In `fold`, a new delivered record (after the filters pass it) with no string `kind`
   or no string `timestamp` fails the fold, as a missing timestamp already does (the delivery is refused, deleted,
   the cursor moves, the peer file is unchanged). Tests in `sending/tests.rs`: (a) a delivery holding one record
   with no `kind` is refused and leaves an existing peer file byte-for-byte unchanged; (b) after a successful take,
   the peer file passes `corpus::merge` with one `Input { name, mode: Mode::History, bytes }`, the assistant's
   compiled filters and the split `merge_histories_in` uses (find it in `builder/api/src/history.rs` or wherever
   `merge_histories_in` lives; use the same value), without error.
2. **No panic in `fold`'s sort.** Replace `a["timestamp"]` / `a["id"]` with `a.get("timestamp").and_then(Value::as_str)`
   and the same for `id` (a `Map` index panics on a missing key, and the old peer file is not re-checked).
3. **Screen another computer's name before it leaves the engine.** `delivery.computer` and the founder's name from
   `computer_list` are another computer's own text. Make `radio::shown` `pub(crate)` and pass every name the engine
   returns through it: `Sent.to` in `send_with`, and each `SentFrom.name` in `take_with` and `history_status`
   (store the name as given in the status files, screen on the way out). Test: a delivery whose `computer` is
   `"Work\u{1b}[31m laptop\u{202e}"` gives `take_deliveries` and `history_status` a name with `?` in place of the
   escape and the bidi mark, and the CLI's `take` line shows the same screened name.
4. **`last-sent.json` keeps the byte count.** Add `bytes: u64` to `LastSent` with `#[serde(default)]` (an older
   file without it reads as 0); `finish_send` writes it and `history_status` returns it. Update `.ai/DECISIONS.md`
   item 8: drop the sentence that says `lastSent.bytes` is zero; say `last-sent.json` holds `at`, `to` and `bytes`.
   Update `status_reads_both_files_and_pending` to check the bytes.
5. **The two red tests are wrong, not the code:**
   - `sending::tests::sends_pending_copy_to_accepted_founder_by_id`: `write_private` answers the canonical path
     (`/private/tmp/…` on macOS). Compare against `fs::canonicalize` of the expected path.
   - `builder/cli/tests/radio.rs` `radio_take_prints_totals_and_computers_without_history_or_keys`: the invented
     `key` is 68 hex characters; Radio's key is 64 (`peer_file` rightly refuses it). Make it exactly 64.

Gate as before. Your sandbox can't open sockets, so the fake-Radio tests may fail there with a socket error: say
which, and Claude runs them outside. Don't commit.

## Fix-up 2 (the security review and the Verifier on `dcd5e4f`, 2026-11-17)

Change only what follows. Same skills, same Never list, test-first, a mutation check per item.

1. **A peer's merged file has a ceiling.** `decisions-from-<peer>.jsonl` never shrinks, so an accepted computer
   sending 64 MiB deliveries of new ids grows it without limit, and every take reads it whole. Add
   `const PEER_LIMIT: u64 = 256 * 1024 * 1024;`. Read the old peer file with
   `private::read_private_optional_at_max(home, slug, None, &file, PEER_LIMIT)` (use the existing function; check its
   signature). A fold whose output is larger than `PEER_LIMIT` is refused like a bad delivery (counted `refused`, the
   delivery deleted, the cursor moves, the peer file unchanged). Count records with the fold's own record count
   (return it from `fold`, e.g. `(Vec<u8>, u64)`), not by parsing the merged bytes again. Test: with a small limit
   made testable (pass the limit into the function that folds, the public path uses `PEER_LIMIT`), an over-limit
   fold leaves the peer file byte-for-byte unchanged and counts one refused.
2. **Only a content problem discards a delivery.** Today any `Err` reading the delivery (an `EMFILE`, an `EIO`, the
   deliveries folder failing its ownership check) counts it refused, deletes it and moves the cursor, so a send is
   lost for good. Split the two: a bad name, a bad key, a link, a non-regular file, a file over 64 MiB, and a
   delivered file that fails `fold`'s shape or filter checks are refused, deleted and passed, as today; any other
   error (the old peer file unreadable or unparsable, an I/O error opening or reading the delivery, the write)
   returns `TAKE_FAILED` with the delivery left in place and the cursor not moved. Find how
   `read_private_optional_at_max` reports a link, a non-regular file and an over-size file (its error values) and
   match those exactly; everything else is I/O. Tests: the old peer file holding a line that isn't JSON makes
   `take` answer `TAKE_FAILED`, leaves the delivery file and `deliveries-taken.json` unchanged; the existing
   refused cases still pass.
3. **`remove_entry` doesn't recurse.** Delete a delivery entry with one `unlinkat(…, 0)`: a file or a link goes; a
   folder (`EISDIR`/`EPERM`) is left where it is and the delivery still counts refused and the cursor moves. Radio
   writes only regular files, so a folder there was put by a process of the person's own. Remove the recursive
   branch and its now-unused code; change the test `nonempty_delivery_directory_is_removed_without_following_nested_links`
   to show the folder and its nested link are left untouched (nothing outside followed), the delivery counted
   refused and the cursor moved.
4. **The threat model says what isn't covered.** In `.ai/SECURITY.md`'s row "History sent between a person's
   computers": say the state folder is never writable by others (an older build may have left it 0750, so the
   group can list file names but not read the 0600 files) instead of "0700 folders"; add the peer-file ceiling
   (256 MiB) and that an I/O fault leaves the delivery for the next take; and add, as not covered: if Radio's
   delivery ids ever started again from a lower number (its store reset), deliveries at or below the saved cursor
   would never be listed and would stay in `deliveries/` until Forget.
5. **A delivered question must pass the build's held-out check (the Verifier's must-fix).** A delivered
   `kind: "question"` record without `project` (or another key `corpus::split` projects) is taken, passes
   `corpus::merge`, then fails `corpus::split` when `merge`'s hold-out marks it held out, and every build stops.
   In `builder/ingest/src/corpus.rs`, move the two key lists `split` projects for a held-out question into one
   `pub fn question_shape(member: &Member, name: &str, line: usize, record: &Map<String, Value>) -> Result<()>` that
   checks every key of both lists (the second list's pick field is `member.pick_field()`), and have `split` keep
   its behaviour (same errors, same output; call the new function or share the lists, no second copy of either
   list). `fold` takes the `Member` (`take_with` already has it) and calls `question_shape` on every new delivered
   record whose `kind` is `"question"` (any question can be held out); a failure fails the fold (refused, as a
   missing `kind`). Also read `builder/build/src/build.rs` `parts` and anything else the build runs per record after
   `split`: if it requires a field `merge`, `split` and this check don't, apply the same treatment and say so; if
   not, say so. Tests: the Verifier's record
   `{"id":"q1","timestamp":"2999-01-01","kind":"question","question":"Which?","header":"Pick","options":[{"label":"A","description":"a"}],"picks":["A"],"own_words":"","notes":""}`
   is refused and the peer file is unchanged; a complete question record is taken; and after a take, the peer file
   run through `corpus::merge` with `Split::Holdout(1.0)` and then `corpus::split` succeeds. `corpus`'s existing
   tests stay green.
6. **Tests the Verifier showed missing.** Each must fail when its rule is removed (mutate a copy and say so):
   (a) a delivered record with `kind` but no string `timestamp` is refused, peer file unchanged; (b) keys of 63
   and 65 characters and a 64-character key with a non-hex character are each refused, deleted and passed;
   (c) a failed send when an older `pending-send.jsonl` exists leaves the new history's bytes in it, not the old;
   (d) a FIFO named like a history file is refused without hanging, removed, and passed. Make the test from item 5
   that runs the brief's existing "missing kind" case meaningful too, or delete it if (a) and item 5 cover it.
7. **Small ones.** `peer_file` takes lowercase hex only (Radio's own key check is lowercase). The CLI shows a
   computer's last time as `HH:MM` when it is today (local) and `YYYY-MM-DD` otherwise (keep `local_time` for the
   join code's expiry as it is; add the date form beside it); test both forms in `builder/cli/tests/radio.rs`
   (an `at` of `1800.0` is a past date).

Not changed, recorded instead: a delivery name outside Radio's character set is counted refused but left in place
(unreachable with a conforming Radio; no path is built from it), and the state lock is held across Radio's send
(up to its 150-second wait). Gate as before; don't commit.

## Fix-up 3 (Claude's review of fix-up 2, 2026-11-17)

1. **A delivered question's options must read as `distil` reads them.** `builder/steps/src/distil.rs` reads a
   question's options through `voice_steps::input::Record::options()` (`builder/steps/src/input.rs`: an array of
   objects, each with a string `label` and a string `description`) and fails the whole step otherwise. In `fold`,
   for a delivered `kind: "question"` record, after `corpus::question_shape`, build
   `voice_steps::input::Record { name: "history", line, map: &record }` and call `.options()`; an error fails the
   fold as content (refused). No copy of that rule. Test: a delivered question whose `options` is a string, and one
   whose option has no `description`, are each refused with the peer file unchanged; mutation-check it.
2. **The CLI fixture is wrong, not the code.** In `builder/cli/tests/radio.rs`
   `radio_take_prints_totals_and_computers_without_history_or_keys`, the invented message record has no
   `project`; fold now (rightly) requires `project` and `text` on a message, as `distil` does. Add
   `"project":"demo"` to it.

Gate as before; don't commit.

## Fix-up 4 (the repeat Verifier on `4fe67d4`: FAIL, 2026-11-17)

Checking a delivered record field by field against each step of the build has now missed four ways to break a
build (an `outcome` outside its values, a non-string `notes` on a message, a `kind` that is neither, a non-string
pick field). Stop chasing steps. **A delivered record is taken only if it has exactly the shape Voice's own reader
writes**, defined once, beside the writer.

1. **One schema, beside the writer.** In `builder/ingest/src/mine.rs`, next to `message_record` and the question
   writer, add `pub fn check_record(member: &Member, record: &Map<String, Value>) -> Result<()>`. First read
   `history::read_claude_code_in` (and anything it calls after mining: filters, redaction) to learn every key a
   record in `decisions-claude-code.jsonl` can carry, then require exactly that set, no other key (so a delivered
   `split` is refused too), with these types, read from the writer, not guessed:
   - `kind` is `"message"` or `"question"`; `id`, `timestamp`, `project` are strings.
   - message: `text` and `context` strings.
   - question: `header` and `question` strings; `multiSelect` a bool; `options` an array whose items have exactly
     the keys the writer's `normalized` items have, each of the type it writes; `recommended` a string or null;
     the member's pick field (`member.pick_field()`) a string or null; `picks` an array of strings; `own_words` and
     `notes` strings; `outcome` one of the values `mine::outcome` can return (name them once, as a const the
     writer and the check share, so they can't drift).
   - If the reader adds a key only sometimes, allow it only with the type it writes.
   A drift test in `mine.rs`'s tests: every record the existing mining tests produce (find the fixture(s) the
   question and message tests mine; run them through `check_record`) passes, so the writer can never write a
   record the check refuses.
2. **`fold` calls `mine::check_record` for every new delivered record, and nothing else for shape.** Remove from
   `fold` the piecemeal checks fix-ups 1 to 3 added (the `kind`/`timestamp` check, the `project`/`text` check,
   the `corpus::question_shape` call, the `Record::options()` call) once `check_record` covers each, and remove
   `corpus::question_shape` if nothing else calls it (keep `QUESTION_FIELDS`/`answer_fields` shared by `split`).
   Keep `privacy::privacy` (filters) and the id rules. Tests: the repeat Verifier's four records are each refused
   with the peer file unchanged: a complete question with `"outcome":"anything"`; a message with `"notes":5`; a
   record with `kind:"note"`; a held-out-ready question whose pick field is `1`; plus a message with an extra key
   and a question whose option lacks `description`. Keep the earlier refusal tests passing (rewrite their records
   to the schema where they relied on a looser one). Mutation-check: dropping any one rule of `check_record` fails
   a test.
3. **Classify a failed delivery read by what the file is, not by the error's words.** Replace
   `delivery_content_error`'s message matching: after a failed read, look at the delivery entry without following
   a link (`fstatat(…, AT_SYMLINK_NOFOLLOW)` on the deliveries folder's descriptor; add a small helper in
   `private.rs` beside `read_private_optional_at_max` if none exists, returning the entry's type, size and whether
   it exists). A link, anything that is not a regular file (a socket, a FIFO, a folder), a regular file over 64 MiB,
   or a regular file this user can't open (`EACCES`; Radio writes 0600 files the user owns, so this is someone
   else's doing) is content: refused, deleted, passed. Anything else (the stat itself failing, `EMFILE`, `EIO`) is
   I/O: `TAKE_FAILED`, delivery and cursor unchanged. Tests: a Unix socket named like a history file is refused,
   removed and passed (it blocked every take before); a mode-000 regular file likewise; the existing link, FIFO,
   size and folder tests still pass.

Not changed, recorded: the cursor-restore path after a failed delete has no test (it needs a delete that fails
after the cursor is written); folding a 256 MiB peer file parses it whole in memory.

Gate as before, plus `cargo test -p voice-ingest`; don't commit.

## Fix-up 5 (the repeat Verifier on `c259b31`: FAIL, 2026-11-17)

The schema is right: no record `check_record` takes is refused by a later step, and the writer's records all pass.
What it misses is size. One delivered record of a few MiB (a 9 MiB `context`, `text` or `notes`, or a 300 KB `id`)
passes, `build::parts` puts it alone in a part, its label prompt goes past the provider's 8 MiB limit, and that
error ends every later build. `fold` keeps stored records as they are, so the record never goes away.

1. **The writer's id and context limits, in `check_record`.** In `builder/ingest/src/mine.rs`:
   - `id` must be exactly `"{kind}:"` followed by 12 lowercase hex characters (`[0-9a-f]`), the form `record_id`
     writes. Name the 12 once (a const `record_id` and the check share).
   - A message's `context` must be at most 300 characters (`chars().count()`). Name the 300 once (a const
     `message_record`'s cut and the check share).
   - Tests in `mine.rs`'s tests, beside `check_record_requires_every_writer_field_and_type`: a message and a
     question whose id is too long, too short, has uppercase or non-hex, or names the other kind, are each refused;
     a context of 300 characters (multi-byte ones included) passes and 301 is refused. The drift test
     `mined_fixture_records_pass_check_record` must still pass unchanged.
2. **A delivered record too big for one build part is skipped, not taken.** In `builder/api/src/sending.rs`
   `fold`, after `check_record` passes, measure the record as one JSONL line the same way `build::parts` does
   (`voice_ingest::mine::to_jsonl(std::slice::from_ref(&record)).len()`); if it is greater than
   `voice_build::build::PART_BYTES`, skip that record (`continue`, like a filtered record) and keep folding the
   rest. Only that record: a real member's one huge pasted message must not sink their whole send. Add one line
   on the rule to `fold`'s doc comment.
   - Tests in `sending/tests.rs`: a delivery with one ordinary message and one message whose `text` makes its line
     `PART_BYTES + 1` bytes is taken: the peer file gains the ordinary record only. A record whose line is exactly
     `PART_BYTES` bytes is kept. A delivery whose only record has an over-long `context` or a malformed `id` is
     refused with the peer file unchanged (the existing refusal path).
3. **Mutation-check** each new rule (drop the id check, the context check, the size skip, flip `>` to `>=`): each
   must fail a test. Use `cp` backups and `cmp`, never `git checkout --`.

Not changed, recorded: a peer can copy a real record id from the build computer's own interview or live files, and
the merge keeps the first copy it reads (peer files sort first); the id form check doesn't stop a deliberate copy.
That was so before this slice and is a PB8.9 design note, not this fix-up's.

Gate: `cargo fmt --check`; `cargo clippy --all-targets --locked -- -D warnings`; `cargo test --locked -p
voice-ingest`, `-p voice-api`, `-p voice-cli --test radio` (the socket tests can't run in your sandbox: say which
you couldn't run). Load is high here: `-j 2` and `--test-threads 2`. Don't commit.

## Fix-up 6 (the repeat Verifier on `0a460af`: FAIL, 2026-11-17)

Fix-up 5 closed the label prompt: no delivered record can push one past the provider's 8 MiB. Two later steps
still can, with records under `PART_BYTES` that `check_record` takes, and a stored record never goes away:

- **Distil.** A row copies `project` (and, for a question, `q`, the option labels and `picks`) in full;
  `distil_prompt` shortens only `msg`, `own` and `notes` (`builder/build/src/build.rs` `distil_prompt`), and a
  prompt over `DISTIL_BYTES` (700,000) ends the build. Two delivered messages with a 390 KB `project` and the same
  text land in one group and end every build. Locally `project` is a folder name (`mine::project_name`), at most
  255 bytes.
- **The backtest.** Held-out questions are the newest by timestamp, `split` keeps `project`, `header`, `question`
  and `options`, and `backtest_prompt` puts up to `BATCH` (40) of them, plus up to `PRECEDENTS` (6) precedents
  each, into one prompt. Thirty delivered questions of 390 KB dated far in the future go past 8 MiB.

**The rule:** a delivered record may be no bigger than a real record of that kind, and small enough that a full
backtest batch of them fits one prompt. Then a peer can do nothing a heavy local history can't.

1. **The writer's two missing limits, in `check_record`** (`builder/ingest/src/mine.rs`):
   - `project` is at most 255 bytes (`len()`), a const named for the folder-name limit and documented as such.
   - a message's `text` is at most 4000 characters (`chars().count()`), one const shared with `own_message`'s
     `> 4000` (replace the literal there).
   - Tests beside the fix-up 5 ones: 255 bytes of `project` passes, 256 is refused; 4000 characters of `text`
     passes (also with a multi-byte character), 4001 is refused. The drift test still passes unchanged.
2. **A smaller line limit for a delivered record.** In `builder/api/src/sending.rs`, replace fix-up 5's
   `PART_BYTES` skip with `const DELIVERED_RECORD_BYTES: usize = 16 * 1024;`, measured the same way (one JSONL
   line, `mine::to_jsonl`), still skipping only that record. Its doc comment says why 16 KiB: a full backtest batch
   (40 held-out questions, each with 6 precedents) of records this size stays under the provider's 8 MiB, and a real
   message (4000 characters, a 300-character context, a 255-byte project) fits unless it is mostly emoji or control
   characters, which is then skipped on its own, not the whole send. Add a test (no socket needed) that pins the
   arithmetic with the real consts: `voice_build::build::BATCH * (1 + voice_build::build::PRECEDENTS) *
   DELIVERED_RECORD_BYTES < voice_agent::provider::MAX_PROMPT` (`voice-api` already depends on both crates; check
   the module paths before writing it). Move fix-up 5's two size tests to
   the new limit: `DELIVERED_RECORD_BYTES + 1` is skipped beside an ordinary record, exactly
   `DELIVERED_RECORD_BYTES` is kept.
3. **Mutation-check** each new rule (drop the project check, the text check, flip each `>` to `>=`, put the skip
   back to `PART_BYTES`): each must fail a test. `cp` backups and `cmp`, never `git checkout --`.

Not changed, recorded: a peer chooses its records' timestamps, so it can make its own questions the held-out set
(the score then measures those); with every record at most 16 KiB this can't end a build. Distil's group limit
(about 43 delivered questions of 16 KiB in one category reach `DISTIL_BYTES`) is the same limit a large local
history already meets ("splitting a large group is not built yet"). A peer can still make the build computer pay
for one label call per part of its records, up to its 256 MiB peer file.

Gate: `cargo fmt --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings`; `cargo test --locked -p
voice-ingest --lib`, and the voice-api `sending` tests (`cargo test --locked -p voice-api --lib sending`); say which
socket tests your sandbox couldn't run. Load is high: `-j 2` and `--test-threads 2`. Don't commit.
