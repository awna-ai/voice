# VF31 fix-up 5: a missing home loads, every followed link outside home's own path counts as linked, and the home walk comes after the plain fences

Fix-up 4 (`e777e9a`) still holds except where this says otherwise. Read `plans/briefs/VF31-fixup-3.md`,
`VF31-fixup-3b.md` and `VF31-fixup-4.md` first. The code is `refuse_fence_links`, `walk_resolve` and
`under_follow` in `scripts/crew_decisions.py` (reuse `absolute_path`, `under`, `under_follow`, `fold`,
`FenceRoots`, `matches_fence`; add no second resolver). The tests are class `FenceLinks` in
`tests/crew_decisions_test.py`. Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/plain-language/SKILL.md` (the messages). Public repo: invented fixtures only. **Commit
nothing.** Match the file's style: short helpers, no new module.

Fix-up 4 is red on the full suites: `crew_decisions_test.py` has 13 errors and 3 failures and
`crew_documents_test.py` 2 failures. Most are one cause: the test home (`TEST_HOME`) does not exist,
so `os.stat(home_resolved)` raises and every load fails closed with "cannot check whether the fenced
folder ~/garden-club is a link". **This time you run the full three suites, not one class** (section 6).

## 1. A missing home has no identity (the reds)

- In the home block, `os.stat(home_resolved)` raising `FileNotFoundError` or `NotADirectoryError`
  sets `home_identity = None`; any other `OSError` and `home_blocked` still fail closed.
- In `inside_home`, the identity loop runs only when `home_identity is not None`. The spelling clause
  is unchanged. No folder can be below a home that does not exist, so this loosens nothing.
- The home block's failure message names the home, not an unrelated fence (the fix-up 3 Verifier's
  "misleading cannot check" should-fix): `compartments: cannot check the home folder, so the fenced
  folders cannot be checked`. Leave every per-fence "cannot check whether the fenced folder X is a
  link" message as it is.

## 2. Every followed link not on home's own path sets `linked[i]` (Verifier must-fix 2)

Today `may_follow` sets `linked[i] = True` only for a link inside home that sits in a follow folder.
A link outside home (`<tmp>/o/al -> <home>/real`) is followed freely and leaves `linked[i]` false, so
that fence's `lstat` runs in round 1 beside a fenced link and looks inside its target.

- `may_follow(candidate)` sets `linked[i] = True` for **every** link it follows, except a link that is
  on home's own path: `candidate` is a strict ancestor of `home` (the lexical spelling) by `under`,
  that is `len(candidate.parts) < len(home.parts) and under(home, candidate)`. (`/var` above a home in
  `/var/folders/...` is the case this keeps unlinked.) Its follow/refuse decision is unchanged.
- Test: `fenced: ["~/secret", str(<tmp>/o/al/inner/deep)]`, `~/secret -> ~/real`,
  `<tmp>/o/al -> <home>/real` (an absolute target), `~/real/inner/deep/` present, `<tmp>` a fresh
  temporary folder outside home. Refused (exit 2) naming `~/secret` as a link; the in-process
  `lstat` record (as the neighbouring tests take it) holds no path at or under `<home>/real/inner`,
  either spelling (the `/var` and `/private/var` forms); and under the kernel tripwire on
  `<home>/real` with `allow_root_metadata` the command is not killed. One mutation (set `linked[i]`
  only inside home again) must go red.

## 3. The plain fences are checked before the home walk (Verifier must-fix 4)

When home itself is a link into a fenced link's target (`HOME=<tmp>/u/me`, `<tmp>/u/me -> <tmp>/data/me`,
fence `<tmp>/fz/secret -> <tmp>/data`), the home walk `lstat`s `<tmp>/data` and `<tmp>/data/me` before
the fence is found to be a link. Close it by checking the plain fences first:

- After the `..` checks and before the home block, a **pre-pass**: for each fence `i` in config order,
  `walk_resolve(lexical[i].parent, guards, may_follow=on_home_path)`, where `guards` is
  `FenceRoots` of every other fence's lexical form and `on_home_path(c)` is the strict-ancestor test of
  section 2 (it computes `home = Path(os.path.abspath(Path.home()))` by spelling only, with no
  filesystem call). An `UnfollowedLink`, a `blocked` result or an `OSError`/`ValueError` from the walk
  means "not plain": skip that fence here, with no message (the main pass decides it, as today).
- For each plain fence, `lstat(parent / lexical[i].name)` once, exactly as round 1 does today
  (missing → fine; another error → the per-fence "cannot check" failure). A link among them raises
  `FenceLinkError` before the home block, with the same messages, the same identity dedupe, the same
  first-spelling report and the same `links` list (and so the same TODO items) as today; reuse that
  code, don't copy it (one small helper both places call).
- A fence checked here is **never `lstat`ed again**: the main pass still walks it (its resolved form
  still guards the others and enters the nesting check), but both rounds skip its `lstat`. Each fence
  root keeps exactly one `lstat` per load: the existing in-process count tests must stay green
  unchanged.
- The rest (home block, walks, nesting check, rounds, the follow check of section 4) is unchanged.
- Tests (set `HOME` for the case: `patch.dict(os.environ, {"HOME": ...})` for the in-process load,
  the child's `env` for the CLI and tripwire run):
  - the case above: refused naming `<tmp>/fz/secret` as a link; no `lstat` at or under `<tmp>/data`
    in either spelling; under the tripwire on `<tmp>/data` with `allow_root_metadata` not killed. One
    mutation (move the pre-pass after the home block) must go red.
  - a plain fence that is not a link plus a fence behind a followed link: loads, and the plain
    fence's root is `lstat`ed exactly once.
  - **Pinned, refused, recorded as R1's case** (no fix): the same home, with the fence spelled through
    home (`fenced: ["~/secret"]`, `~/secret -> <tmp>/data`). It is still refused (exit 2) and the test
    says in a comment that the home walk's one look at home's own folder is R1's accepted look (a
    fenced link whose target holds another folder Voice must look at).

## 4. A follow folder inside a fence's resolved form is refused (fix-up 3 Verifier should-fix)

After the walks and the nesting check, before either round of `lstat`s: if any follow form
(`follow_forms`, both spellings) is at or under any fence's `resolved[j]` by `under`, raise
`compartments: the follow folder {value} is inside the fenced folder {fence_value}; a fenced
folder's links are never followed` (the existing message, the follow value as written). No new
filesystem call: compare spellings only. Test: `fenced: [str(<resolved home>/x)]` and
`follow: ["~/x/in"]`, so the lexical check of fix-up 3 misses (the fence spelled by the resolved home,
the follow by `~`); skip when the resolved home equals its spelling. Refused with that message; one
mutation (drop the check) red.

