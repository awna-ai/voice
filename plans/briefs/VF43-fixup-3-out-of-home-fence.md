# VF43 fix-up 3 brief: pin a fence outside home, and name its over-match

Card: VF43 (Persona Builder board). Lane V, runner V1. You are Codex; implement this in the worktree you were
started in. Read `AGENTS.md` first. Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`.

This branch holds VF43 (`plans/briefs/VF43-dash-for-folded-letter.md`, commit `7f3870e`), fix-up 1
(`plans/briefs/VF43-fixup-1-dash-boundary.md`, `183df4f`) and fix-up 2
(`plans/briefs/VF43-fixup-2-record-home-spellings.md`, `8e84c61`). Keep all of it. **Do not change
`scripts/crew_decisions.py`.** The repeat Verifier passed `8e84c61` with two should-fixes; this brief is only
those two: one test and one decision clause.

## What is missing (measured by Claude on this branch, `HOME=/Users/sam`)

Fix-up 2 made `record_compartment` also try each fence's two full spellings, `fold(encode(f))` and
`fold(fence_encode(f))`. Their only reach of their own (one that the project spellings and the `--` variant do not
already give) is a fence outside home, which has no home to strip:

| Fences `["~/garden-club", "/opt/vault", "/Volumes/Secret"]` | `183df4f` | `8e84c61` |
|---|---|---|
| `project_name("-System-Volumes-Data" + encode("/opt/vault"))` (`.System-Volumes-Data-opt-vault`) | `p:…` | `fenced` |
| `my-opt-vault` | `p:my-opt-vault` | `fenced` |
| `code--opt-vault` | `p:code--opt-vault` | `fenced` |
| `backup-volumes-secret` | `p:backup-volumes-secret` | `fenced` |
| `project_name(encode("/opt/vault"))` (`.opt-vault`) | `fenced` | `fenced` |
| `project_name(encode("/opt/other"))` (`.opt-other`) | `p:.opt-other` | `p:.opt-other` |
| `project_name(encode("~/projects/work"))` | `work` | `work` |

1. No test pins the first row, so dropping the full spellings would pass the suite.
2. The decision entry names only the hidden `.ssh` over-match, not the out-of-home one (rows 2 to 4).

## Tests first (in `tests/crew_decisions_test.py`, class `Compartments`)

Add one test right after `test_record_full_fence_spelling_matches_a_data_volume_project`:

`test_record_full_fence_spelling_matches_a_project_outside_home`. Set
`self.config["fenced"] = ["~/garden-club", "/opt/vault", "/Volumes/Secret"]` and patch the home with
`patch.dict(os.environ, {"HOME": "/Users/sam"})` around `check_compartments` and every call, as the test above it
does. One `subTest` per project:

- `"fenced"`: `self.c.project_name("-System-Volumes-Data" + self.c.encode("/opt/vault"))`, then the accepted
  over-match `"my-opt-vault"`, `"code--opt-vault"` and `"backup-volumes-secret"`;
- not fenced: `self.c.project_name(self.c.encode("/opt/other"))` routes as `"p:" + that project`, and
  `self.c.project_name(self.c.encode("~/projects/work"))` routes to `"work"`.

The code already passes this test, so prove it bites with a mutation check: in a scratch copy of
`scripts/crew_decisions.py` (save it with `cp`, compare with `cmp`, never `git checkout --`), delete the two lines
`yield full_plain` and `yield full_folded` in `record_compartment`, run the new test, and confirm the data-volume
subtest and the three over-match subtests go red while the two routed subtests stay green. Report exactly which
went red. Restore the file from your copy and confirm `cmp` shows it unchanged.

## The decision (`.ai/DECISIONS.md`)

In the entry `## 2026-11-20: a "-" in a transcript folder's name may stand for a folded letter (VF43)`, make
exactly this one replacement and touch no other line.

Replace this line:

```
any other folder (`code--ssh`); that over-match is accepted.
```

with these lines, word for word:

```
any other folder (`code--ssh`); that over-match is accepted. A fence outside home has no home to strip, so its
full spelling matches any project that holds it: `/opt/vault` fences a data-volume record
(`.System-Volumes-Data-opt-vault`) and also `my-opt-vault` and `code--opt-vault`, and `/Volumes/Secret` fences
`backup-volumes-secret`; that over-match is accepted too.
```

## Run

Run `python3 -m unittest tests.crew_decisions_test.Compartments tests.crew_decisions_test.FenceLinks`, then the
whole file with `python3 tests/crew_decisions_test.py`. Report the counts. The machine is loaded: a subprocess test
that hits its 60-second timeout and passes alone is load, not a regression; name it. Do not run
`scripts/ci-local.sh` (Claude runs the gate).

## Commit

**Do not commit and do not push**: your sandbox cannot write this worktree's git index. Leave the two changed
files (`tests/crew_decisions_test.py`, `.ai/DECISIONS.md`) uncommitted and say in your last message that nothing
was committed. Claude commits this brief and your change. No real person's name, handle or path anywhere.
