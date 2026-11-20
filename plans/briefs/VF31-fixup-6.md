# VF31 fix-up 6: a backstop for any look inside a fence, and links outside home followed only inside a follow folder

The owner picked V-Q31 A. Fix-up 5 (`86b3396`) still holds except where this says otherwise. Read
`plans/briefs/VF31-fixup-3.md`, `VF31-fixup-4.md` and `VF31-fixup-5.md` (with both amendments) first.
The code is `refuse_fence_links` and `walk_resolve` in `scripts/crew_decisions.py` (reuse `under`,
`under_follow`, `absolute_path`, `FenceRoots`, `matches_fence`; add no second resolver and no new module).
The tests are class `FenceLinks` in `tests/crew_decisions_test.py`. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md` (the messages). Public
repo: invented fixtures only. **Commit nothing.** Match the file's style: short helpers.

Why: Voice learns where a fence written through a link really is only by walking that path, so until every
walk is done it cannot tell that another walk went inside it. Each fix-up closed the cases found, and each
review found new ones. A closes the family by checking afterwards (section 1) and by refusing most of it
before any look (section 2).

## 1. The backstop: any look inside a fence refuses the map

- `walk_resolve` gains a keyword-only `looked=None`. When it is a list, append `candidate` to it
  immediately before `os.lstat(candidate)` (the `readlink` that may follow is of the same path, so it is
  covered). Nothing else in `walk_resolve` changes; its other callers (`fenced_forms`, `path_compartment`)
  pass nothing and behave exactly as today.
- In `refuse_fence_links`, keep one list `looked = []` and record every filesystem look the function makes,
  **except each round's own `os.lstat(resolved[i])`** (that is each fence's single check):
  - pass `looked=looked` to the home walk and to every fence's parent walk (section 2 removes the
    function's only other looks, the home `stat` and `inside_home`'s ancestor `stat`s; any look left
    besides the rounds' own `lstat`s must be recorded).
- After both rounds finish with no link found (the point where today the function returns and the map
  loads), check `looked`: if any recorded path is **at or under** any fence's `resolved[j]` or
  `lexical[j]` by `under` (that is, `under(path, root)`, equal included), raise
  `CompartmentConfigError` with
  `compartments: checking the fenced folders looked inside the fenced folder {value}, because another path leads into it through a link; write each fenced folder by its real path`
  where `{value}` is the first such fence in config order, written as configured (control characters
  shown as `?`, section 4). It is not a `FenceLinkError`: no TODO item.
- Order is unchanged otherwise: a fence that is a link still raises `FenceLinkError` from its round
  (with its TODO items), and every earlier refusal (`..`, nesting, unfollowed link, blocked walk, the
  follow-inside-fence check) still comes first. The backstop only ever turns a load into a refusal.
- Why "at or under" never refuses a list that should load: a walk or an ancestor `stat` reaches a fence's
  real place only when one path leads into another fence, and every such list is one the owner wants
  refused. Each fence root keeps exactly one `lstat` of its own; the existing in-process count tests must
  stay green unchanged.

Tests (in-process, recording `lstat`, `stat`, `readlink`, `scandir` and `open` the way
`test_outside_followed_link_defers_fence_probe` does; `<D>` a fresh `tempfile.mkdtemp()` outside home,
`follow: [str(<D>)]` so section 2 lets the walks run):

- **The security review's first high:** `fenced: [<D>/o/lnk2/z, <D>/b/deep]`, `<D>/o/lnk2 -> <D>/r2`,
  `<D>/b -> <D>/r2/z/s1/s2/s3`, `<D>/r2/z/s1/s2/s3 -> <D>/elsewhere`, every target folder present
  (`<D>/r2/z`, `<D>/elsewhere/deep`). Fix-up 5 loads it; now refused with the backstop message naming
  `<D>/o/lnk2/z`, and the CLI (`self.cli("where")` with that config) exits 2 with that message and empty
  stdout. Mutation: skip the backstop check: red.
- **The Verifier's should-fix, R2 with `follow: ["~"]` and home's second spelling:** fixture of
  `test_r2_fence_through_link_into_another_fence_refused_one_look_accepted`, with `follow: ["~"]`, once as
  written and once with the first fence spelled by the resolved home
  (`str(self.home.resolve() / "t/s/x/y")`; skip that subtest when the resolved home equals its spelling).
  Both refused (any `CompartmentConfigError`, never a load). If either loads, the backstop is wrong: stop
  and report.
- **A list that loads keeps loading:** `test_plain_fence_with_followed_fence_has_one_root_probe`,
  `test_real_missing_and_linked_parent_load` and every other test that loads today must pass unchanged
  (or as section 2 allows). Add none for this.

## 2. Links outside home are followed only inside a follow folder, except links on home's own path

Today `may_follow` refuses a link only when it is inside home and not under a follow folder; a link
outside home is followed freely.

- New decision, same `linked[i]` rule as fix-up 5:
  - `candidate` on home's own path (a strict ancestor of `home` by spelling:
    `len(candidate.parts) < len(home.parts) and under(home, candidate)`, the `/var` case), or equal to
    `home` itself (`tuple(map(fold, candidate.parts)) == tuple(map(fold, home.parts))`): followed;
    `linked[i]` is set only when it is not a strict ancestor (exactly as fix-up 5 sets it).
  - otherwise, followed only when `any(under_follow(candidate, folder) for folder in follow_forms)`
    (inside home or not), and then `linked[i] = True`;
  - otherwise `return False` (the walk raises `UnfollowedLink`, the existing message, before any
    `readlink`).
- **Remove `inside_home`, `home_identity` and the home block's `os.stat(home_resolved)`.** Under the new
  rule a link inside home and a link outside it end the same way (follow folder or refuse), so the
  identity check no longer changes any answer, and dropping it removes its `stat`s (fewer looks). Keep the
  home walk, `home_blocked` and its fail-closed message. Section 1's recording then has no `stat`s to
  record; drop those two bullets with it.
  Exempt: `test_home_stat_error_fails_closed` pins the removed `stat`; delete it and say so.
  `test_missing_home_has_no_identity`, `test_home_identity_refuses_when_spelling_misses` and
  `test_ancestor_stat_error_requires_follow` must still pass unchanged (each list is still refused or
  loaded as before, now by the follow rule); if one cannot, stop and report.
- The message stays exactly as it is (`assert_unfollowed` pins it). `follow` may name the link itself
  (`follow: ["/tmp"]` follows `/tmp`): `under_follow` already matches equal parts. Do not change it.
- The home walk (`walk_resolve(home, ...)`) still takes no `may_follow`: home's own links are free.

Tests:

- **The security review's second high:** `fenced: [<D>/real/x/y, <D>/a/lnk/x]`, `<D>/a/lnk -> <D>/real`,
  `<D>/real/x -> <D>/elsewhere` (present). No `follow`: refused through a link (`UnfollowedLink`'s
  message, the link named as the walk meets it), and no `readlink` of `<D>/real/x` in either spelling.
  Mutation: follow links outside home freely again: red.
- **The first high without `follow`:** section 1's first fixture with no `follow`: refused through a link,
  and no `lstat` at or under `<D>/r2/z` (either spelling).
- **A fence under `/tmp`:** where `/tmp` is a link (`os.path.islink("/tmp")`; skip otherwise),
  `fenced: [<a mkdtemp(dir="/tmp")>/a/f]` refuses naming the link `/tmp`; with `follow: ["/tmp"]` it
  loads.

**Exempt tests.** These pin "links outside home are followed freely", which the owner has now changed.
Change only what follows, and say for each what you changed:

- `test_two_outside_fences_stay_free`: its `/tmp` fence now needs `follow`. Rename it
  `test_two_outside_fences_load_with_follow`, add `follow: ["/tmp"]`, nothing else. (The new `/tmp` test
  above pins the refusal.)
- `test_common_link_outside_home_stays_free`: rename `test_common_link_outside_home_needs_follow`;
  first assert the list without `follow` refuses through `<root>/via` (resolved spelling, as the walk
  meets it), then add `follow: [str(root)]` and keep both of its loads.
- `test_outside_followed_link_defers_fence_probe` and `test_r4_outside_link_into_home_refused`: add the
  narrowest `follow` that keeps each test's purpose (the folder that holds the outside link, `<outside>/o`
  and `<outside>` respectively, both spellings needn't be listed: the walk meets the link by its
  resolved spelling, so write `str(outside.resolve() / ...)` if the written one doesn't match). Nothing
  else changes in them.
- Any other test that fails **only** because a link outside home (not on home's own path) is no longer
  followed without `follow`: add the narrowest `follow` entry and say so. **Any other failure: stop and
  report it; do not edit the test.**

## 3. The Verifier's must-fix M9: pin the resolved-forms guard

Nothing fails today when the fence walks' guards drop the resolved forms
(`FenceRoots([lexical others] + list(resolved.values()))` → lexical only). Add a test that kills that
mutant without a tripwire. Suggested fixture: `follow: ["~"]`, `~/alias -> ~/real`,
`fenced: ["~/alias/inner", "~/real/inner/deep"]` (`~/real/inner/deep` present). The shorter fence walks
first and resolves to `~/real/inner`; the second fence's walk must then stop at `~/real/inner`: refused
with exactly `compartments: the fenced folder ~/real/inner/deep is reached through another fenced folder;
fence only that folder`, and no `lstat` of `~/real/inner` in either spelling. With the mutant it reaches
the nesting check instead (a different message) after an `lstat` of `~/real/inner`. Run the mutant and
confirm red; if this fixture does not kill it, find one that does and say which.

## 4. Control characters in every message, and in the TODO text

The security review's low: only the unfollowed-link message replaces control characters today.

- Add one helper (`def shown(text): return "".join("?" if unicodedata.category(ch)[0] == "C" else ch for ch in text)`)
  and use it for the two existing replacements and for every config value or path put into a
  `CompartmentConfigError` or `FenceLinkError` message in `refuse_fence_links`, and for the link in the
  TODO text in `post_fence_link_todos`. The TODO dedupe compares the shown text.
- Test: a fenced link named `~/esc\x1bape` (a link to `~/real`) refuses with `~/esc?ape` in the message
  and no `\x1b`; with a `todo_command` (as the TODO tests run it) the added item text holds `?` and no
  `\x1b`.
- The `"is a link"` refusals and their TODO items are otherwise unchanged.

## 5. The R1 home-link pin uses an exact list (Verifier should-fix)

`test_r1_home_link_into_fenced_link_target_refused_one_look_accepted` asserts with `assertIn`. Make it an
exact list of the `lstat` looks under `data` (both spellings), as
`test_r1_linked_home_into_fenced_link_target_first_spelling_looks_accepted` does: record what the code
does today, assert exactly that, and say in the summary what the list is.

## 6. Run (Python 3.12 first on PATH, one process at a time; the machine is loaded)

```
PATH=~/.pyenv/versions/3.12.14/bin:$PATH python3 tests/crew_decisions_test.py
PATH=~/.pyenv/versions/3.12.14/bin:$PATH python3 tests/crew_documents_test.py
PATH=~/.pyenv/versions/3.12.14/bin:$PATH python3 tests/crew_hardening_test.py
```

The kernel tripwires cannot start inside your sandbox (`sandbox_apply` is refused there): their skips are
expected, and the Claude reviewer reruns all three suites outside it. Count a suite as passing when every
failure left is a skip of that kind, and list each skipped test by name. One mutation per section (copy
aside, mutate, run the one test, restore with `cp`, check with `cmp`). Summarise the diff, each red you
saw first, each mutation and its result, each exempt test and what changed in it, and the three suites'
final counts and exit codes.
