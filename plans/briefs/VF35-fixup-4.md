# VF35 fix-up 4: the `/.resolve` message names the whole alias start (lane: V1)

Branch `lane-v1/VF35`, on top of `ae03979`. Code in `scripts/crew_decisions.py`, tests in
`tests/crew_decisions_test.py`. Change no other file. Skill: `~/.codex/skills/tdd-skill/SKILL.md`. Run Python as
`~/.pyenv/versions/3.12.14/bin/python3` with `PYTHONDONTWRITEBYTECODE=1`. No network. Do not commit (your sandbox
cannot write the git index); write the commit message below to `/tmp/vf35-fixup-4-msg.txt`.

## The finding (the repeat Verifier's should-fix, measured on `ae03979`)

A fence written `/.resolve/1/<path>` is refused with "starts with /.resolve ... write its path without /.resolve".
Doing that literally gives `/1/<path>`, which loads and fences nothing. The message must name the whole alias start.

## The change

In `mac_alias(path)`: when `path.parts[1]` folded (`fold`) equals `.resolve` and the path has a `parts[2]`, return
`"/" + path.parts[1] + "/" + path.parts[2]` (as written, e.g. `/.resolve/1`). Every other case is unchanged. Nothing
else in `refuse_fence_links` changes: its messages already print what `mac_alias` returns.

## Tests

- In `test_data_volume_spelling_refused_before_any_look`, the `/.resolve/1` subtest's expected prefix becomes
  `/.resolve/1`. Keep its zero-probe assertion.
- A unit test of `mac_alias` alone: `/.resolve/1/a/b` gives `/.resolve/1`; `/.RESOLVE/2/a` gives `/.RESOLVE/2`;
  bare `/.resolve` gives `/.resolve`; `/.nofollow/a` gives `/.nofollow`; `/System/Volumes/Data/a` gives
  `/System/Volumes/Data`; `/Users/a` gives None.

## Proof (report each)

- Red first: the changed subtest and the new unit test fail on `ae03979`.
- Mutation: drop the new branch: red; restore with `cp` and `cmp`.
- `python3 tests/crew_decisions_test.py FenceLinks`: report the last line with its skip count.
- Commit message: `fence: VF35 fix-up 4, a /.resolve/<n> spelling is named whole in the refusal, so following the message cannot leave /<n>/<path>, which fences nothing (repeat Verifier should-fix; lane: V1)`
