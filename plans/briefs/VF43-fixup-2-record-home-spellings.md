# VF43 fix-up 2 brief: a hidden fence matches a stored record whose project kept a home

Card: VF43 (Persona Builder board). Lane V, runner V1. You are Codex; implement this in the worktree you were
started in. Read `AGENTS.md` first. Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`.

This branch holds VF43 (`plans/briefs/VF43-dash-for-folded-letter.md`, commit `7f3870e`) and its fix-up 1
(`plans/briefs/VF43-fixup-1-dash-boundary.md`, commit `183df4f`). Keep all of it except what this brief changes.
The repeat Verifier passed `183df4f` with three should-fixes, and the security review found the first one too.

## What is wrong (found by the reviews, invented homes)

1. **A hidden fence misses a stored record whose project kept a home.** `project_name` strips this Mac's home, or
   `-Users-<name>-` / `-home-<name>-` of another Mac, from a folder name. It cannot strip a home when the folder
   name starts with the data volume (`-System-Volumes-Data-Users-sam--ssh`), or when the home itself was typed
   with a fold letter (`/Users/ſam/.ssh` is named `-Users--am--ssh`, which the other-Mac pattern does not match).
   So the stored project is `.System-Volumes-Data-Users-sam--ssh` or `.Users--am--ssh`. Fence `~/.ssh` has the
   spellings `.ssh` (both `encode` and `fence_encode`), and neither occurs in those projects, so
   `record_compartment` routes them as `p:…` instead of `"fenced"`. (Mining is safe: `folder_compartment` fences
   those folders. Stored records are what this fixes.)
2. **The decision entry does not name the end rule's over-match.** Fence `~/projects/clients` fences the
   worktree folders (`…-client--claude-worktrees-x`) and hidden children (`~/projects/client/.git`, folder
   `…-client--git`) of a sibling `~/projects/client`, because those names equal the ones `~/projects/clientſ/…`
   gives. Measured on this branch: `~/projects/client` itself routes to its compartment, its `.git` child and its
   worktree are `"fenced"`.
3. **The decision entry's `~/s` line is stale.** It says `~/s` fences every hidden folder in home; since
   fix-up 1 it does not. Measured on this branch with `HOME=/home/fixture` and fence `~/s`: `~/.foo`,
   `~/.config/x` and `~/projects/.hidden` are not fenced; `~/ſ`, `~/ſ/x`, `~/-` and `~/-/x` are; and a stored
   record whose project holds `--` (`a--b`) is fenced.

## The fix (decided; do exactly this)

1. **`record_compartment` in `scripts/crew_decisions.py`.** Keep `folded` and `dotless` as they are. Build the
   list of spellings to try for each fence `f` from its two folded encodings, `full_plain = fold(encode(f))` and
   `full_folded = fold(fence_encode(f))`:
   - the two project spellings already used: `fold(project_name(...))` of each encoding (unchanged);
   - the two full spellings themselves, `full_plain` and `full_folded` (they start with `-` and keep the home);
   - for each project spelling that starts with `.` (a hidden fence), that spelling with its leading `.` written
     as `--`, that is `"--" + spelling[1:]` (`~/.ssh` gives `--ssh`). A project spelling that does not start with
     `.` gets no such variant.

   The record is `"fenced"` when any of those spellings satisfies `dashed_in(spelling, folded)` or
   `dashed_in(spelling, dotless)`. Write the spellings as a small local generator or helper inside
   `record_compartment` (for example `def spellings(f): …` yielding each one), so the `any(...)` stays one
   readable expression; do not add a module-level function. `dashed_in`, `folder_compartment`,
   `FOLDS_TO_ASCII`, `encode`, `fence_encode`, `project_name`, `matches` and everything after the fenced check
   stay exactly as they are.
2. **`.ai/DECISIONS.md`.** In the entry `## 2026-11-20: a "-" in a transcript folder's name may stand for a folded
   letter (VF43)`, make exactly these two replacements and touch no other line.

   Replace this line:

   ```
   On a stored record's project only the project's leading `.` may stand for `-`; a hidden fence's `.` matches a `.`.
   ```

   with these lines, word for word:

   ```
   On a stored record's project only the project's leading `.` may stand for `-`; a hidden fence's `.` matches a `.`,
   or `--`, which is how the fence reads after a home that `project_name` could not strip (a data-volume folder,
   `.System-Volumes-Data-Users-<u>--ssh`, or a home typed with ſ, `.Users--am--ssh`). A fence's full spelling, its
   home included, is matched against the project too. So `~/.ssh` also fences a record of a hidden `.ssh` below
   any other folder (`code--ssh`); that over-match is accepted.
   ```

   Replace these two lines:

   ```
   The accepted over-match: `~/secret-notes` fences the folder of a real `~/.ecret-notes`, whose name is identical,
   and a fence whose last name is only fold letters, such as `~/s`, fences every hidden folder in home.
   ```

   with these lines, word for word:

   ```
   The accepted over-match: `~/secret-notes` fences the folder of a real `~/.ecret-notes`, whose name is identical.
   `~/projects/clients` fences the worktrees (`…-client--claude-worktrees-…`) and hidden children
   (`~/projects/client/.git`) of `~/projects/client`, whose folder names equal those of `~/projects/clientſ/…`.
   A fence whose last name is only fold letters, such as `~/s`, fences what is spelled like `~/ſ` and its children
   (a real `~/-` too) and every stored record whose project holds `--`; it no longer fences every hidden folder.
   ```

## Tests first (in `tests/crew_decisions_test.py`, class `Compartments`)

Write these, run them, and see each new assertion that expects `"fenced"` fail on the current code before you
change `record_compartment`. Use only invented homes; build projects with `self.c.project_name(...)` of a folder
name as the existing record tests do, and `claude_name` (already in the file) for a path holding ſ. Patch the home
with `patch.dict(os.environ, {"HOME": "/Users/sam"})` around both `check_compartments` and the calls, as
`test_folder_dash_for_last_letters_does_not_fence_the_stem` does with `/home/fixture`.

1. `test_record_hidden_fence_matches_a_project_that_kept_its_home`: fences `["~/garden-club", "~/.ssh"]`. Each of
   these is `"fenced"`, one `subTest` per project:
   - data volume, this Mac: `project_name("-System-Volumes-Data" + encode("~/.ssh"))`;
   - home typed with ſ: `project_name(claude_name("/Users/ſam/.ssh"))`;
   - data volume, another Mac: `project_name("-System-Volumes-Data-Users-smith--ssh")`;
   - the accepted over-match, a hidden `.ssh` below another folder: `project_name(encode("~/code/.ssh"))`.
2. `test_record_hidden_fence_still_leaves_lookalikes_routed`: same fences and home. These are not fenced and
   route as `p:<project>`: `open-ssh`, `ssh`, and `project_name(encode("~/projects/openssh"))`. Keep the existing
   `test_record_hidden_fence_dot_matches_only_a_dot` and `test_record_hidden_fence_keeps_a_plain_substring_match`
   unchanged; they must stay green.
3. `test_record_full_fence_spelling_matches_a_data_volume_project`: fences `["~/garden-club"]`, same home.
   `project_name("-System-Volumes-Data" + encode("~/garden-club"))` is `"fenced"` (it already is through the
   plain spelling; this pins it), and `project_name(encode("~/projects/work"))` still routes to `"work"`.
4. `test_record_fold_letter_fence_matches_a_double_dash_project`: fences `["~/garden-club", "~/s"]`, same home.
   A record with project `a--b` is `"fenced"` and one with project `.foo` routes as `p:.foo` (pins the decision's
   `~/s` line).

Then a mutation check of your own: in a scratch copy of `scripts/crew_decisions.py` (copy it with `cp`, compare
with `cmp`, never `git checkout --`), drop the `--` variant and confirm test 1's other-Mac case goes red; drop the
full spellings and confirm the ſ-home case still passes through the `--` variant or goes red (report which).
Restore the file and confirm `cmp` shows it unchanged.

## Run

Run the `Compartments` and `FenceLinks` classes, then the whole file, with Python 3:
`python3 -m unittest tests.crew_decisions_test.Compartments tests.crew_decisions_test.FenceLinks` and
`python3 tests/crew_decisions_test.py`. Report the counts. The machine is loaded: subprocess tests that hit their
60-second timeout and pass when run alone are load, not regressions; say which they were. Do not run
`scripts/ci-local.sh` (Claude runs the gate).

## Commit

One commit, message in a file (`git commit -F`), subject:
`fence: VF43 fix-up 2, a hidden fence matches a stored record whose project kept a home (its "." as "--", and the full spelling), and the decision names the end rule's over-match and the current ~/s reach (repeat Verifier should-fixes; lane: V1)`.
Commit this brief in a separate commit first: `plans: VF43 fix-up 2 brief, a hidden fence matches a stored record whose project kept a home, and the decision's over-match lines (repeat Verifier should-fixes; lane: V1)`.
Do not push. No real person's name, handle or path anywhere.
