# VF35 fix-up 2: the messages end in a fix, and the Verifier's test should-fixes (lane: V1)

Branch `lane-v1/VF35`, on top of fix-up 1 (`89147ab`). Code goes in `scripts/crew_decisions.py`, tests in
`tests/crew_decisions_test.py`, class `FenceLinks`. Change no other file: Claude writes the decision text in
`.ai/DECISIONS.md` and the record. Skill: `~/.codex/skills/tdd-skill/SKILL.md` (red first, a mutation per claim,
restore with a `cp` backup and `cmp`, never `git checkout --`). Commit with `git commit -F <file>`, no AI attribution
or `Co-Authored-By` trailer. No network. Never print an environment value.

## The decision this fix-up builds (made by V1, 2026-11-20)

VF35 keeps **refusing** the Mac's alias spellings; it does not switch to comparing spellings. The Verifier's
should-fix (1) is a folder that exists only on the data volume, such as stock macOS `/home`
(`/home -> /System/Volumes/Data/home`, checked on this Mac). Written `/home/x`, it is refused as "reached through a
link into /System/Volumes/Data"; written `/System/Volumes/Data/home/x`, it is refused as "starts with
/System/Volumes/Data". Today each message tells the user to write the other spelling, so they point at each other.
That stays refused (failing closed is the safe side, and such a folder is rare), but each message must end in a fix
that works: move the folder into the home folder.

## 1. The two messages (code)

In `refuse_fence_links`, replace the three message texts that use `mac_alias` with these, exactly
(`{kind}` is `fenced` or `follow`, `{value}` is `shown(value)`, `{prefix}` is `shown(prefix)`):

- Written in an alias spelling (both the `follow` and the `fenced` check):
  `compartments: the {kind} folder {value} starts with {prefix}, a second name the Mac uses for other folders; write its path without {prefix}, or, if it has no path without it, move the folder into your home folder`
- A fence whose resolved path lands in an alias spelling:
  `compartments: the fenced folder {value} is reached through a link into {prefix}, a second name the Mac uses for other folders; Voice can only fence a folder it can name without {prefix}, so move the folder into your home folder or fence the folder the link stands in for`

Update every existing assertion of the old texts to the new ones. Nothing else in the checks changes.

## 2. New and tightened tests (all in `FenceLinks`)

a. **`test_data_volume_spelling_refused_before_any_look`: zero looks.** Today it only asserts no probe touched an
   alias path. Assert instead that `fence_looks`' probes record **no call at all** whose first argument is a path
   (any path), for every subtest: the refusal comes before Voice looks at anything. Keep the subtests. Add two:
   `("follow", "/system/volumes/data" + str(self.home), "/System/Volumes/Data")` (lower-case follow) and
   `("follow", "/.nofollow" + str(self.home), "/.nofollow")`. Check first that `prefix` in the message is
   whatever `mac_alias` returns for that spelling; if it returns the written case (`/system/volumes/data`),
   do not change the code: assert `str(DATA_VOLUME)` only if that is what the code shows, and report which.
b. **Lower-case link target (check 2).** A copy of `test_fence_through_link_into_data_volume_refused` whose link `L`
   points at `"/system/volumes/data" + str(outside.resolve() / "real")` (lower case). Skip as the helper does when the
   lower-case spelling is not the same folder (`os.path.samefile`). Assert the refusal and its message.
c. **HOME in the alias spelling.** With `HOME` set (via the test's existing HOME patching, look at how `self.home`
   is applied in `setUp`) to `"/System/Volumes/Data" + str(self.home.resolve())`, skipping when that spelling is not
   the same folder, a fence `~/real/inner` is refused before any look with the written-spelling message, `{value}`
   being `~/real/inner` and `{prefix}` `/System/Volumes/Data`. Report the exact message you get if it differs and
   stop to explain rather than change code.
d. **HOME a link into the data volume** (the Verifier's should-fix 4). Make a folder `h` in a temp dir and a link
   `hl -> "/System/Volumes/Data" + str(h.resolve())`, skip as above, set HOME to `hl`, fence `~/sec`, follow
   `~/sec/inside`. Assert the load is refused (any `CompartmentConfigError`, not `FenceLinkError`), and assert which
   message it is in a comment plus the exact text. If it loads, stop and report: do not add code for it.
e. **The `/home` case as a test of the new messages.** Build it from temp folders, never the real `/home`: reuse
   `data_volume_folder()` (it has `L -> /System/Volumes/Data<outside>/real`); fence `str(outside / "L/a/f")` gives
   the resolved message (already covered by the existing test, now with the new text), and fence
   `"/System/Volumes/Data" + str(outside.resolve() / "real/a/f")` gives the written message. One test, two subtests,
   asserting that neither message tells the user to write the other spelling: both end with
   `move the folder into your home folder` plus their tail as written above.

## 3. The test nits

- `data_volume_folder()` calls `self.skipTest` only after `mkdtemp` and `samefile`; on a system without
  `/System/Volumes/Data`, `os.path.samefile` raises `FileNotFoundError` before the skip. Wrap the check so any
  `OSError` skips.
- The backstop test (`test_backstop_refuses_look_inside_later_resolved_fence` and its `outside_backstop_fences`
  helper) keeps its own copy of that skip: make it call one shared helper (add
  `def require_data_volume_spelling(self, folder)` that skips unless the data-volume spelling of `folder` is the
  same folder, catching `OSError`), and use it from `data_volume_folder`, the backstop helper and the new tests.

## Proof (report each)

- Red first for 1 (old texts asserted under the new expectation fail) and for 2a (show the call the old test let
  through, if any; if none, say so: then 2a is a pin, and the mutation below must still go red).
- Mutations, each red then restored with `cmp`: (i) move the written-spelling check after the first `lstat` in
  `refuse_fence_links`: 2a goes red; (ii) make `mac_alias` case-sensitive (compare parts without `fold`): 2b goes red,
  and the lower-case subtests in 2a; (iii) drop check 2 (the resolved-form refusal): 2b and 2e's first subtest go red.
- Suites, with `PYTHONDONTWRITEBYTECODE=1`: `python3 tests/crew_decisions_test.py FenceLinks`,
  `python3 tests/crew_decisions_test.py Compartments`, then the whole `python3 tests/crew_decisions_test.py`.
  Report each last line, with the skip count.
- Commit message: `fence: VF35 fix-up 2, both alias refusals end in a fix that works (move the folder into home), zero looks before the refusal pinned, lower-case, HOME and data-volume-only cases tested, one shared skip (lane: V1)`
