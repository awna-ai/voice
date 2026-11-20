# VF39 fix-up 2 brief: pin the fence side of the fold, and name what the fold accepts

Card: VF39. Lane V, runner V1. You are Codex; work in this worktree (branch `lane-v1/VF39`, head `25da5f8`). Read
`plans/briefs/VF39-fold-before-encode.md` and `plans/briefs/VF39-fixup-1.md` first. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`.

## Why (the repeat Verifier's should-fix, PASS otherwise)

No test pins the fence side of the fold. With `FenceRoots` (`scripts/crew_decisions.py`, the
`value.comparisons = …` line) reverted to `fold(encode(root))`, Compartments and FenceLinks still pass, yet that
mutant unfences a look `origin/main` fences: fence `~/straße`, look `/System/Volumes/Data<home>/straße/f`. Both
existing data-volume tests use ASCII fences.

## The change (decided; do exactly this)

1. **Tests only, no code change.** In `test_long_s_look_in_data_volume_spelling_is_fenced` and
   `test_where_long_s_look_in_data_volume_spelling_is_fenced`, add two pairs to each `(name, spelling)` tuple,
   after the two that are there: `("straße-notes", "straße-notes")` (a non-ASCII fence, looked at in its own
   spelling) and `("glaß-jar", "GLASS-JAR")` (a non-ASCII fence, looked at in a capital ASCII spelling). Keep
   using `folded_data_volume_fence`, which already skips when the temporary folder has no such spelling.
2. **Mutation check, and report it:** back up `scripts/crew_decisions.py` with `cp`, change the `FenceRoots` line
   to `fold(encode(root))`, run the two tests and see at least one new subtest fail, restore with `cp`, and confirm
   with `cmp` that the file is unchanged.
3. **`.ai/DECISIONS.md`, the VF39 entry at the end** (keep its heading and every line already there). Add these
   two sentences after "every check fails closed." (re-wrap the paragraph, lines at most 115 characters):
   "The folded half also fences folders and projects that hold the folded letters (a fence `~/straße` also fences
   `strasse-photos`); that over-match is accepted, as the plain half's substring match already over-matches.
   A capital ASCII letter with a combining mark (`J̌` for `ǰ`) still slips past the data-volume check: VF42."
   Leave the VF40 entry above it exactly as it is.

## Run

`python3 tests/crew_decisions_test.py Compartments FenceLinks` from the worktree root, green. Do not run the whole
file or `scripts/ci-local.sh` (Claude runs the gate).

## Never

Real names; any file besides `tests/crew_decisions_test.py` and `.ai/DECISIONS.md`; changing
`scripts/crew_decisions.py`; committing.
