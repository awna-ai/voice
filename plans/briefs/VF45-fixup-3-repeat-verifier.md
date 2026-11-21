# VF45 fix-up 3: the repeat Verifier's should-fixes on `58528f1`

Card: VF45. Lane V, runner V1. You are Codex; work in the worktree you were started in. Read `AGENTS.md` and the
three earlier VF45 briefs in `plans/briefs/` first. Skill: `~/.codex/skills/tdd-skill/SKILL.md`.

## The findings (repeat Verifier, Opus, PASS with should-fixes; `HOME=/Users/sam` in the examples)

1. **A fence on the transcript root or on one transcript folder doesn't hide the records mined from it.**
   Fence `~/.claude/projects`, or `~/.claude/projects/-Users-sam-projects-secret-lab`, with the stored record
   `{"project": "secret-lab", "compartment": "p:secret-lab"}`: `record_compartment` returns `p:secret-lab`
   (`mine` itself skips those folders).
2. **Nothing tests the other-home strip in `folded_project_name`.** Changing
   `re.sub(r"^-(users|home)-[a-z0-9]+-", "", name)` to `name` keeps all 276 tests green, while fence
   `/Users/other/work/secret-lab` then stops fencing `{"project": "work-secret-lab"}`.
3. **The every-record notice misses a fence whose name spelling folds to "".** Fence `~/projects-` or
   `~/projects_` gives the spelling `""`, and `dashed_in("", …)` is true, so every record is fenced, but
   `fence_name_spellings(c)[1]` is False and `similar` prints nothing on stderr.
4. **DECISIONS over-claims:** a live record saved under a declared compartment is not dropped by mining again
   (`live_record` copies the line's `compartment` and never checks its `work` path).

## The change (decided; do exactly this)

In `scripts/crew_decisions.py`:

1. In `fence_strips_from_names`, add `home + [".claude"]` and `home + [".claude", "projects"]` to the targets
   whose prefix the fence's parts may be (a fence on `~/.claude` or `~/.claude/projects` fences every record:
   every record was mined from under it).
2. In `fence_name_spellings`'s `spellings(f)`: compute `parts` as in `fence_strips_from_names` (folded parts of
   `os.path.abspath(Path(f).expanduser())`, without the `system/volumes/data` step); when
   `len(parts) > len(home) + 2` and `parts[:len(home) + 2] == home + [".claude", "projects"]`, also yield
   `folded_project_name(Path(f).expanduser().parts[len(home) + 3])` (the transcript folder's own name, as written,
   so `folded_project_name` sees the encoded folder name), and its `--` dot variant when it starts with `.`.
   `home` is `[fold(p) for p in Path.home().parts[1:]]`, as in `fence_strips_from_names`; factor that and the
   parts computation into one small helper `fence_parts(fence)` returning `(parts, home)` used by both, with the
   data-volume step kept in `fence_strips_from_names` only.
3. In `fence_name_spellings`, the every-record verdict becomes
   `any(fence_strips_from_names(f) for f in c["fenced"]) or "" in spellings` (an empty spelling matches every
   name).
4. `.ai/DECISIONS.md`, the VF45 entry: replace "and a live record saved under a declared compartment, are dropped
   only by mining again after the fence is added (filed as its own card)" so the entry says: a record whose
   message was in a later fence but whose transcript folder was not is dropped only by mining again; a live
   record saved under a declared compartment is not dropped even then, since the live log's `work` path is never
   re-checked; both are VF48. Add, in the same entry, that a fence on `~/.claude`, `~/.claude/projects` or one
   transcript folder fences the records mined from it. Keep the entry at 12 lines at most.

## Tests first (red, then green), `tests/crew_decisions_test.py`, class `Compartments`

- In `test_record_fence_on_home_or_projects_fences_every_record`, add the fences `"~/.claude"` and
  `"~/.claude/projects"`, and `"~/projects-"` (an empty name spelling). If `check_compartments` refuses one,
  drop it and say so.
- New `test_record_fence_on_one_transcript_folder_fences_its_records`: `HOME` patched to `/Users/sam`; fence
  `"~/.claude/projects/-Users-sam-projects-secret-lab"`: `{"project": "secret-lab"}` and `{"project":
  "secret-lab", "compartment": "p:secret-lab"}` are `"fenced"`, `{"project": "work"}` is `"work"`.
- New `test_record_fence_inside_another_macs_home_fences_its_records`: `HOME` patched to `/Users/sam`; fence
  `"/Users/other/work/secret-lab"`: `{"project": "work-secret-lab"}` and `{"project": "work-secret-lab",
  "compartment": "p:work-secret-lab"}` are `"fenced"`; `{"project": "work"}` is `"work"`.
- In `test_similar_says_when_a_fence_fences_every_record`, add a third case: `fenced` gains `"~/projects-"`;
  exit 0, stdout `[]`, stderr contains `every stored record is fenced`. If config refuses `~/projects-`, use
  `~/projects_` instead.

Watch each fail on its assertion first. Mutations to run and report (`cp` backup, `cmp` restore): dropping the
`.claude` targets, the transcript-folder spelling, the other-home `re.sub` in `folded_project_name`, and the
`"" in spellings` term each turn a test red. Then `python3 tests/crew_decisions_test.py`,
`python3 tests/crew_hardening_test.py`, `python3 tests/crew_documents_test.py`, each on its own: all green.

Do not commit. Write one message, `/tmp/vf45-fx3-V1.txt`: `crew: VF45 fix-up 3, a fence on ~/.claude, the
transcript root or one transcript folder fences its records, an empty fence spelling sets the every-record
notice, the other-home strip is tested, and DECISIONS names the live-record gap exactly (repeat Verifier
should-fixes; lane: V1)`. Change no file but `scripts/crew_decisions.py`, `tests/crew_decisions_test.py` and
`.ai/DECISIONS.md`. Report the red runs, the four mutations and the green totals.
