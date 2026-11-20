# VF35 fix-up 3: a home folder reached through a link into an alias spelling refuses the map (lane: V1)

Branch `lane-v1/VF35`, on top of fix-up 2 (`3670e10`). Code goes in `scripts/crew_decisions.py`, tests in
`tests/crew_decisions_test.py`, class `FenceLinks`. Change no other file. Skill: `~/.codex/skills/tdd-skill/SKILL.md`
(red first, a mutation per claim, restore with a `cp` backup and `cmp`, never `git checkout --`). Run Python as
`~/.pyenv/versions/3.12.14/bin/python3` with `PYTHONDONTWRITEBYTECODE=1`. No network. Never print an environment
value. Your sandbox cannot write the git index: do not commit; leave the change in the working tree and put the
commit message below in `/tmp/vf35-fixup-3-msg.txt`.

## The hole (the Verifier's should-fix 4, reproduced by V1 on `3670e10`)

With `HOME` a link `D/hl -> /System/Volumes/Data<D>/home`, `fenced: ["<D>/home/x"]` (plain, absolute) and
`follow: ["~/x/links"]`, the map **loads**. `follow_forms` adds the home-resolved form
`/System/Volumes/Data<D>/home/x/links`, and the follow-inside-fence check compares it to the plain fence with
`under`, which does not match across spellings. So a follow folder inside a fence goes unrefused. The control (HOME a
plain link to `<D>/home`) is refused today.

## The rule (decided by V1)

In `refuse_fence_links`, right after the home walk (after `home_resolved, home_blocked = walk_resolve(...)` and its
`try`/`except`, before `follow_forms` is built), add: if `mac_alias(home_resolved)` returns a prefix, raise
`CompartmentConfigError` with exactly:

`compartments: the home folder is reached through a link into {prefix}, a second name the Mac uses for other folders; Voice can only fence folders when the home folder has a path without {prefix}`

(`{prefix}` is `shown(prefix)`.) It is a plain config error, never `FenceLinkError`. It runs only when there is at
least one fence (the existing `if not lexical: return` already comes first). Nothing else changes. A home written
in an alias spelling is still refused earlier by the written-spelling checks when a `~` value uses it, as now.

## Tests (all in `FenceLinks`)

1. **Replace `test_home_link_into_data_volume_refused`** (fix-up 2's version only tested two `~` values, which the
   written check catches). Build the Verifier's case: `D = Path(tempfile.mkdtemp())` with cleanup,
   `self.require_data_volume_spelling(D)`, `(D / "home/x/links").mkdir(parents=True)`, link
   `D / "hl"` -> `"/System/Volumes/Data" + str(D.resolve() / "home")`, `fenced = [str(D / "home/x")]`,
   `follow = ["~/x/links"]`, `HOME` patched to `str(D / "hl")`. Assert `CompartmentConfigError`, not
   `FenceLinkError`, with the exact message above (prefix `/System/Volumes/Data`). Also assert, with `fence_looks`,
   that no probe call's path is at or under `D / "home/x"` or `/System/Volumes/Data` + `str(D.resolve() / "home/x")`
   (the refusal comes before any look at the fence).
2. **The control still refuses as before:** same layout but `hl -> D/home` (plain): the existing follow-inside-fence
   message, `compartments: the follow folder ~/x/links is inside the fenced folder <D>/home/x; a fenced folder's links are never followed`.
3. **A `/.nofollow` subtest of 1:** link `hl -> "/.nofollow" + str(D.resolve() / "home")`; skip when
   `os.path.samefile` on that spelling raises `OSError` or is False. Same message with prefix `/.nofollow`.
4. **No fences, no refusal:** with the alias-linked HOME of 1 and `fenced = []` (and no `follow`), the map loads.

## Proof (report each)

- Red first: test 1 and test 3 fail on `3670e10` (the map loads); show it.
- Mutations, each red then restored with `cp` and `cmp`: (i) remove the new check: 1 and 3 go red; (ii) move it
  after the follow-inside-fence loop: 1's no-look assertion or message goes red (report which; if neither goes red,
  say so and stop); (iii) run it before `if not lexical: return`: 4 goes red.
- Suites: `FenceLinks`, `Compartments`, then the whole `tests/crew_decisions_test.py`. Report each last line with
  its skip count. If any test fails that also fails on `3670e10`, name it and say so.
- Commit message for `/tmp/vf35-fixup-3-msg.txt`:
  `fence: VF35 fix-up 3, a home folder reached through a link into an alias spelling refuses the map, so a follow folder inside a fence can't hide behind it (Verifier should-fix 4; lane: V1)`
