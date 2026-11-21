# VF45 fix-up 2: the Verifier's and the security review's findings on `eff22de`

Card: VF45. Lane V, runner V1. You are Codex; work in the worktree you were started in. Read `AGENTS.md`,
`plans/briefs/VF45-stored-record-fence-recheck.md` and `plans/briefs/VF45-fixup-1-home-outside-users.md` first.
Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`.

All examples below were reproduced by the Verifier or by Claude with `HOME=/Users/sam`.

## The findings

1. **Must-fix: a fence that spells the stripped part in another case is not matched.** `project_name` strips
   the home prefix and `projects-` case-sensitively. Fence `~/PROJECTS/secret-lab` (or `~/Projects/…`,
   `/USERS/SAM/PROJECTS/SECRET-LAB`): its `project_name` spelling is `projects-secret-lab`, which does not occur
   in the record name `secret-lab`, so `{"project": "secret-lab", "compartment": "p:secret-lab"}` is served,
   while `folder_compartment` says `"fenced"` for that folder.
2. **Must-fix: a live record keeps a saved `p:` compartment a later fence covers.** `live_record` names every
   live record `"project": "live"`. `record --work ~/projects/secret-lab/plan.md` saves `p:secret-lab`; after
   fencing `~/projects/secret-lab`, `{"project": "live", "compartment": "p:secret-lab"}` is still served.
3. **A fence on a project's `.claude` or `.claude/worktrees` folder misses its worktree records.**
   `project_name` drops `--claude-worktrees-…` first, so `~/projects/app/.claude/worktrees/feat` gives `app`.
   Fence `~/projects/app/.claude/worktrees` or `~/projects/app/.claude`: `{"project": "app", "compartment":
   "p:app"}` is served.
4. **Another Mac's home with a non-alphanumeric name does not trigger.** `fence_strips_from_names` uses
   `-(users|home)(-[a-z0-9]+(-projects)?)?` on the encoded spelling, so `/Users/jo.doe`, `/Users/jo.doe/projects`,
   `/Users/sam-x` and `/home/first_last` return False, though that Mac's own `project_name` stripped its home
   exactly before the records were merged here.
5. **Performance.** Measured 73–77 µs per `record_compartment` call per fence (2000 records, 30 fences:
   9.2 s), because every call re-encodes every fence (`Path.home`, `expanduser`) and `cmd_similar` calls it
   twice per record.
6. **A fence that fences every record does so silently:** `similar` prints `[]` and exits 0.
7. **The DECISIONS entry over-claims:** a record whose message `cwd` was inside a later fence, but whose
   transcript folder was not, is stored as `{"project": "work", "compartment": "work"}` and is still served
   until a re-mine drops it; a live record saved under a declared compartment (like `work`) has the same gap.

## The change (decided; do exactly this), in `scripts/crew_decisions.py`

1. **Case-insensitive name spellings for fences.** Add, right before `fence_strips_from_names`:

   ```python
   def folded_project_name(folder):
       """fold(project_name(folder)), with every strip matched in any case: a fence may spell ~/PROJECTS."""
   ```

   It does what `project_name` does, on `fold(folder)`, with each pattern folded or `re.I`: drop
   `--claude-worktrees-.*`; drop `fold(encode(Path.home())) + "-"` when the name starts with it, else
   `re.sub(r"^-(users|home)-[a-z0-9]+-", "", name)`; drop a leading `projects-`; turn a leading `-` into `.`.
   Do not change `project_name` (record names must stay as they are). In `project_fenced`'s spellings, replace
   both `fold(project_name(encode(f)))` and `fold(project_name(fence_encode(f)))` with
   `folded_project_name(encode(f))` and `folded_project_name(fence_encode(f))`. Keep the `--` dot variant and
   the two full spellings as they are.
2. **Worktree folders.** In the same spellings, for each `enc` in `(encode(f), fence_encode(f))`, when
   `re.search(r"--claude(-worktrees)?$", enc, re.I)` matches, also yield `folded_project_name(enc[:match.start()])`
   and, when that starts with `.`, `"--" + it[1:]`.
3. **`fence_strips_from_names` on path parts, not on the encoded spelling.** Replace its body:
   `parts = [fold(p) for p in Path(os.path.abspath(Path(fence).expanduser())).parts[1:]]`; when `parts[:3] ==
   ["system", "volumes", "data"]`, drop those three; `home = [fold(p) for p in Path.home().parts[1:]]`. Return
   True when `parts` is a prefix of `home` or of `home + ["projects"]` (this covers `/`, `/Users`, home and
   `~/projects`), or when `parts[0] in ("users", "home")` and either `len(parts) <= 2` or (`len(parts) == 3`
   and `parts[2] == "projects"`). Otherwise False. Update its docstring to say "another Mac's /Users/<name> or
   /home/<name>, whatever the name holds". No filesystem access (`abspath` and `expanduser` touch none).
4. **Compute once per map.** Add `fence_name_spellings(c)` returning a tuple `(spellings, every)`: `spellings`
   a tuple of every fence's folded spellings (what `spellings(f)` yields, for all fences, in order, duplicates
   removed), `every` = `any(fence_strips_from_names(f) for f in c["fenced"])`. Cache it on the map the way
   `fenced_roots` caches (`getattr(c, "_name_spellings", None)`; store only when `isinstance(c,
   CompartmentMap)`; add `_name_spellings = None` to `CompartmentMap`'s class attributes). `project_fenced(c,
   project)` becomes: `spellings, every = fence_name_spellings(c)`; return `every or any(dashed_in(s, folded) or
   dashed_in(s, dotless) for s in spellings)`, with `folded` and `dotless` as today. Move the `spellings`
   generator to be a helper of `fence_name_spellings`.
5. **A stored `p:` compartment is re-checked.** In `record_compartment`, between the project check and
   `if "compartment" in record: return record["compartment"]`: when the stored compartment is a `str` that
   starts with `"p:"` and is longer than 2 characters and `project_fenced(c, stored[2:])`, return `"fenced"`.
6. **`cmd_similar` calls `record_compartment` once per record** (bind it with `:=` or a small loop; same
   result). When `m.compartments is not None` and `fence_name_spellings(m.compartments)[1]` is true, print one
   line to stderr before the hits: `similar: a fence covers the home folder, ~/projects or another Mac's home,
   so every stored record is fenced`. The exit status and stdout are unchanged.
7. **`.ai/DECISIONS.md`**, the VF45 entry: keep it at 10 lines at most, plain sentences. Add: a fence spelled
   in another case, and a fence on a project's `.claude` or `.claude/worktrees` folder, fence that project's
   records; a stored `p:` compartment is re-checked like a project name, which covers live records saved under
   one; `similar` says on stderr when a fence fences every record. Replace the over-claim with the limit: a
   record whose message was in a later fence but whose transcript folder was not, and a live record saved
   under a declared compartment, are dropped only by mining again after the fence is added (filed as its own
   card).

## Tests first (red, then green), `tests/crew_decisions_test.py`, class `Compartments`

Watch each new or changed test fail on its assertion against the current code, then implement.

- In `test_record_fence_on_home_or_projects_fences_every_record`, add the fences `"~/Projects"`,
  `"/Users/jo.doe"`, `"/Users/jo.doe/projects"`, `"/Users/sam-x"` and `"/home/first_last"`.
- In `test_record_lookalikes_of_home_and_projects_fences_still_route`, remove `"/Users/sam-x"` (now a possible
  home) and add `"/Users/sam/projects/x"` and `"/srv/projects"`.
- New `test_record_case_variant_fence_fences_the_project`: `HOME` patched to `/Users/sam`; for each fence in
  `("~/PROJECTS/secret-lab", "~/Projects/secret-lab", "/USERS/SAM/PROJECTS/SECRET-LAB")`: `fenced` =
  `["~/garden-club", fence]`; `{"project": "secret-lab"}` and `{"project": "secret-lab", "compartment":
  "p:secret-lab"}` are `"fenced"`; `{"project": "work"}` is `"work"`. If `check_compartments` refuses one, drop
  it and say so.
- New `test_record_fence_on_a_projects_worktrees_folder_fences_its_records`: `HOME` patched; for each fence in
  `("~/projects/app/.claude/worktrees", "~/projects/app/.claude", "~/projects/app/.CLAUDE/Worktrees")`:
  `{"project": "app"}` and `{"project": "app", "compartment": "p:app"}` are `"fenced"`, `{"project": "work"}`
  is `"work"`.
- New `test_record_stored_p_compartment_is_rechecked_against_fences`: `fenced` = `["~/garden-club",
  "~/projects/secret-lab"]`; `{"project": "live", "compartment": "p:secret-lab"}` is `"fenced"`;
  `{"project": "live", "compartment": "p:other-lab"}` is `"p:other-lab"`; `{"project": "live", "compartment":
  "client"}` is `"client"`; `{"project": "live", "compartment": "p:"}` is `"p:"`.
- New `test_similar_says_when_a_fence_fences_every_record`: through `self.h.crew`, a corpus with one ordinary
  message record (shape as in `test_similar_drops_a_stored_record_a_later_fence_covers`), `fenced` gains
  `"~/projects"`; `similar --compartment all --corpus d.jsonl --query <text> --json`: exit 0, stdout parses to
  `[]`, stderr contains `every stored record is fenced`. Without that fence, stderr does not contain it.
- New `test_fence_name_spellings_are_computed_once_per_map`: `c = self.c.check_compartments(self.config)`;
  call `record_compartment(c, {"project": "work"})` once, then patch `self.c.fence_strips_from_names` with a
  `side_effect=AssertionError` and call it 50 more times with varied projects: no error.

Mutation checks to run and report (`cp` backup, `cmp` restore, never `git checkout`): (a) `folded_project_name`
without `re.I` / folding on the `projects-` strip turns the case-variant test red; (b) no worktree spellings
turns the worktree test red; (c) no `p:` re-check turns the `p:` test red; (d) the path-parts rule without the
`len(parts) <= 2` branch turns the home-or-projects test red; (e) no cache turns the once-per-map test red.
Then run each suite on its own, as the gate does: `python3 tests/crew_decisions_test.py`,
`python3 tests/crew_hardening_test.py`, `python3 tests/crew_documents_test.py`: all green.

Do not commit (the sandbox can't write the worktree's index). Write two messages, tests first:
`/tmp/vf45-fx2-tests-V1.txt`: `crew tests: VF45 fix-up 2, case-variant, worktree and other-home fences, stored
p: compartments, the every-record notice and one spelling pass per map (red; Verifier and security review
findings; lane: V1)` and `/tmp/vf45-fx2-code-V1.txt`: `crew: VF45 fix-up 2, fence spellings are folded and
computed once per map, worktree and other-home fences fence their records, a stored p: compartment is
re-checked, and similar says when every record is fenced (Verifier and security review findings; lane: V1)`.
Change no file but `scripts/crew_decisions.py`, `tests/crew_decisions_test.py` and `.ai/DECISIONS.md`. Report the
red runs, the five mutations and the green totals.
