# VF48 brief: a stored record keeps its folder spelling, re-checked against the fences when it is read

Card: VF48 (Persona Builder board). Lane V, runner V1. You are Codex; implement this in the worktree you were
started in. Read `AGENTS.md` first. Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`.

## The defects (found by VF45's Verifiers; reproduced by Claude at `f00efe3` by reading the code)

All code is in `scripts/crew_decisions.py`. `record_compartment(c, record)` decides which compartment a stored
record belongs to and returns `"fenced"` for one today's fences cover; `cmd_similar` drops those. Since VF45 it
re-checks the record's `project` name and a stored `p:` compartment. Two cases still get through:

1. **A message whose cwd was inside a fence added after mining, while its transcript folder was not.**
   `cmd_mine` names the record after the transcript folder (`project_name(folder.name)`) and stores the session's
   compartment. Say `~/projects/work` is the transcript folder and the message's `cwd` was
   `~/projects/work/secret-sub`; the record is `{"project": "work", "compartment": "work", …}`. Fence
   `~/projects/work/secret-sub` today and the record is still served: neither `work` nor `p:work` holds the
   fence's spelling. Only mining again drops it (`record_cwd_drop_reason`).
2. **A live-log record saved under a declared compartment.** `live_record(m, line)` gives every live record the
   project `live` and copies `line["compartment"]` as it is; it never looks at the line's `work` path. A line
   recorded with `--work ~/projects/client/secret/x.md` under compartment `client`, before
   `~/projects/client/secret` was fenced, is kept by every later `mine` and served by `similar`.

## The fix (decided; do exactly this)

### 1. One lexical fence check on a folded folder spelling

Add, right before `folder_compartment`:

```python
def fence_folder_spellings(c):
    """The folded encode and fence_encode spelling of every fence, computed once per validated map."""
```

It returns `tuple(dict.fromkeys(s for f in c["fenced"] for s in (fold(encode(f)), fold(fence_encode(f)))))`,
cached on the map exactly the way `fence_name_spellings` caches (`getattr(c, "_folder_spellings", None)`, stored
only `if isinstance(c, CompartmentMap)`).

```python
def folder_fenced(c, folded):
    """True when a folded folder spelling (fold(encode(path)), Claude Code's transcript folder name) holds a
    fence's spelling. Reads no file or folder."""
    return any(dashed_in(s, folded) for s in fence_folder_spellings(c))
```

Then the first two lines of `folder_compartment` become:

```python
    folded = fold(folder_name)
    if folder_fenced(c, folded):
        return "fenced"
```

Same result as today (the same two spellings per fence, the same `dashed_in`); it only stops recomputing them.

### 2. Every mined record keeps its folder spelling: the key `fence_folder`

- **`cmd_mine`.** In the `for h, cwd in _harvest(…)` loop, right after `cwd_folder = …`, add
  `if cwd_folder is not None: h["fence_folder"] = fold(cwd_folder)`. Always, with or without a compartment map:
  a fence can be added after mining. Nothing else in the loop changes.
- **`cmd_record`.** The live line gains the key `"fence_folder"`, placed right after the principal's answer key
  and before `"compartment"`:
  `fold(encode(os.path.abspath(absolute_path(values["work"], cwd_of(args)))))`. Always, with or without a map.
  The line's `"work"` stays as the person gave it.
- **`live_record`.** Right after the `isinstance(line, dict)` check, work out the line's folder spelling:
  - `line["fence_folder"]` when it is a non-empty `str`;
  - else, when `line.get("work")` is a non-empty `str` and `Path(work).expanduser()` is absolute,
    `fold(encode(os.path.abspath(Path(work).expanduser())))`;
  - else `None` (a line written before VF48 with a relative `work`, or none).

  Then, when `m.compartments is not None`:
  - a spelling for which `folder_fenced(m.compartments, spelling)` is true returns the skip reason
    `"in a fenced folder"`;
  - no spelling at all while `m.compartments["fenced"]` is non-empty also returns `"in a fenced folder"`
    (it fails closed: the line cannot say where it came from).

  A record that is built gets `"fence_folder": spelling` right after `"project": "live"` when the spelling is
  not `None`. Nothing reads `"work"` from the line besides this.
- **`mine_live`'s summary.** After `{skipped['unreadable lines']} unreadable lines`, append
  `f", {skipped['in a fenced folder']} in a fenced folder"` only when that count is non-zero (the way `cmd_mine`
  appends `fenced records`). No other output changes.

### 3. `record_compartment` re-checks the stored spelling

After the two existing `project_fenced` checks and before `if "compartment" in record:`, add:

```python
    if "fence_folder" in record:
        folder = record["fence_folder"]
        if not isinstance(folder, str) or not folder or folder_fenced(c, folder):
            return "fenced"
