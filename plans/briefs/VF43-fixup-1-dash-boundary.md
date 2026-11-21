# VF43 fix-up 1 brief: a "-" that stands for a fence's last letters must end the name or meet a "-"

Card: VF43 (Persona Builder board). Lane V, runner V1. You are Codex; implement this in the worktree you were
started in. Read `AGENTS.md` first. Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`.

This branch holds VF43 (`plans/briefs/VF43-dash-for-folded-letter.md`, commit `7f3870e`): `FOLDS_TO_ASCII`,
`dashed_in`, and their use in `folder_compartment` and `record_compartment` in `scripts/crew_decisions.py`.
Keep all of it except what this brief changes. The Verifier failed it on one must-fix and three should-fixes.

## What is wrong (found by the Verifier, invented homes)

1. **Must-fix: a fence whose last name ends in a fold letter fences its stem's siblings and children.**
   Fence `~/clients` has the spelling `…-clients`. The folder of `~/client-portal` is `…-client-portal`: `client`
   matches, then its `-` stands for the last `s`, so `dashed_in` returns `True` and the folder is fenced. The same
   fences `~/client/acme` (`…-client-acme`) under `~/clients`, and `~/note-taker` under `~/notes`. Those are ordinary
   folders; only a real cwd spelled `~/clientſ` (folder `…-client-`) and its children (`…-client--acme`) should
   match.
2. **Should-fix: a hidden fence's `.` matches any `-` in a stored record's project.** `record_compartment`'s local
   `comparable` turns a leading `.` into `-` on the fence side too, so fence `~/.ssh` (project spelling `.ssh`,
   made `-ssh`) fences a record whose project is `open-ssh`.
3. **Should-fix: a match the old code made is lost.** Before VF43, fence `~/.ai` fenced a record whose project is
   `claude.ai` (`.ai` is a substring). With `comparable`, the fence side is `-ai` and no longer matches it.
4. **Should-fix: the decision text is not accurate.** The Verifier read Claude Code 2.1.311's binary: it takes the
   cwd through `realpathSync` and then `normalize("NFC")` before the replace, so the Kelvin sign most likely
   reaches the replace as `K` already; the case that really happens is a name on disk holding `ſ`, `ß` or a
   ligature. The entry also says "fence checks fail closed" without naming what still fails open: a character
   outside the BMP is two UTF-16 units, so it becomes `--`, which a single `-` never stands for, and a name cut at
   200 characters is not matched. Both are filed as VF44.

## The fix (decided; do exactly this)

1. **`dashed_in`, the end rule.** When a `-` in `name` stands for the last letters of `spelling` (that is,
   `i + len(t) == len(spelling)` for a token `t`), that is a match only if `name` ends right after that `-`, or
   the next character of `name` is `-`. A match reached through a plain character comparison is unchanged and
   still returns `True` at once, so an exact substring is still a match. Implement it in the same left-to-right
   scan: keep a boolean `pending`, set during a character's step when a `-` reaches the end of `spelling` this way
   (do not add `len(spelling)` to the reached set for it). At the start of the next character, if `pending` is
   set and the character is `-`, return `True`; otherwise clear it before the step. After the loop, return
   `pending`. Never index `spelling` at `len(spelling)`. Update the docstring in one plain sentence: a `-` that
   stands for the spelling's last letters must end the name or be followed by `-`, since `~/clientſ` and its
   children are spelled `…-client-` and `…-client--…`.
2. **`record_compartment`, the fence side stays raw.** Remove the local `comparable` helper. For each fence `f`
   and each of its two spellings (`fold(project_name(encode(f)))` and `fold(project_name(fence_encode(f)))`), the
   record is `"fenced"` when `dashed_in(spelling, project)` or `dashed_in(spelling, dotless)`, where `project` is
   `fold(record_project)` and `dotless` is `"-" + project[1:]` when `project` starts with `.`, else `project`.
   Only the record's project gets the `.`-to-`-` step (`project_name` makes a folder's leading `-` a `.`, and that
   `-` may stand for a folded letter). Compute `project` and `dotless` once, before the `any(...)`. Keep the
   existing `matches` helper and everything after the fenced check as it is.
3. Nothing else in the code changes: `folder_compartment` keeps its two `dashed_in` calls, and `FOLDS_TO_ASCII`,
   `encode`, `fence_encode`, `matches_fence`, `project_name` and the routing stay as they are.
4. **`.ai/DECISIONS.md`.** Replace the body of the entry `## 2026-11-20: a "-" in a transcript folder's name may
   stand for a folded letter (VF43)` (keep its heading) with these lines, word for word:

   ```
   Claude Code 2.1.311 resolves the cwd, normalizes it to NFC, then replaces each UTF-16 unit outside
   `[a-zA-Z0-9]` with `-` (`e.replace(/[^a-zA-Z0-9]/g, "-")`); a name over 200 characters is cut and given a hash.
   So a name on disk holding ſ, ß or a ligature leaves a lone `-` in the folder name, with no letter of it left.
   A `-` in a folder name may stand for any non-ASCII character whose fold is ASCII letters (`FOLDS_TO_ASCII`).
   A `-` that stands for a fence's last letters counts only at the end of the name or before another `-`, so
   `~/clients` still fences `~/clientſ` and its children but not `~/client-portal` or `~/client/acme`.
   On a stored record's project only the project's leading `.` may stand for `-`; a hidden fence's `.` matches a `.`.
   The accepted over-match: `~/secret-notes` fences the folder of a real `~/.ecret-notes`, whose name is identical,
   and a fence whose last name is only fold letters, such as `~/s`, fences every hidden folder in home.
   This fails closed only for those letters: a character outside the BMP becomes `--`, and a cut name is not
   matched; both are VF44. VF39's Kelvin line holds for looks; for folder names this rule covers the Kelvin sign
   too, in case it ever reaches the replace unnormalized.
   ```

## Tests first (red, then green), in `tests/crew_decisions_test.py`, class `Compartments`

Invented folders only; use the file's existing `claude_name` helper for Claude Code's spelling. Put the new tests
right after `test_folder_dash_rule_accepted_over_match`. Run each against the current code first and watch it
fail on its assertion (the first, third and fourth fail today; the second's fenced subtests already pass, and its
`not fenced` subtests fail).

- `test_folder_dash_for_last_letters_does_not_fence_the_stem`: fences `["~/garden-club", "~/clients",
  "~/notes"]`. For each of `~/client-portal`, `~/client/acme`, `~/note-taker`, and for both `encode(path)` and
  `"-System-Volumes-Data" + encode(path)`, `folder_compartment` returns the exact project value it returned before
  VF43 (read it at `d0d873a`, the branch's base, and write the literal into the test, for example
  `"p:client-portal"`); one subtest each.
- `test_folder_dash_for_last_letters_fences_the_real_folder_and_children`: same fences. For each of
  `~/clientſ`, `~/clientſ/acme`, `~/noteſ`, `~/noteſ/drafts`, and for `claude_name(path)`, its data-volume form
  and its worktree form `name + "--claude-worktrees-x"`, `folder_compartment` returns `"fenced"`.
- `test_record_hidden_fence_dot_matches_only_a_dot`: fences `["~/garden-club", "~/.ssh"]`. A record whose project
  is `open-ssh` is not fenced (assert the exact value `record_compartment` returned at `d0d873a`); a record whose
  project is `project_name(encode("~/.ssh"))` is `"fenced"`.
- `test_record_hidden_fence_keeps_a_plain_substring_match`: fences `["~/garden-club", "~/.ai"]`. A record whose
  project is `claude.ai` is `"fenced"`.
- `test_record_dash_for_last_letters`: fences `["~/garden-club", "~/clients"]`. A record whose project is
  `client-portal` is not fenced (exact value from `d0d873a`); a record whose project is
  `project_name(claude_name("~/clientſ"))` is `"fenced"`.
- Add one direct test of the helper, `test_dashed_in_end_rule`, with literal folded strings:
  `dashed_in("clients", "client-")`, `dashed_in("clients", "client--acme")` and `dashed_in("secret", "-ecret")`
  are `True`; `dashed_in("clients", "client-portal")`, `dashed_in("clients", "client-acme")` and
  `dashed_in("secret", "ecret")` are `False`; `dashed_in("clients", "clients-app")` is `True` (a plain substring).

The VF43 tests already in the file must stay green, unchanged.

Mutation checks after green, each with a `cp` backup and `cmp` to restore (never `git checkout --`):
(a) make the end rule accept any next character (add `len(spelling)` to the reached set as before) and confirm
the stem tests go red; (b) put the `.`-to-`-` step back on the fence side and confirm the `open-ssh` test goes
red; (c) compare only `dotless` (drop the raw `project` comparison) and confirm the `claude.ai` test goes red.
Restore after each and confirm `cmp`. Report each mutation's red test names.

## Run

- `~/.pyenv/versions/3.12.14/bin/python3 tests/crew_decisions_test.py Compartments FenceLinks` (green, with only
  the skips the file already had);
- the whole file once at the end with the same interpreter, and once with `python3.9` if it is on PATH;
- do not run `scripts/ci-local.sh` (Claude runs the gate). Commit nothing; Claude reviews and commits.
