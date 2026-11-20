# VF35: a fence or follow folder in the Mac's data-volume spelling is refused

Card VF35 (`2xLWsw1K`). This builds on VF31 (branch `lane-v1/VF31`, head `34c4e1a`), which is landing now. Read
`plans/briefs/VF31-fixup-6.md` and `plans/briefs/VF31-fixup-7.md` for how `refuse_fence_links` works today. The
code is `refuse_fence_links` in `scripts/crew_decisions.py`. The tests go in class `FenceLinks` in
`tests/crew_decisions_test.py`. Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`. This is a public repo:
use invented fixtures only. **Commit nothing.** Leave `.ai/**` alone.

## The problem

On macOS `/System/Volumes/Data/<path>` is the same folder as `<path>` (a firmlink, not a symlink, so the walk
never turns one into the other). `under`, `matches_fence` and `fenced_forms` compare spellings. `matches_fence`
also matches the encoded spelling as a substring, so a **plain** fence (`/Users/u/f`) catches a look in the
data-volume spelling (`/System/Volumes/Data/Users/u/f/x`), but never the reverse. So a fence that is written in the
data-volume spelling, or that resolves into it through a link, is fenced after the load only by that spelling.
Reads by the plain spelling are then not fenced, and VF31's backstop misses looks inside it.

## The decision (made at pickup; do not change it)

Refuse it, failing closed, the way VF31 refuses a fence that is a link. Never compare the two spellings. Add a
module constant next to `refuse_fence_links`:

```python
DATA_VOLUME = Path("/System/Volumes/Data")
```

Every check below uses `under(path, DATA_VOLUME)`. It folds case, so `/system/volumes/data/...` is refused too.
The checks are by spelling only and run on every platform. None of them looks at the disk.

### 1. Written in that spelling: refused before any look

- In the first loop of `refuse_fence_links` (over `c.get("follow", [])`), right after the `".."` check, compute
  `follow` as it does now (`Path(os.path.abspath(absolute_path(value, Path.cwd())))`), then:
  if `under(follow, DATA_VOLUME)`, raise `CompartmentConfigError` with exactly
  `f"compartments: the follow folder {shown(value)} starts with /System/Volumes/Data, the Mac's second name for the same folder; write it without /System/Volumes/Data"`.
  This check goes **before** the existing "follow folder inside a fenced folder" loop.
- In the second loop (over `c["fenced"]`), right after its `".."` check, compute
  `Path(os.path.abspath(absolute_path(value, Path.cwd())))`; if it is `under` `DATA_VOLUME`, raise
  `CompartmentConfigError` with exactly
  `f"compartments: the fenced folder {shown(value)} starts with /System/Volumes/Data, the Mac's second name for the same folder; write it without /System/Volumes/Data"`.
- Both checks come before the home walk, so the refusal looks at nothing on disk.

### 2. Reached through a link into that spelling: refused right after the walk

- Right after `resolved[i] = parent / lexical[i].name`, add: if `under(resolved[i], DATA_VOLUME)`, raise
  `CompartmentConfigError` with exactly
  `f"compartments: the fenced folder {shown(values[i])} is reached through a link into /System/Volumes/Data, the Mac's second name for the same folder; write the fence by its path without /System/Volumes/Data"`.
- It is a `CompartmentConfigError`, never a `FenceLinkError`, so no TODO item is added.
- Nothing else in `refuse_fence_links`, `fenced_forms`, `matches_fence`, the backstop or `walk_resolve` changes.

## Tests (red first, then green)

Put all three in `FenceLinks`. For the macOS tests, use the same skip as VF31 fix-up 7's data-volume test:
skip unless `Path("/System/Volumes/Data").is_dir()` and
`os.path.samefile("/System/Volumes/Data" + str(D.resolve()), D.resolve())`, where `D` is a fresh
`Path(tempfile.mkdtemp())` outside home, removed with `self.addCleanup(shutil.rmtree, D)`. Make the skip a small helper on
the class if two tests share it.

1. `test_data_volume_spelling_refused_before_any_look` (every platform). Three `subTest`s, each asserting the
   exact message, that the exception is **not** a `FenceLinkError`, and, with `self.fence_looks(stack)` wrapped
   around the load, that no `lstat`, `stat`, `readlink`, `scandir` or `open` call had a path `under`
   `DATA_VOLUME` (convert each `call.args[0]` with `Path(...)`, skipping non-path arguments such as ints):
   - `fenced: ["/System/Volumes/Data" + str(self.home / "real/inner")]` gives the fenced message;
   - `fenced: ["/system/volumes/data" + str(self.home / "real/inner")]` gives the fenced message, naming the
     value as written (lower case);
   - `fenced: ["~/real/inner"]` with `follow: ["/System/Volumes/Data" + str(self.home)]` gives the follow message.

   Mutations: remove check 1's fenced branch, and the first two subtests go red. Remove the follow branch, and
   the third goes red.
2. `test_fence_through_link_into_data_volume_refused` (macOS skip). `D/real/a/f` present,
   `D/L -> "/System/Volumes/Data" + str(D.resolve() / "real")` (a directory link),
   `follow: [str(D.resolve())]`, `fenced: [str(D / "L/a/f")]`. Assert the exact "reached through a link"
   message for `str(D / "L/a/f")`, and that the exception is not a `FenceLinkError`. Mutation: remove check 2,
   and it loads (red).
3. `test_plain_fence_still_fences_data_volume_spelling` (macOS skip). This proves check 2's advice works. Use the same
   `D` and link, `follow: [str(D.resolve())]`, and `fenced: [str(D.resolve() / "real/a/f")]`: it loads. With
   `c = member.compartments`, each of these reports fenced (`self.c.safe_resolve(c, p, os.getcwd())[1]` is
   `True`): `D / "L/a/f/notes.md"`, `D.resolve() / "real/a/f/notes.md"`, and
   `Path("/System/Volumes/Data" + str(D.resolve() / "real/a/f/notes.md"))`. Mutation: in `matches_fence`, drop
   `or spelling in encoded`, and this test goes red. Run only this test for that mutation, since others depend on it too.

## Run

`PATH=~/.pyenv/versions/3.12.14/bin:$PATH python3 tests/crew_decisions_test.py FenceLinks` and
`... crew_decisions_test.py Compartments`, plus the three new tests by name. Do one mutation per check as listed:
copy the file aside, mutate, run the one test, restore with `cp`, and confirm with `cmp`. Never use
`git checkout --`. Skip the full suites: the Claude reviewer runs them outside your sandbox. If any other test
fails, stop and report it. Summarise the diff, each red you saw first, each mutation and its result, and the counts.

## Amendment 1 (after the first run): keep VF31 fix-up 7's backstop proof reachable

The first run found `FenceLinks.test_backstop_refuses_firmlink_spelling_inside_fence` red. Its link
`fol/al -> /System/Volumes/Data<D>/real/f1/sub` ends in the data-volume spelling, so check 2 now refuses it first.
That is right for that fixture. But the backstop's data-volume match is still needed for a link that enters the
data-volume spelling inside a fence and then climbs back out with `..` to a plain folder, because that fence's
resolved path is plain and check 2 lets it through. Change that one test so it proves this case:

- Add `(outside / "out").mkdir()`.
- Make the `fol/al` link's target the string
  `"/System/Volumes/Data" + str(outside.resolve() / "real/f1/sub") + "/" + "../" * n + str(outside.resolve() / "out").lstrip("/")`,
  where `n = len(Path("/System/Volumes/Data" + str(outside.resolve() / "real/f1/sub")).parts)`. Extra `..` at `/`
  stay at `/`.
- Everything else in the test stays as it is: same `follow`, same `fenced`, the same expected backstop message
  naming `self.config["fenced"][1]`, and not a `FenceLinkError`.
- Mutation: change the backstop back to `under`
  (`any(under(path, root) for root in (resolved[j], lexical[j]) for path in looked)`). The test must go red
  (it loads), then restore it. Also confirm that with check 2 removed this test still passes, which shows it no
  longer depends on check 2.

Then do the brief's mutations and runs as written (FenceLinks, Compartments, the three new tests by name).
