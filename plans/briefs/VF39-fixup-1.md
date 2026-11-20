# VF39 fix-up 1 brief: folder and record fence checks match both spellings

Card: VF39. Lane V, runner V1. You are Codex; work in this worktree (branch `lane-v1/VF39`, head `891f968`). Read
`plans/briefs/VF39-fold-before-encode.md` first. Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`.

## The defect (the Verifier and the security review, both must-fix; reproduced by Claude on `891f968`)

`folder_compartment` and `record_compartment` compare a fence against a Claude Code project folder name, and that
name is built by plain `encode` (it turns `ß`, `ſ`, ligatures into `-`). The first brief made the fence side
`fence_encode` only, so a fence written with one of those letters no longer matches its own folder:
fence `~/straße-notes`, folder `encode("~/straße-notes")`: `folder_compartment` and `record_compartment` both
returned `fenced` on `origin/main` and return `p:stra-e-notes` on `891f968` (measured). That is a regression: `mine`
then opens the folder's transcripts, and `similar` keeps its records.

## The fix (decided; do exactly this)

1. `folder_compartment`: the folder is fenced when **either** spelling of a fence is in it:
   `any(fold(encode(f)) in folded or fold(fence_encode(f)) in folded for f in c["fenced"])`.
2. `record_compartment`: the same, with both `fold(project_name(encode(f)))` and
   `fold(project_name(fence_encode(f)))` checked against `fold(project)`.
3. `FenceRoots` and `matches_fence` stay as they are on `891f968` (the Verifier measured them: nothing they matched
   before stops matching).
4. Rewrite the VF39 entry at the end of `.ai/DECISIONS.md` (keep its heading): `matches_fence` compares the folded
   spelling, which never unfences a spelling that opens the fence; the folder and record checks compare both the
   plain and the folded spelling, because Claude Code's folder names are built the plain way; so every check fails
   closed. Keep the lines on `encode`, routing and the Kelvin sign. Lines at most 115 characters.

## Tests first (red on `891f968`, then green)

In `Compartments.test_fence_spelled_with_long_s_matches_folder_and_record` (rename it
`test_fence_with_folded_letters_matches_folder_and_record`), keep the current cases and add, as subtests, the
same-spelling direction for each of `~/straße-notes`, `~/ſecret-notes` and `~/ﬁle-box` (U+FB01): with that fence in
`fenced`, `folder_compartment(c, encode(<fence>))` and `record_compartment(c, {"project": project_name(encode(<fence>))})`
are both `"fenced"`. Run them on `891f968` first and see them fail; then green.

Add one end-to-end `mine` case beside `test_case_and_unicode_fence_cli_and_mine`: fence `~/straße-notes`, a session in
folder `encode("~/straße-notes").lower()` whose record has **no** `cwd`; `mine` reports `1 fenced folders` and writes
nothing (copy that test's shape). It must fail on `891f968`.

Mutation check: revert each `or` half you added (one at a time) and confirm a test goes red; restore with `cp`.

## Run

`~/.pyenv/versions/3.12.14/bin/python3 tests/crew_decisions_test.py Compartments FenceLinks` green. Do not run the
whole file or `scripts/ci-local.sh` (Claude runs the gate).

## Never

Real names; any file besides `scripts/crew_decisions.py`, `tests/crew_decisions_test.py`, `.ai/DECISIONS.md`;
changing `encode`, routing or messages; committing.
