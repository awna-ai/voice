# VF48 fix-up 3 brief: a fence written through a link hides a stored record

Card: VF48. Lane V, runner V1. You are Codex; implement this in the worktree you were started in, on top of
`HEAD`. Read `AGENTS.md`, `plans/briefs/VF48-record-folder-fence-recheck.md`, `plans/briefs/VF48-fixup-1.md` and
`plans/briefs/VF48-fixup-2.md` first. Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`. Leave every change
uncommitted (your sandbox can't write git metadata); Claude commits. Scratch only under a fresh `mktemp -d`,
removed when done.

The security review on fix-up 2 found one gap inside VF48's own claim. Fix exactly this.

## Must-fix

1. **A fence written through a link in its parent doesn't hide a stored record.** With `follow: ["~"]` and
   `~/work -> ~/projects/acme`: `record --work ~/projects/acme/secret/x.md`, then `sources mine`, then add the
   fence `~/work/secret`. `similar --compartment all --json` serves the record, and a second `sources mine`
   keeps it ("kept 1 answers"). The record-time check refuses that path, because `fenced_forms` resolves the
   fence's parent, but the read check compares only the fence's literal spellings (`fence_folder_spellings`:
   `f` and `abspath(expanduser(f))`). The same holds for a mined message whose cwd is the real path.

   Fix: `stored_fence_spellings(c)` also includes `fold(encode(root))` and `fold(fence_encode(root))` for every
   root in `fenced_forms(c, "/")` (the fence paths are already limited to `/` and `~/`, so the cwd doesn't
   matter), before the step that adds the other-home suffixes, so those suffixes cover the resolved roots too.
   A root that `fenced_forms` returns as the filesystem anchor (`/`, when resolving the parent was blocked)
   gives the spelling `-`, which fences every record: keep it, because that is the closed side and it matches
   the record-time check. Read `FenceRoots` to see how to iterate its roots; do not change `fenced_forms`,
   `walk_resolve`, `safe_resolve` or `fence_folder_spellings` (mining's transcript-folder check keeps its
   current spellings). The change reads nothing inside a fence: `fenced_forms` resolves only each fence's
   parent, as it already does at load.

## DECISIONS

In the VF48 entry of `.ai/DECISIONS.md`, add one sentence: a stored spelling is also checked against each
fence's spelling with its parent's links resolved, so a fence written through a link hides a record stored by
the real path. Keep the entry's wrap.

## Tests first (red on `HEAD`, on their assertions), then green

Python, class `Compartments` in `tests/crew_decisions_test.py`, links made in the test's own temp home:
- the repro above, through the CLI (`record`, `sources mine`, add the fence, then `similar --compartment all
  --json` serves `[]` and a second `sources mine` keeps none);
- a mined message whose cwd is `<home>/projects/acme/secret`, in the transcript folder of `~/projects/acme`,
  mined with no fence, then the fence `~/work/secret` added: `similar` serves `[]`;
- `record_compartment` on a record with `fence_folder` of the real path returns `"fenced"` under that map;
- a control: with the fence `~/work/other`, the same record is still served (no over-fencing of a sibling).

Mutation (scratch copy, `cp` back, `cmp`; never `git checkout --`): drop the `fenced_forms` spellings from
`stored_fence_spellings`. It must turn the new tests red.

## Run, at most 2 heavy processes at a time

`python3 -m unittest tests.crew_decisions_test` and the other suites under `tests/` that import
`crew_decisions` (grep for them). No Rust changes are expected. End with a short report: red reasons, mutation
result, totals.
