# VF31 fix-up 3: a member's `follow` list (the folders whose links Voice may follow)

Read `plans/briefs/VF31-fence-link-refusal.md`, `plans/briefs/VF31-fixup-1.md`,
`plans/briefs/VF31-fixup-2.md` and `plans/briefs/VF31-follow-list-design.md` first; everything there
still holds except where this says otherwise. The owner picked the follow list (V-Q30, choice 1 A):
it closes R3 and R4, and R1 and R2 are recorded and pinned by tests as "refused, one look accepted".
Rule (b) is unchanged: a fence that is itself a link refuses the map, and Voice never reads that link.

The code is `check_compartments`, `walk_resolve` and `refuse_fence_links` in
`scripts/crew_decisions.py` (reuse `absolute_path`, `under`, `fold`, `FenceRoots` and
`matches_fence`; add no second resolver). The tests are class `FenceLinks` in
`tests/crew_decisions_test.py`. Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/plain-language/SKILL.md` (the error message). Public repo: invented fixtures only,
no real names or paths. **Commit nothing.** Match the file's style: short helpers, no new module.

Test first for each rule (red on its assertion, then green), and a mutation per rule (copy the file
aside, mutate, run the test, restore with `cp`, check with `cmp`). Report every red run and every
mutation in your summary.

## 1. The config key

- The compartment map gains an optional top-level `"follow"`: a list of path strings, each validated
  by the same `path()` rule as `fenced` (starts with `/` or `~/`, no NUL or newline). Add `"follow"`
  to the allowed keys in `check_compartments`. A non-list is `compartments: invalid follow`.
- In `refuse_fence_links`, before any filesystem call (beside the existing `..` check, and before it
  in the same loop style):
  - a follow folder with a `..` part raises
    `CompartmentConfigError(f'compartments: the follow folder {value} has a ".." part; write it without ".."')`;
  - a follow folder whose lexical form (`Path(os.path.abspath(absolute_path(value, Path.cwd())))`) is
    at or under any fence's lexical form (`under(follow, fence)`, folded, as `under` does) raises
    `CompartmentConfigError(f"compartments: the follow folder {value} is inside the fenced folder {fence_value}; a fenced folder's links are never followed")`.
  - A fence inside a follow folder is allowed and normal (a shared projects folder holds fenced
    projects).
- Voice never `lstat`s, `stat`s or reads a follow folder to check it. It is compared by spelling
  only (section 2).

## 2. Which links a fence's walk may follow

Add an optional keyword `may_follow=None` to `walk_resolve`. When it is `None`, behaviour is exactly
today's (every other caller, `fenced_forms` included, is unchanged). When it is a callable, the walk
calls `may_follow(candidate)` **after** the `lstat` that shows `candidate` is a link and **before**
`os.readlink`; if it returns false, the walk raises `UnfollowedLink(candidate)` (a new small
exception class beside `FenceLinkError`, not a `CompartmentConfigError`; `refuse_fence_links`
converts it, below). The fence check before the `lstat` stays first, as today.

In `refuse_fence_links`, build the predicate once, with no filesystem call beyond what the walks
already make:

- `home` = `Path(os.path.abspath(Path.home()))`, and `home_resolved` from
  `walk_resolve(home, FenceRoots(lexical))` (no `may_follow`). If that walk is blocked or raises
  `OSError`/`ValueError`, raise the existing
  `cannot check whether the fenced folder {values[0]} is a link` error.
- A link is **followed freely** when the candidate is at or above home in either spelling
  (`under(home, candidate)` or `under(home_resolved, candidate)`): these are the links on the home
  folder's own path, such as `/var` or `/tmp` on macOS.
- For a fence whose lexical form is **not** under `home`, a link is also followed freely when it is
  at or above the fences' common folder (`base` or `base_resolved`, exactly as fix-up 2 computes
  them). For a fence under `home`, the common folder gives no freedom: every link strictly below
  home needs a follow folder.
- Otherwise a link is followed only when it sits at or below a follow folder: `under(candidate, f)`
  for any spelling `f` of a follow folder. A follow folder's spellings are its lexical form and, when
  that form is under `home`, the same path with the `home` prefix replaced by `home_resolved` (a
  string rewrite, no probe). Build a tuple of spellings once.
- Every per-fence parent walk in step 1 of fix-up 2 passes this predicate (the fence's own
  "under home" flag decides the common-folder clause). The `base` walk itself keeps no predicate.
- `UnfollowedLink` raised by a fence's walk becomes
  `CompartmentConfigError(f'compartments: the fenced folder {value} is reached through the link {shown}, which is not in a "follow" folder; add the folder that holds the link to "follow", or write the fence by its real path')`,
  where `shown` is the link's path with a leading `home_resolved` or `home` written as `~`. It is a
  plain `CompartmentConfigError`: no TODO item (the TODO tool is only for a fence that is a link).
  The walks all run before any fence `lstat` (as fix-up 2 orders them), so this refusal happens
  before any fence is looked at.
- Keep fix-up 2's round ordering (`linked[i]`, two rounds of `lstat`) as it is.

## 3. Docstring and decision

- The module docstring's line after "A fenced folder that is itself a link refuses the whole
  compartment map (exit 2)." gains one sentence: `"follow"` lists the folders whose links Voice may
  follow on a fenced folder's path; any other link below the home folder stops the load before Voice
  reads it.
- Do not edit `.ai/**`; the lane writes the decision.

## 4. Tests (class `FenceLinks`, invented folders under the test home)

1. **Validation:** `follow` not a list; an entry not starting `/` or `~/`; `~/a/../b`; a follow
   folder equal to a fence; one inside a fence. Each raises `CompartmentConfigError` with its message
   and makes **no** `lstat` call (patch `os.lstat` with `wraps`, assert not called).
2. **R3 closed:** `fenced: ["~/secret", "~/alias/inner/deep"]`, `~/alias` and `~/secret` both links
   to `~/real`, `~/real/inner` a real folder, no `follow`. The load raises the unfollowed-link
   message naming `~/alias`; `os.readlink` is never called on `~/alias` (either spelling), and no
   `lstat` touches anything under `~/real` (either spelling). Same config with `follow: ["~"]`: still
   refused (exit 2 class, any message), as fix-up 2 behaves.
3. **R4 closed (a single fence through a link under home):** `fenced: ["~/hop/f"]`, `~/hop` a link to
   `~/elsewhere` (a real folder holding `f`). With no `follow`: refused naming `~/hop`, no
   `readlink` of `~/hop`. With `follow: ["~"]`, and again with `follow: ["~/hop"]`: it loads.
4. **The old linked-parent cases now need `follow`:** in
   `test_real_missing_and_linked_parent_load`, the `~/links/secret` case gains `follow: ["~"]` (it
   still loads); add the same case without `follow`, refused naming `~/links`. In
   `test_common_link_above_fences_keeps_both_in_first_round`, both configs gain `follow: ["~"]` and
   keep their assertions; add a case of `["~/via/a", "~/via/b"]` without `follow`, refused naming
   `~/via` with no `lstat` of anything under `~/top`.
5. **System links stay free:** the test home sits under the temp folder (on macOS under `/var`, a
   link). A fence under home with no link of its own (`["~/real"]`) loads with no `follow`. Add a
   fence outside home: an invented folder created under `tempfile.mkdtemp()` (resolve nothing),
   fenced by that spelling: it loads with no `follow`.
6. **R1 pinned, "refused, one look accepted":** `fenced: ["~/secret", "~/real/inner"]`,
   `~/secret -> ~/real`. Refused with `FenceLinkError` naming `~/secret`; the only `lstat` under
   `~/real` (either spelling) is exactly one, of `real/inner`; no `readlink` of `~/secret`; no
   `scandir`, `open` or `stat` under `~/real`. Name it `test_r1_fenced_link_and_fence_inside_its_target_refused_one_look_accepted`.
7. **R2 pinned, "refused, one look accepted":** `fenced: ["~/t/s/x/y", "~/p/q/al/s"]`, `~/p/q/al ->
   ~/t`, `~/t/s/x` real. With no `follow`: refused naming `~/p/q/al` (the unfollowed-link message),
   with no `readlink` of `al`; the looks under `~/t/s` are only the walk's `lstat`s of `~/t/s` and
   `~/t/s/x` (assert that exact list, either spelling). Name it `test_r2_fence_through_link_into_another_fence_refused_one_look_accepted`.
8. **A link inside a follow folder that points elsewhere:** `follow: ["~/projects"]`,
   `~/projects/shared -> ~/wiki` (real), `fenced: ["~/projects/shared/f", "~/other/g"]`: loads; the
   walk reads `~/projects/shared`. Then a link inside `~/wiki` on a fence path (`~/wiki/l -> ~/w2`,
   fence `~/projects/shared/l/h`): refused, since `~/wiki` is not a follow folder.

Run, with Python 3.12 first on PATH as the gate does:
`python3 tests/crew_decisions_test.py`, `python3 tests/crew_documents_test.py`,
`python3 tests/crew_hardening_test.py` (all green, the kernel tripwires run, not skipped), then
`python3 -m pyflakes scripts/crew_decisions.py tests/crew_decisions_test.py` if pyflakes is installed.
Summarise the diff, the reds, the mutations, and anything you could not do.