## 5. Should-fixes pinned by tests

- Part-wise follow matching: `follow: ["~/fl"]`, a fence reached through the link `~/flx/ln`: refused
  naming `~/flx/ln`. A `startswith`-on-strings mutant of `under_follow` must go red.
- `test_resolved_parent_cli_tripwire_never_probes_inside_fence` now gets "is reached through another
  fenced folder" instead of "cannot check whether the fenced folder … is a link". If the tripwire half
  of that test still passes (the command was not killed), change only that `assertIn` to the new
  refusal, and say so. If it is killed, stop and report.
- Every other existing test passes unchanged. If one cannot, stop and report it rather than editing it.

## 6. Run (Python 3.12 first on PATH, one process at a time; the machine is loaded)

```
PATH=~/.pyenv/versions/3.12.14/bin:$PATH python3 tests/crew_decisions_test.py
PATH=~/.pyenv/versions/3.12.14/bin:$PATH python3 tests/crew_documents_test.py
PATH=~/.pyenv/versions/3.12.14/bin:$PATH python3 tests/crew_hardening_test.py
```

All three must exit 0, with no new skip (the decisions suite has 1 skip today). One mutation per
section (copy aside, mutate, run the one test, restore with `cp`, check with `cmp`). Summarise the
diff, each red you saw, each mutation and its result, and the three suites' final counts and exit
codes.

## Amendment (after the first pass stopped at section 5)

Section 1's new home message conflicts with three existing tests that pin the old one:
`test_home_stat_error_fails_closed`, `test_home_walk_errors_fail_closed_naming_first_fence` (two
subtests) and `test_home_walk_fails_closed` (three subtests). **They are exempt:** change only their
expected message to `compartments: cannot check the home folder, so the fenced folders cannot be
checked`, and rename `test_home_walk_errors_fail_closed_naming_first_fence` to
`test_home_walk_errors_fail_closed_naming_home`. Nothing else in them changes. Section 1 as built stays.
Then do sections 2 to 5 and section 6.

The kernel tripwires cannot start inside your sandbox (`sandbox_apply` is refused there); their skips
in your runs are expected, and the Claude reviewer reruns all three suites outside it. Count a suite
as passing when every failure left is a skip of that kind, and list each skipped test by name.
