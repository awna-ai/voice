# VF43 brief: a "-" in a transcript folder's name may stand for a letter a fence spelling folds from

Card: VF43 (Persona Builder board). Lane V, runner V1. You are Codex; implement this in the worktree you were
started in. Read `AGENTS.md` first. Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`.

This branch sits on VF39's (`fence_encode` and the fold-before-encode fence checks); do not undo any of it.

## The defect (reproduced at `25da5f8` by VF39's security review, invented home)

Claude Code names a session's transcript folder from its cwd with exactly this code (read by Claude in Claude
Code 2.1.311's own source, 2026-11-20): `e.replace(/[^a-zA-Z0-9]/g, "-")`, cut to 200 characters plus a hash
when longer. There is no Unicode normalization, and the replace works on UTF-16 units. So a cwd typed
`~/ſecret-notes` (U+017F, long s; APFS opens it as `~/secret-notes`) gets the folder
`-Users-<u>--ecret-notes`: the `ſ` is a lone `-`, and no letter of it survives. The same holds for the Kelvin
sign (U+212A, opened as `k`), `ß` (opened as `ss`) and the ligatures `ﬀ ﬁ ﬂ ﬃ ﬄ ﬅ ﬆ`.

With `~/secret-notes` fenced, `folder_compartment(c, "-Users-<u>--ecret-notes")` returns `p:.ecret-notes`, and
the data-volume spelling `-System-Volumes-Data-Users-<u>--ecret-notes` returns a project too, not `"fenced"`:
neither fence spelling (`fold(encode(f))`, `fold(fence_encode(f))`) is a substring, because the folder has `-`
where the fence has `s`. `cmd_mine` then reads that folder. A record that carries a `cwd` is still dropped by
`record_cwd_drop_reason` (its `matches_fence` check catches the `ſ` cwd since VF39), but a record with no `cwd`
reaches the harvest. `record_compartment` has the same gap for a stored record with no `compartment` key whose
`project` came from such a folder (`.ecret-notes`).

## The fix (decided; do exactly this)

The letter is gone from the folder name, so the check fails closed: a `-` in a folder name may stand for one
character whose fold is one of the ASCII strings APFS opens it as.

1. Next to `fence_encode` in `scripts/crew_decisions.py`, add the constant and one helper:

   ```python
   # fold(c) of every non-ASCII character c whose fold is ASCII letters: ſ→s, K (Kelvin)→k, ß and ẞ→ss,
   # ﬀ→ff, ﬁ→fi, ﬂ→fl, ﬃ→ffi, ﬄ→ffl, ﬅ and ﬆ→st (a scan of every code point; the test rescans it).
   FOLDS_TO_ASCII = frozenset({"ff", "ffi", "ffl", "fi", "fl", "k", "s", "ss", "st"})


   @functools.lru_cache(maxsize=4096)
   def dashed_in(spelling, name):
       """True when spelling occurs in name, where a "-" in name may stand for any run of spelling in
       FOLDS_TO_ASCII: Claude Code writes ſ in a cwd as a lone "-". Both arguments are already folded."""
   ```

   Add `import functools` in the import block, alphabetically. `dashed_in` is a plain left-to-right scan, no
   regex: keep a set of positions in `spelling` reached so far; for each character of `name`, add position 0,
   then from each position `i` advance to `i + 1` when `spelling[i]` equals the character, and, when the
   character is `"-"`, to `i + len(t)` for every `t` in `FOLDS_TO_ASCII` with `spelling.startswith(t, i)`.
   Return `True` as soon as `len(spelling)` is reached; an empty `spelling` is `True`. An exact substring is
   still a match, so `dashed_in` replaces the plain `in` checks below rather than adding to them.
2. `folder_compartment`'s first check becomes:

   ```python
   if any(dashed_in(fold(encode(f)), folded) or dashed_in(fold(fence_encode(f)), folded) for f in c["fenced"]):
   ```

3. `record_compartment`'s fenced check uses `dashed_in` the same way on both spellings, with the record's
   project name made comparable first: `project_name` turns a folder's leading `-` into `.`, so compare against
   `"-" + project[1:]` when the folded project starts with `.`, and against the folded project otherwise. Apply
   the same leading-`.`-to-`-` step to each fence's `project_name(...)` spelling, so the two sides agree. Write
   it as one small local helper inside `record_compartment`; do not change `project_name`.
4. Nothing else changes. `encode`, `fence_encode`, `matches_fence`, `FenceRoots`, the hub and compartment
   routing comparisons, and `record_cwd_drop_reason` stay as they are (it already calls `folder_compartment`,
   so it gains the fix for free).
5. Add a `.ai/DECISIONS.md` entry right after the VF39 one: `## 2026-11-20: a "-" in a transcript folder's name
   may stand for a folded letter (VF43)`, 4 to 7 lines, plain sentences: Claude Code's exact folder naming (no
   normalization, UTF-16 units, so `ſ` and the Kelvin sign leave a lone `-`); the rule; that it fails closed;
   the accepted over-match (a fence `~/secret-notes` also fences the folder of a real `~/.ecret-notes`, whose
   name is identical, and a fence whose last name is only fold letters, such as `~/s`, fences every hidden folder
   in home); and that the VF39 entry's Kelvin line holds for looks but not for folder names, which this covers.

## Tests first (red, then green), in `tests/crew_decisions_test.py`, class `Compartments`

Invented folders only. Run each new test against the unfixed code first and watch it fail on its assertion.
These are string and `mine` checks against the harness home; no skip is needed. Build folder names with a
local helper that mimics Claude Code exactly, `re.sub(r"[^A-Za-z0-9]", "-", str(Path(path).expanduser()))`
with no normalization (do not use `encode`, which NFC-normalizes and so turns the Kelvin sign into `K`).

- `test_folder_with_a_dash_for_a_folded_letter_is_fenced`: with `fenced` = `["~/garden-club",
  "~/secret-notes", "~/glass-jar", "~/kite-box", "~/fine-print"]`, each folder below is `"fenced"` through
  `folder_compartment`, as one subtest each: the Claude Code name of `~/ſecret-notes`, of `~/glaß-jar` (`ß`
  for `ss`), of `~/Kite-box` (Kelvin sign) and of `~/ﬁne-print` (ligature for `fi`), and
  for each the data-volume form `"-System-Volumes-Data" + name`. Also the worktree form
  `name + "--claude-worktrees-x"`.
- `test_folder_dash_rule_leaves_ordinary_folders_routed`: with the same fences, these keep their old answer
  (assert the exact value `folder_compartment` returned before the change, read from the unfixed code and
  written into the test): `encode("~/projects/work")` stays `"work"`; `encode("~/ecret-notes")` (the dash must
  stand for a letter, not for nothing) and `encode("~/xecret-notes")` stay project compartments, not
  `"fenced"`.
- `test_folder_dash_rule_accepted_over_match`: `encode("~/.ecret-notes")` is `"fenced"` (it is the same name as
  the `ſ` folder); this pins the decision's accepted over-match.
- `test_mine_skips_a_no_cwd_record_in_a_dash_spelled_fence_folder`: model it on
  `test_folded_letter_fence_mine_skips_plain_encoded_folder`. Fence `~/secret-notes`; one session in the Claude
  Code folder of `~/ſecret-notes` and one in its data-volume form, each holding one `said(...)` record with its
  `cwd` deleted. Patch `read_session` to raise, run `cmd_mine` in process, then run `mine` through `self.h.crew`:
  exit 0, `2 fenced folders` in stdout, and an empty `out.jsonl`.
- `test_record_project_with_a_dash_for_a_folded_letter_is_fenced`: `record_compartment(c, {"project":
  project_name(<Claude Code name of ~/ſecret-notes>)})` is `"fenced"` with fence `~/secret-notes`, and a record
  whose project is `project_name(encode("~/projects/work"))` still gives `"work"`.
- `test_folds_to_ascii_covers_every_code_point`: scan every code point from `0x80` to `0x10FFFF`, collect
  `fold(chr(cp))` where it is non-empty, ASCII and `isalnum()`, and assert that set is a subset of
  `FOLDS_TO_ASCII` (a newer Unicode table may add one; the test then names it). It takes several seconds; that
  is accepted.

Mutation checks after green, each with a `cp` backup and `cmp` to restore (never `git checkout --`): make
`dashed_in` return `spelling in name` and confirm the fenced-folder, mine and record tests go red; drop `"s"`
from `FOLDS_TO_ASCII` and confirm the `ſ` subtests and the code-point test go red. Restore and confirm `cmp`.

## Run

- `~/.pyenv/versions/3.12.14/bin/python3 tests/crew_decisions_test.py Compartments FenceLinks` (green, the
  skips the file already had and no new ones);
- the whole file once at the end with the same interpreter, and once with `python3.9` if it is on PATH;
- do not run `scripts/ci-local.sh` (Claude runs the gate). Commit nothing; Claude reviews and commits.
