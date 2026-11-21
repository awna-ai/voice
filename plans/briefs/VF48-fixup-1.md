# VF48 fix-up 1 brief: the Rust port writes the spelling too, no crash on `~text`, and the re-check closes its gaps

Card: VF48. Lane V, runner V1. You are Codex; implement this in the worktree you were started in, on top of
`HEAD` (`1c258a2`). Read `AGENTS.md` and `plans/briefs/VF48-record-folder-fence-recheck.md` first. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`. You cannot write git metadata in your sandbox: leave every change
uncommitted; Claude commits. Write nothing outside this worktree except scratch files under a fresh
`mktemp -d`, and remove those when done.

The Verifier (Fable) failed `1c258a2` and the security review found three more gaps. Fix every item below, in
this order. Items 1 to 4 are must-fix.

## Must-fix

1. **The Rust mine no longer matches Python byte for byte.** `builder/ingest/tests/mine_parity.rs`
   `all_accepted_fixtures_match_python_bytes_and_summary` fails (`JSONL case ada_fixture`): Python `mine` now
   adds `fence_folder` to every record with a cwd and `builder/ingest/src/mine.rs` does not. `scripts/ci-local.sh`
   runs it with `VOICE_REQUIRE_PARITY=1`, so the gate is red. Fix in `mine.rs`: wherever the record is kept, when
   the line's cwd is a non-empty string, insert `"fence_folder"` as the record's last key, with the value item 5
   defines (the same spelling Python stores). Python's `encode` output is ASCII only (`[^A-Za-z0-9]` → `-`), so
   `fold` of it is ASCII lowercasing: use `to_ascii_lowercase()` on the encoded spelling in Rust. Keep the
   existing `cwd_folder` used for the excluded filter and `cwd_score` exactly as it is.
2. **The Rust `voice record` no longer matches Python byte for byte.** `builder/cli/tests/record.rs`
   `stdin_records_match_flags_and_python_bytes` and `records_match_python_bytes_with_and_without_optional_fields`
   fail: Python's line ends with `"fence_folder"`, Rust's has no such key. Fix in `builder/cli/src/record.rs`:
   insert `"fence_folder"` right after the answer field, computed exactly as item 3's Python helper computes it
   with no compartment map (lexical, no filesystem). Python's base folder for a relative `work` is
   `cwd_of(args)`: `--cwd`, else `$CLAUDE_PROJECT_DIR`, else the process's current folder. The Rust command has no
   `--cwd`; use `$CLAUDE_PROJECT_DIR` when set and non-empty, else `std::env::current_dir()`. Expand only `~`
   and a leading `~/` with the home path the command already uses. Normalise `.` and `..` lexically the way
   Python's `os.path.abspath` does. Both parity tests must pass, and the two lines must stay byte-identical.
3. **`record --work "~5 minute fix"` crashes.** `cmd_record` exits 1 with `RuntimeError: Could not determine
   home directory.` (from `Path.expanduser()` on `~5…` or `~nosuchuser/…`), even with no compartment map; before
   VF48 it exited 0. Add one helper in `scripts/crew_decisions.py`, right before `cmd_record`:

   ```python
   def lexical_path(path, cwd):
       """path made absolute without touching the filesystem: only "~" and a leading "~/" are this home;
       any other "~text" is an ordinary relative name. None when it stays relative and cwd is None."""
   ```

   `~` → `Path.home()`; `~/rest` → `Path.home() / rest`; anything else as written. A relative result is joined
   to `cwd` (itself passed through the same `~` rule) or returns `None` when `cwd` is `None`. Return
   `Path(os.path.abspath(p))`. Never call `expanduser` on anything else. `cmd_record` uses
   `lexical_path(values["work"], cwd_of(args))` for the spelling (item 7 adds the map case).
4. **One live line with `"work": "~5 minute fix"` makes `sources mine` exit 1** with the same error and leaves
   `decisions-live.jsonl` stale. `live_record` uses `lexical_path(work, None)` instead of
   `Path(work).expanduser()`. No other change to `mine_live`'s exception handling.

## Should-fix

5. **The spelling is never normalised, so `..` escapes.** A cwd `…/projects/work/open/../secret-sub`, with
   `~/projects/work/secret-sub` fenced later, is dropped by a re-mine but served by `similar`. Define the stored
   spelling everywhere as `fold(encode(lexical_path(<path>, <base>)))`: in `cmd_mine` with base `None`
   (a transcript's cwd is absolute; a relative or `~text` cwd that stays relative gets no `fence_folder`), in
   `cmd_record`, and in `live_record`. Rust item 1 normalises the same way (`.`, `..`, `~` and `~/` as above;
   a relative cwd gets no `fence_folder`). Add one parity fixture line or case if the existing fixtures have no
   cwd containing `..`, so the Rust and Python spellings are compared on it.
6. **A live line's own `fence_folder` outranks its `work`.** `work: "~/projects/client/secret/x.md"` with a
   clear `fence_folder` (or `"x"`, or `" "`) is kept. Fix: in `live_record`, compute both the line's
   `fence_folder` and the absolute `work` path's spelling; return `"in a fenced folder"` when either is fenced.
   Store the line's `fence_folder` when it is valid (item 8), else the `work` spelling. Neither, while the map
   fences anything → `"in a fenced folder"`, as today.
7. **A symlinked `work` spelling escapes.** With `~/link-in -> ~/projects/client/secret`, `record --work
   ~/link-in/x.md` before the fence stores `…-link-in-x-md`, so after the fence `sources mine` keeps it and
   `similar` serves it. Fix: when `m.compartments is not None`, `cmd_record` stores the spelling of
   `safe_resolve(m.compartments, values["work"], cwd_of(args))[0]` (the resolved path; `cmd_record` already
   refuses a fenced one, and `safe_resolve` never steps inside a fence). With no map it stays lexical (item 3),
   which keeps the Rust parity.
8. **An unfolded or raw stored spelling fails open.** `{"fence_folder": encode("~/projects/client/secret/x.md")}`
   (capitals kept), a raw path string, or `"x"` give `client`. Fix: a valid spelling is a `str` that
   `re.fullmatch(r"-[a-z0-9-]*", …)` accepts (every real one starts with `-`, an encoded `/`). In
   `record_compartment`, a present `fence_folder` that is not valid → `"fenced"`. In `live_record`, a present
   line `fence_folder` that is not valid → `"in a fenced folder"` when `m.compartments is not None`; with no map
   it is ignored.
9. **A record mined on another Mac escapes a `~` fence.** A record whose `fence_folder` is
   `-users-al-projects-work-secret-sub-notes` is served on a Mac whose fence `~/projects/work/secret-sub`
   spells this home's login. Fix: add `stored_fence_spellings(c)`, cached like `fence_folder_spellings`
   (`_stored_spellings`), returning `fence_folder_spellings(c)` plus, for each of those spellings that starts
   with `fold(encode(Path.home())) + "-"`, the rest from that `-` on (for `~/projects/work/secret-sub`,
   `-projects-work-secret-sub`), so the folder matches under any home. Add `stored_folder_fenced(c, folded)`
   (the same `any(dashed_in(…))` over those) and use it in `record_compartment` and `live_record` instead of
   `folder_fenced`. `folder_compartment` keeps `folder_fenced`: transcript folders are this Mac's own.
10. **A fence spelled with `..` is not matched by spelling.** `check_compartments` accepts
    `~/projects/client/../client/secret`. Fix: `fence_folder_spellings` also adds the spellings of
    `os.path.abspath(Path(f).expanduser())` (both `encode` and `fence_encode`), keeping the as-written ones.
11. **The spelling reaches the labelers.** `cmd_chunk` copies whole train records into `train-NN.jsonl`, which
    the guided build hands to labeling sub-agents, so the full cwd (home login included) leaves the private
    corpus. Fix: `cmd_chunk` writes each train record without the `fence_folder` key (build new dicts; the held-out
    chunks already use `QUESTION_FIELDS`).
12. **Declare the new caches** `_folder_spellings = None` and `_stored_spellings = None` on `CompartmentMap`,
    with the other cache slots.

## Notes (no change)

Multi-character case folds (U+0130 and similar) are not covered by `dashed_in`; that is VF44's family and
gets its own card. Pre-VF48 stored records without a spelling are covered only after a re-mine; pre-VF48
relative-work live lines are dropped under any fence. Both are already in the VF48 DECISIONS entry.

## DECISIONS

Rewrite the VF48 entry in `.ai/DECISIONS.md` (keep it 6 to 9 lines, wrapped like its neighbours, plain
sentences) so it also says: the spelling is of the normalised absolute path (`~text` other than `~` and `~/`
is an ordinary name); a recorded `work` under a compartment map is stored resolved, so a link can't hide it;
a record's spelling also matches a `~` fence under any home, a fail-closed over-fence like VF45's; a stored
spelling must look like one (`-[a-z0-9-]*`) or the record is fenced; `chunk` leaves the spelling out of the
labeling files; the Rust mine and `voice record` write the same spelling.

## Tests first (red on `HEAD`, on their assertions), then green

Python, class `Compartments` in `tests/crew_decisions_test.py`, invented folders only:
- `test_record_and_live_line_with_a_tilde_name_do_not_crash`: `record --work "~5 minute fix"` exits 0 with and
  without a map and stores the spelling of `<cwd>/~5 minute fix`; `sources mine` over a live log holding a line
  with `"work": "~5 minute fix"` exits 0 (the line is kept with no map, dropped as `in a fenced folder` under a
  fencing map).
- `test_mined_spelling_is_normalised`: a mined message with cwd `<home>/projects/work/open/../secret-sub` stores
  `fold(encode("<home>/projects/work/secret-sub"))`, and with that folder fenced later `similar` serves nothing.
- `test_live_line_spelling_never_outranks_a_fenced_work_path`: item 6's inputs.
- `test_record_stores_the_resolved_work_under_a_map`: item 7's link (create it in the test's own temp home),
  then the fence, then `sources mine` keeps 0 and `similar --compartment all` serves 0.
- `test_stored_spelling_must_look_like_one`: item 8's values in `record_compartment` (`"fenced"`) and in
  `live_record` under a map (`"in a fenced folder"`); a valid clear spelling still routes.
- `test_record_mined_under_another_home_is_fenced_by_a_home_fence`: `HOME` patched to `/Users/sam` (as
  `test_record_fence_on_home_or_projects_fences_every_record` does), fence `~/projects/work/secret-sub`; a record
  with `fence_folder` `fold(encode("/Users/al/projects/work/secret-sub/notes"))` is `"fenced"`, a line with
  absolute `work` `/Users/al/projects/work/secret-sub/x.md` gives `"in a fenced folder"`, a record spelled
  `fold(encode("/Users/al/projects/work"))` stays `"work"`; `folder_compartment` of
  `-Users-al-projects-work-secret-sub` returns what it returns on `HEAD` (compute it there first and pin it).
- `test_record_fence_with_dot_dot_matches_its_folder`: fence `~/projects/client/../client/secret`; a record and
  a live line under `~/projects/client/secret/x.md` are fenced.
- `test_chunk_drops_the_stored_folder_spelling`: follow the existing `chunk` tests; the key is gone and every
  other key is kept.
- In `test_mine_keeps_the_message_folder_spelling_and_a_later_fence_hides_it`, also mine once with a member that
  has no compartment map and assert the record still carries `fence_folder`.

Rust: the two record parity tests and `mine_parity` must pass; add the `..` cwd case from item 5 to the mine
parity comparison if none exists.

Mutation checks (scratch copy outside the repo, `cp` back and `cmp` after each, never `git checkout --`):
drop `lexical_path`'s `~text` rule (use `expanduser`) → the tilde test goes red; drop the `abspath` → the
normalised test goes red; drop item 6's work check → its test goes red; drop item 7's resolve → its test goes
red; drop item 8's pattern → its test goes red; drop item 9's home-less spellings → its test goes red; drop item
10's abspath spellings → its test goes red; drop item 11 → its test goes red; drop the Rust `fence_folder`
insert in `mine.rs` and in `record.rs` → their parity tests go red.

## Run, at most 2 test processes at a time (the machine is throttled)

- `python3 -m unittest tests.crew_decisions_test` and every other suite that imports `crew_decisions`
  (`grep -l crew_decisions tests/*.py`).
- `cargo fmt --check`; `VOICE_REQUIRE_PARITY=1 cargo test --locked -p voice-ingest --test mine_parity` and
  the cli record tests (find the package names in `builder/*/Cargo.toml`; use the env and features
  `scripts/ci-local.sh` gives them); `cargo clippy --all-targets --locked -- -D warnings` for the two touched
  crates.

End with a short report: each test's red reason, each mutation's result, every suite and cargo command's totals.
