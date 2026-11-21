# VF45 fix-up 1: test the `~/projects` fence on a home outside /Users

Card: VF45. Lane V, runner V1. You are Codex; work in the worktree you were started in. Read `AGENTS.md` and
`plans/briefs/VF45-stored-record-fence-recheck.md` first. Skill: `~/.codex/skills/tdd-skill/SKILL.md`.

## Why

Claude's mutation check on `eff22de` dropped `fold(encode(Path.home() / "projects"))` from the targets in
`fence_strips_from_names`, and every VF45 test still passed. The tests that fence `~/projects` patch `HOME` to
`/Users/sam`, where `re.fullmatch(r"-(users|home)(-[a-z0-9]+(-projects)?)?", s)` already matches
`-users-sam-projects`. The `~/projects` target only matters for a home outside `/Users` and `/home`, such as
the harness's own `TEST_HOME` (a temporary folder under `/var/folders`), and nothing tests that.

## The change (test only; do exactly this)

In `tests/crew_decisions_test.py`, class `Compartments`, add one test right after
`test_record_fence_on_home_or_projects_fences_every_record`:

- `test_record_fence_on_projects_fences_every_record_on_a_home_outside_users`: no `HOME` patch (the module's
  own `TEST_HOME` is in effect). First `self.assertFalse(str(Path.home()).startswith(("/Users/", "/home/")))`,
  so the test cannot pass for the wrong reason if the harness moves. Then for each fence in `("~/projects",
  str(TEST_HOME / "projects"))`: `fenced` = `["~/garden-club", fence]`, `c = self.c.check_compartments(...)`,
  and for each project in `(self.c.project_name(self.c.encode("~/projects/work2")), "work", "client-portal")`
  assert `record_compartment` is `"fenced"` for `{"project": project}` and for `{"project": project,
  "compartment": "work"}`, one `subTest(fence=..., project=..., stored=...)` each. Then, still in the same
  test, set `fenced` = `["~/garden-club", "~/projectsx"]` and assert `{"project": "work"}` is `"work"` (the
  lookalike still routes on this home).

Do not change any other file. Run the new test (green), then this mutation and show it red: in
`scripts/crew_decisions.py` change `(fold(encode(Path.home())), fold(encode(Path.home() / "projects")))` to
`(fold(encode(Path.home())),)`, run the new test, see it fail on its assertions, then restore the line exactly
(`git diff scripts/` must print nothing afterwards). Then run `python3 -m unittest tests.crew_decisions_test`:
all green.

Do not commit (the sandbox can't write the worktree's index); write the commit message to
`/tmp/vf45-fixup1-commit-V1.txt`: `crew tests: VF45 fix-up 1, a fence on ~/projects fences every record on a
home outside /Users too (mutation survivor; lane: V1)`. Report the red mutation run and the green totals.
