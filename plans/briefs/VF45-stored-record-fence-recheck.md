# VF45 brief: a stored record is re-checked against the fences every time it is read

Card: VF45 (Persona Builder board). Lane V, runner V1. You are Codex; implement this in the worktree you were
started in. Read `AGENTS.md` first. Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`.

## The defects (found by VF43's security review; reproduced by Claude at `a2ebd09` by reading the code)

`record_compartment(c, record)` in `scripts/crew_decisions.py` decides which compartment a stored record
(a line of `decisions.jsonl`) belongs to. `cmd_similar` uses it to drop fenced records and to keep only the
asked compartment. Two gaps:

1. **A stored `compartment` key is trusted as it is.** The first two lines are
   `if "compartment" in record: return record["compartment"]`. `cmd_mine` writes that key when it mines. A
   record mined before a fence was added (say `~/projects/secret-lab` fenced today, the record mined last week
   with `"project": "secret-lab", "compartment": "p:secret-lab"`) is still served by `similar --compartment all`
   and by `similar --compartment p:secret-lab`. The fence never sees it.
2. **A fence on a folder `project_name` strips never matches.** `project_name` drops the home folder's
   spelling and then a leading `projects-`, so a session in `~/projects/work2` gets the project `work2`. A fence
   `~/projects` has the spellings `projects`, `-users-<u>-projects` and so on, none of which occurs in `work2`,
   so the record is not fenced, while `folder_compartment` on the folder `-Users-<u>-projects-work2` says
   `"fenced"`. The same holds for a fence on the home folder itself (`~`) and on anything above it (`/Users`,
   `/`), and for another Mac's home (`/Users/<other>`, `/home/<other>`, which `project_name` also strips).

A record carries only its `project` name: no folder, no `cwd`. So the read-time check works from the project
name alone and must fail closed when the name cannot tell.

## The fix (decided; do exactly this)

1. In `scripts/crew_decisions.py`, move the fence test that is inside `record_compartment` today (the
   `folded`, `dotless`, `spellings(f)` and the `any(dashed_in(...))` lines) unchanged into a new top-level
   function placed right before `record_compartment`:

   ```python
   def project_fenced(c, project):
       """True when a record's project name may come from a fenced folder. The name is all a stored record
       keeps, so a fence on a folder project_name drops from every name (home, ~/projects, or above one)
       fences every record."""
   ```

   It returns `True` when either of these holds, `False` otherwise:
   - the existing spelling check, moved as it is (same spellings, same `dashed_in` calls, same `dotless` step);
   - `any(fence_strips_from_names(f) for f in c["fenced"])`, with the helper below.

2. Add, right before `project_fenced`:

   ```python
   def fence_strips_from_names(fence):
       """True when project_name may drop this fence's whole folder from a record's name: the fence is the
       home folder, ~/projects, another Mac's /Users/<name> or /home/<name> (or its projects), or a folder
       above one of them. Such a fence leaves no letter of itself in the names of the records under it."""
   ```

   Compute it on folded encoded spellings, never on the filesystem (no `stat`, no `resolve`, no listing: a
   fence is never looked inside). For each `s` in `(fold(encode(fence)), fold(fence_encode(fence)))`:
   - drop one leading `-system-volumes-data` from `s` when `s` starts with `-system-volumes-data-` (the data
     volume spelling of the same folder);
   - return `True` when `s == "-"` (the root folder), or when for some `t` in
     `(fold(encode(Path.home())), fold(encode(Path.home() / "projects")))` either `t == s` or
     `t.startswith(s + "-")`, or when `re.fullmatch(r"-(users|home)(-[a-z0-9]+(-projects)?)?", s)` matches.

   Return `False` when no spelling matched. Do not cache it; it is a few string comparisons per fence.

3. `record_compartment` becomes:

   ```python
   def record_compartment(c, record):
       """Re-check every read record's project against today's fences, so a record mined before a fence was
       added never routes past it; a stored compartment is kept only when the project is clear."""
       project = record.get("project") if "compartment" in record else record["project"]
       if isinstance(project, str) and project_fenced(c, project):
           return "fenced"
       if "compartment" in record:
           return record["compartment"]
       ...  # the rest exactly as today: the declared-folder match on the project, else "p:" + project
   ```

   A record with a stored `compartment` and no `project` (or a non-string one) keeps its stored compartment, as
   today. A record with neither key still raises `KeyError`, as today. The `matches(folder)` helper and the
   `found` lines stay as they are.

4. Nothing else changes: not `project_name`, `folder_compartment`, `path_compartment`, `cmd_mine`,
   `cmd_similar`, `live_record` or any fence-walk code. `cmd_similar` already calls `record_compartment`, so
   it gains the fix.

5. Add a `.ai/DECISIONS.md` entry right after the VF43 one: `## 2026-11-21: a stored record is re-checked
   against the fences when it is read (VF45)`, 4 to 7 lines, plain sentences: the stored `compartment` is kept
   only when the record's project passes today's fences, so a fence added after mining hides the records it
   covers; a record keeps only its project name, so a fence on the home folder, on `~/projects`, on another
   Mac's home or on a folder above one fences every stored record (the name can't say where it came from); the
   accepted over-fence (a fence `~/projects` also fences records from `~/notes`, and a fence spelling found
   anywhere in a project name fences it, as before, now for stored records too); and that the check reads no
   file or folder.

## Tests first (red, then green), in `tests/crew_decisions_test.py`, class `Compartments`

Invented folders and records only. Write all four tests, run them against the unfixed code and watch each fail
on its assertion (not on an import or a missing name; the third may pass on the unfixed code, which is fine and
is there to keep the routing), then implement.

- `test_record_stored_compartment_is_rechecked_against_fences`: `fenced` = `["~/garden-club",
  "~/projects/secret-lab"]`, `c = self.c.check_compartments(self.config)`. Assert, each in its own `subTest`:
  `{"project": self.c.project_name(self.c.encode("~/projects/secret-lab")), "compartment": "p:secret-lab"}` is
  `"fenced"`; `{"project": "secret-lab-app", "compartment": "work"}` is `"fenced"`; `{"project": "work",
  "compartment": "work"}` is `"work"`; `{"project": "live", "compartment": "client"}` is `"client"`;
  `{"compartment": "hub-mixed"}` is `"hub-mixed"`.
- `test_record_fence_on_home_or_projects_fences_every_record`: with `patch.dict(os.environ, {"HOME":
  "/Users/sam"})`, for each fence in `("~/projects", "~/PROJECTS", "~", "/Users", "/Users/sam", "/",
  "/Users/other", "/home/other/projects", "/System/Volumes/Data/Users/sam/projects")`: set `fenced` to
  `["~/garden-club", fence]`, check the config, and for each project in
  `(self.c.project_name(self.c.encode("~/projects/work2")), "work", ".config", "client-portal")` assert
  `record_compartment` is `"fenced"` both for `{"project": project}` and for `{"project": project,
  "compartment": "work"}`. One `subTest(fence=..., project=..., stored=...)` per assertion. If
  `check_compartments` refuses one of these fences as a config, leave that fence out and say so in your report.
- `test_record_lookalikes_of_home_and_projects_fences_still_route`: same `HOME` patch; for each fence in
  `("~/projectsx", "~/proj", "/Users/sam/proj", "/Users/sam-x", "/home2")`, `fenced` = `["~/garden-club", fence]`:
  `{"project": "work"}` is `"work"`, `{"project": "work", "compartment": "work"}` is `"work"` and
  `{"project": "notes-app"}` is `"p:notes-app"`.
- `test_similar_drops_a_stored_record_a_later_fence_covers` (end to end, through `self.h.crew`): write a corpus
  `d.jsonl` with two message records built in the same shape the existing `similar` tests use (see
  `ExampleMember`, around the `"similar", "--corpus", "d.jsonl"` call): one with `"project":
  self.c.project_name(self.c.encode("~/projects/secret-lab"))`, `"compartment": "p:secret-lab"`, the other with
  `"project": "work"`, `"compartment": "work"`, both with the same invented text. Add `"~/projects/secret-lab"`
  to `fenced` and save the config the way this class does (`self.save_config()`). Run `similar
  --compartment all --corpus d.jsonl --query "<that text>" --json`: exit 0 and only the `work` record's id comes
  back. Run it again with `--compartment p:secret-lab`: exit 0 and an empty list.

Then run `python3 -m unittest tests.crew_decisions_test` (from the repo root) and `python3 -m unittest
tests.crew_hardening_test tests.crew_documents_test`: all green. Keep the test file's style (subTests, no new
helpers beyond a one-line local one if a test needs it).

## Commits

Two commits, messages written to a file and passed with `git commit -F`, no AI attribution or trailer:
1. `crew tests: VF45, a stored record is re-checked against today's fences, and a fence on home or ~/projects
   fences every record (red; lane: V1)` with only the test file.
2. `crew: VF45, record_compartment re-checks a stored record's project against the fences on every read, and a
   fence on a folder project_name strips fences every record (lane: V1)` with the code and `.ai/DECISIONS.md`.

Do not push. Do not touch any other file. Report the red run's failing assertions and the green run's totals.
