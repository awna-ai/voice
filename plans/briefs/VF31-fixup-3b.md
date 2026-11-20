# VF31 fix-up 3b: the brief's three conflicts, settled

Fix-up 3 (`plans/briefs/VF31-fixup-3.md`, your uncommitted work in this worktree) is right as far as
it goes; keep everything except what this changes. You reported three conflicts and one edge. They
are planning misses in the brief, decided here. **Commit nothing.** Skills as in fix-up 3.

## 1. No common-folder walk; each fence walks a free prefix, then a checked rest

The unguarded `base` walk reads links below home (`~/hop`, `~/via`) before any fence walk can refuse
them. Remove the `base_resolved, base_blocked = walk_resolve(base, ...)` walk. In its place, in
`refuse_fence_links`:

- Keep `base = Path(os.path.commonpath([...all fences' lexical parents...]))` inside the `try` (no
  probe; it keeps `test_commonpath_errors_fail_closed_naming_first_fence` meaningful). Also compute
  `outside_base`, the `commonpath` of the lexical parents of the fences **not** under `home` (only
  when there is at least one).
- Keep the home walk (`home_resolved`, guarded by `FenceRoots(lexical)`, fail closed when blocked)
  first.
- For each fence, in the existing shortest-first order: its **start** is `home` when
  `under(lexical[i], home)`, else `outside_base`. Resolve the start once per distinct start with
  `walk_resolve(start, FenceRoots(lexical))` and **no** predicate (the start of an under-home fence is
  `home`, already resolved). A blocked start walk, or `OSError`/`ValueError`, raises the existing
  `cannot check whether the fenced folder {values[i]} is a link` error.
- Then walk the rest:
  `walk_resolve(start_resolved.joinpath(*parent_lexical.parts[len(start.parts):]), guards, may_follow=pred)`
  where `parent_lexical = lexical[i].parent`, `guards` as today, and `pred(candidate)` is
  `under(home, candidate) or under(home_resolved, candidate) or any(under(candidate, f) for f in follow_forms)`.
  Drop the `under_home` and `base` clauses from the predicate: the free prefix now does that job,
  which also fixes the `/var/.../via` edge you found.
- `linked[i]` becomes: this fence's rest walk followed at least one link that was not at or above
  home (record it inside `pred` when it returns true through the follow clause only). Rounds use it
  exactly as before.
- `resolved[i] = parent / lexical[i].name` as before, with `parent` from the rest walk.

## 2. R1's accepted looks are two, and the test says so

In R1 (`["~/secret", "~/real/inner"]`, `~/secret -> ~/real`), the walk to `~/real/inner` looks at
`~/real` itself, then the fence `lstat` looks at `~/real/inner`. Both are accepted. Change the R1
test to assert that the `lstat` calls under `~/real` (either spelling) are exactly
`[<resolved home>/real, <resolved home>/real/inner]` in that order, still no `readlink` of
`~/secret` and no `scandir`, `open` or `stat` under `~/real`.

## 3. Keep the other strict assertions

R4 (no `readlink` of `~/hop`) and case 4's no-`lstat`-under-`~/top` must now pass as written. Add a
test for the edge you found: an outside-home fence `<tmp>/via/f` with `<tmp>/via -> <tmp>/top`
(`tmp` from `tempfile.mkdtemp()`, unresolved, so on macOS it is spelled under `/var`), and a second
fence `<tmp>/via/g`: both start at `<tmp>/via`, so it loads (or refuses only as fix-up 2 would for a
fenced link), with no `follow`. Bare `~` in `follow` stays accepted (record it in your summary).

## Run

Run only, with Python 3.12 first on PATH and at most 2 workers:
`python3 tests/crew_decisions_test.py FenceLinks`, then `python3 tests/crew_documents_test.py` and
`python3 tests/crew_hardening_test.py`. The lane runs the full suites. One mutation per change in
1 and 3 (copy aside, mutate, run, restore with `cp`, `cmp`). Summarise the diff, the reds and the
mutations.