```

A present but malformed `fence_folder` (not a string, or empty) fails closed. A record with no `fence_folder`
(mined before VF48) behaves exactly as today. Update the docstring to say the record's stored folder spelling is
re-checked too.

### 4. Nothing else changes

Not `project_fenced`, `fence_name_spellings`, `path_compartment`, `record_cwd_drop_reason`, `cmd_similar`,
`cmd_merge` (it already writes whole records, so the key travels) or any fence-walk code. Never `stat`,
`resolve`, list or open anything in the new checks: they work on strings only.

### 5. `.ai/DECISIONS.md`

Add an entry right after the VF45 one: `## 2026-11-21: a stored record keeps its folder spelling, re-checked
against the fences when it is read (VF48)`, 4 to 6 lines, plain sentences: every mined record keeps its
message's folded cwd spelling, and every live line and live record the spelling of its `work` path, as
`fence_folder`; a record whose spelling holds a fence's spelling is fenced when it is read, so a fence added
after mining hides it without mining again, and `mine` drops a fenced live line; the check reads no file or
folder; a record mined before VF48 carries no spelling, so mining again adds it; a live line written before
VF48 with a relative `work` path can't say where it came from, so it is dropped whenever any folder is fenced;
the stored spelling holds the folder's full path, home included, and stays in the member's private state like
the rest of the record. Leave the VF45 entry as it is.

## Tests first (red, then green), in `tests/crew_decisions_test.py`, class `Compartments`

Invented records and folders only, under the test home the class already uses. Write them all first, run them,
and confirm each fails on its assertion on the old code (not on an import or a typo). Then implement.

1. `test_record_stored_folder_spelling_is_rechecked_against_fences`: with fences `["~/garden-club",
   "~/projects/work/secret-sub"]`, `record_compartment` gives `"fenced"` for
   `{"project": "work", "compartment": "work", "fence_folder": fold(encode("~/projects/work/secret-sub/notes"))}`,
   for the same record without `"compartment"`, and for a fence spelled `~/Projects/Work/Secret-Sub`; it gives
   `"work"` for `fence_folder` `fold(encode("~/projects/work"))` and for the record with no `fence_folder`.
   Build the spellings with the module's own `fold` and `encode`.
2. `test_record_malformed_folder_spelling_fails_closed`: `fence_folder` of `""`, `None`, `3` and `["x"]` each
   give `"fenced"`, with only `~/garden-club` fenced.
3. `test_record_stored_live_record_folder_is_rechecked`: `{"project": "live", "compartment": "client",
   "fence_folder": fold(encode("~/projects/client/secret/x.md"))}` is `"fenced"` with `~/projects/client/secret`
   fenced and `"client"` without it.
4. `test_mine_keeps_the_message_folder_spelling_and_a_later_fence_hides_it` (end to end through the CLI, the
   way the class's other `mine` tests build transcripts): a transcript folder for `~/projects/work` holding one
   message whose `cwd` is `~/projects/work/secret-sub`. Mine with only `~/garden-club` fenced: the record has
   `fence_folder == fold(encode(<that cwd>))`. Then add `~/projects/work/secret-sub` to the fences, save the
   config, and run `similar --compartment all --corpus <the mined file> --query <the message text> --json`: it
   prints `[]`. With the fence absent it serves the record.
5. `test_mine_live_drops_a_line_a_later_fence_covers`: a live log (write it directly, the shape `cmd_record`
   writes) with one answered line, `"work": "~/projects/client/secret/x.md"`, `"compartment": "client"`, no
   `fence_folder`. With `~/projects/client/secret` fenced, `mine` with the live source keeps 0 answers and its
   output says `1 in a fenced folder`; with only `~/garden-club` fenced it keeps 1 and the stored record's
   `fence_folder` is `fold(encode(<the expanded work path>))`. Find how the existing live-source tests run
   `mine_live` (search `"kind": "live"` and `live.jsonl` in `tests/`) and follow them.
6. `test_mine_live_line_without_a_folder_fails_closed_only_under_fences`: a line with `"work": "notes/x.md"`
   and no `fence_folder` is dropped as `in a fenced folder` when the map fences anything, and kept when the
   member has no compartment map; a line with a relative `work` but a `fence_folder` outside every fence is kept.
7. `test_record_writes_the_work_folder_spelling`: `record --cwd <home>/projects/work --work notes/x` writes
   `fence_folder == fold(encode(<home>/projects/work/notes/x))` and keeps `"work": "notes/x"`. Update
   `test_record_uses_the_member_and_principal_keys` (it pins the line's keys and values) to expect
   `fence_folder` after `sam_answer`, with the value computed the same way from the harness's cwd; that is the
   only existing assertion you change.
8. `test_fence_folder_spellings_are_computed_once_per_map`: after one `record_compartment` call with a
   `fence_folder`, 50 more such calls run with `fence_encode` patched to raise (copy
   `test_fence_name_spellings_are_computed_once_per_map`).

Mutation checks (keep the scratch copy outside the repo; `cp` the file back and `cmp` it after each; never
`git checkout --`): (a) drop the `folder_fenced` line in `record_compartment` → tests 1, 3 and 4 go red;
(b) drop the `h["fence_folder"]` line in `cmd_mine` → test 4 goes red; (c) drop the fenced return in
`live_record` → tests 5 and 6 go red; (d) make the no-spelling case keep the line → test 6 goes red; (e) drop
the malformed check → test 2 goes red. Report each result.

Then run, from the repo root, `python3 -m unittest tests.crew_decisions_test` and every other suite that imports
`crew_decisions` (`grep -l crew_decisions tests/*.py`), each green. Run them with 2 workers at most; the machine
is throttled.

## Commits

Two commits, messages in a file (`git commit -F`), no AI trailer:
1. `crew tests: VF48, a stored record's folder spelling and a live line's work path are re-checked against today's fences (red; lane: V1)`
2. `crew: VF48, every mined record keeps its message's folded cwd spelling and every live record its work path's, re-checked against the fences when read, and mine drops a fenced live line (lane: V1)` — with the code and the DECISIONS entry.

End with a short report: the commits, each test's red reason, each mutation's result, the suite totals.
