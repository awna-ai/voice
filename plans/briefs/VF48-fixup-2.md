# VF48 fix-up 2 brief: Rust chunk drops the spelling, both spellings are kept, and two spelling edge cases

Card: VF48. Lane V, runner V1. You are Codex; implement this in the worktree you were started in, on top of
`HEAD` (`1f4e4e2`). Read `AGENTS.md`, `plans/briefs/VF48-record-folder-fence-recheck.md` and
`plans/briefs/VF48-fixup-1.md` first. Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`. Leave every change
uncommitted (your sandbox can't write git metadata); Claude commits. Scratch only under a fresh `mktemp -d`,
removed when done.

The repeat Verifier (Opus) failed `1f4e4e2`. Fix exactly these.

## Must-fix

1. **Rust `voice chunk` still writes the spelling.** `builder/steps/src/chunk.rs` copies whole train records,
   so `fence_folder` (the full cwd spelling, home login included) still reaches the labeling files, and Rust and
   Python chunk output now differ. Fix: the Rust chunk drops `fence_folder` and `fence_folder_resolved` (item 2)
   from each train record it writes, exactly as Python's `cmd_chunk` does (Python drops both keys too). Add a
   Rust test that chunks a corpus line carrying both keys and asserts neither is in `train-01.jsonl` while every
   other key is kept in order; if a Python-parity harness exists for chunk (search `builder/steps/tests`), add
   the case there too.
2. **Storing only the resolved spelling lets a fence written through the link miss.** With `follow: ["~"]`,
   `~/work -> ~/projects/acme`, `record --work ~/work/secret/x.md`, then `sources mine`, then the fence
   `~/work/secret`: `similar --compartment all` serves the record (it served `[]` at `1c258a2`). Fix: the line
   and the record keep both spellings. `fence_folder` is always the lexical spelling (the same value as with no
   map, which keeps the Rust parity). Under a compartment map, when the resolved spelling differs from it,
   `cmd_record` also writes `"fence_folder_resolved"` right after `fence_folder`. `live_record` checks every
   spelling it has (a valid `fence_folder`, a valid `fence_folder_resolved`, the `work` spelling) and copies
   `fence_folder_resolved` onto the record right after `fence_folder` when it is valid. A present but invalid
   `fence_folder_resolved` is treated like an invalid `fence_folder` (fenced under a map). `record_compartment`
   checks `fence_folder_resolved` exactly as it checks `fence_folder` (present and invalid → `"fenced"`, a fence
   spelling in it → `"fenced"`). Python `cmd_chunk` drops both keys.

## Should-fix

3. **A relative `--cwd` under a map stores a broken spelling.** `safe_resolve` with a relative path drops its
   first part (an old `walk_resolve` behaviour), so `record --cwd work --work notes/x.md` stores `notes-x-md`
   under a map and the next `sources mine` drops the line. Fix: `cmd_record` resolves the lexical absolute path
   (`safe_resolve(m.compartments, <lexical_path result>, "/")`), never the raw relative one. Do not change
   `safe_resolve` or `walk_resolve`.
4. **`~//x` is spelled `/x`.** `Path.home() / "/x"` drops the home; `encode`, `expanduser` and the shell read
   `~//x` as `<home>/x`. Fix in Python `lexical_path` and in Rust `fence_folder`: for a leading `~/`, strip every
   leading `/` from the rest before joining it to home (`~//x` and `~///x` → `<home>/x`). Update the Rust parity
   case named "home path with an absolute remainder" to a fitting name; both sides must still match.
5. **`absolute_path` still crashes on `~text`.** `record --cwd "~5" --work x` exits 1 with `RuntimeError: Could
   not determine home directory.` under a map. Fix: `absolute_path` expands only `~` and a leading `~/` (the same
   rule as `encode` and `lexical_path`; config paths are already limited to `/` and `~/`), anything else as an
   ordinary relative name.

## DECISIONS

In the VF48 entry, replace "Under a compartment map, recorded `work` is stored resolved, so a link cannot hide
it." with one sentence saying that under a compartment map a recorded `work` keeps its resolved spelling beside
the written one and both are checked, so neither a link nor a fence written through one can hide it. Keep the
entry's wrap.

## Tests first (red on `HEAD`, on their assertions), then green

Python, class `Compartments`: the link-then-fence case of item 2 (create the link in the test's own temp home;
assert `similar` serves `[]` and the stored record carries both keys); `fence_folder_resolved` invalid values
fence in `record_compartment` and in `live_record` under a map; item 3's relative `--cwd` stores
`fold(encode(<abs cwd>/notes/x.md))` and the line is kept by `sources mine`; item 4's `~//x` spelling equals
`fold(encode(<home>/x))`; item 5's `record --cwd "~5" --work x` under a map exits 0; Python `chunk` drops
`fence_folder_resolved`. Rust: item 1's chunk test; the record and mine parity tests still pass.

Mutations (scratch copy, `cp` back, `cmp`; never `git checkout --`): drop the resolved check in
`record_compartment`; write only the resolved spelling again; resolve the raw relative path; drop the `/` strip
in Python and in Rust; drop the Rust chunk key removal. Each must turn its test red.

## Run, at most 2 heavy processes at a time

`python3 -m unittest tests.crew_decisions_test` and the other suites importing `crew_decisions`;
`cargo fmt --check`; `VOICE_REQUIRE_PARITY=1` mine parity and cli record tests; the touched steps crate's tests;
`cargo clippy --all-targets --locked -- -D warnings` on the touched crates, with the env and features
`scripts/ci-local.sh` uses. End with a short report: red reasons, mutation results, totals.
