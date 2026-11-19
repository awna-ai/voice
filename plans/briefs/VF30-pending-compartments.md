# VF30: `pending --compartment` refuses an unknown name, and keeps compartments apart

Branch `lane-v1/VF30`, off `main`. Two files: `scripts/crew_decisions.py` and `tests/crew_decisions_test.py` (plus
the usage text at the top of `crew_decisions.py`). Found by the Verifier on `e2ac948`, which added
`pending --compartment`. Four gaps, each a failing test first.

Read first: `cmd_pending`, `queue_files`, `queue_compartment`, `open_control_qs`, `answered_escalations`,
`path_compartment`, `safe_resolve`, `route_compartment`, `live_record`, `record_compartment`; and in the tests the
`Compartments` class (`setUp` builds the map: compartments `work` and `client`, fence `~/garden-club`, hub paths
`~/.hub/projects` and `~/.other-hub/projects`, route `client-stack → client`) and its `test_pending_*` tests.

## 1. An unknown `--compartment` name is refused

Today `pending --compartment nope` prints nothing and exits 0, so a typo silently shows nothing forever.

- In `cmd_pending`, keep the two existing refusals first and unchanged (`fenced` → exit 1 with
  `pending: refusing compartment fenced`; no map → exit 1 with `pending: --compartment requires compartments
  configuration`). Then: a name is known when it is a key of `m.compartments["compartments"]`, or matches
  `p:[A-Za-z0-9.][A-Za-z0-9._-]*` in full (the same pattern `check_compartments` uses for `p:` route targets; put it
  in one module-level constant and use it in both places). Anything else raises
  `CompartmentConfigError(f"pending: unknown compartment; known: {', '.join(sorted(names))}, or p:<project>")`,
  which `run_main` turns into exit 2. Never echo the name the person typed.
- All three refusals happen before `seen.json` is read or written, and print nothing on stdout.
- **Red first:** extend `test_pending_compartment_refusals_do_not_touch_seen` (or add a neighbour in its style) with
  `nope`, `all`, `hub-mixed`, `p:` and `p:-x` → exit 2, stderr exactly
  `crew_decisions: pending: unknown compartment; known: client, work, or p:<project>\n`, stdout empty, `seen.json`
  byte-identical (absent stays absent). And `p:other` is accepted (exit 0).

## 2. `seen` is kept per compartment

Today `seen.json` is one list of ids. Two compartments whose folders hold the same session id give two items with the
same id (`<session>:<tool_id>`); a run scoped to one marks the id seen, and the other compartment never prints its
item.

- **Without compartments configured** (`m.compartments is None`): nothing changes. The file stays a sorted JSON list
  of id strings, byte for byte (`test_pending_without_compartments_preserves_output_bytes` must pass unchanged).
- **With compartments configured:** an item's seen key is the two-item list `[compartment, id]`, using the item's own
  `compartment` field (every pending item carries one when a map is configured: asks from `transcript_compartment`,
  queue items from `queue_compartment`).
- **The file** stays one JSON list. On read, accept both entry forms: a string is a legacy entry and means that id is
  seen in every compartment (it was printed once, to whoever ran then); a two-item list of strings is
  `[compartment, id]`. Any other entry shape, or a file that isn't a JSON list, raises
  `Failed("pending: the seen file is not readable")` (exit 1), and the file is left as it was.
- An item is new when its id is not a legacy entry, its `[compartment, id]` is not an entry, and it is in scope
  (`--compartment` absent, or equal to the item's compartment).
- On write (only when something new was printed, as today): keep every entry already in the file (legacy strings
  included), add `[i["compartment"], i["id"]]` for each new item, and write the list sorted by
  `json.dumps(entry, ensure_ascii=False)` with the existing `private_write`. An unscoped run marks each printed item
  under its own compartment; a scoped run marks only what it printed.
- **Red first** (in `Compartments`): two `asking(...)` sessions with the **same** session id, one in
  `~/projects/work` and one in `~/projects/client` (encoded folder names via `self.c.encode`). Run
  `--compartment work`: prints only the work one. Then `--compartment client`: prints the client one (fails today).
  Then each again: prints nothing. Assert `seen.json`'s exact content after each step. Second test: a legacy
  `seen.json` (`["a:t"]`) suppresses `a:t` in both compartments and is kept on the next write. Third: a seen file
  holding `{}` or `[1]` → exit 1, stderr `crew_decisions: pending: the seen file is not readable\n`, file unchanged.
- Update the existing tests that read `seen.json` under compartments (`test_pending_compartment_leaves_other_events_unseen`,
  `test_pending_queue_uses_project_folder_for_questions_and_answers`, `test_pending_compartments_and_fenced_queues`)
  to the new entry form; change only their `seen.json` expectations.

## 3. A queue item is labelled by its logical folder, never by where a link points

`queue_compartment(m, path)` labels `<queues>/<folder>/queue.md` with `path_compartment(c, <folder>)`, which
resolves links. So a hub folder that is a symlink is labelled by the folder it points to, and a hub folder whose
name the hub rule rejects (anything not `[A-Za-z0-9][A-Za-z0-9._-]*`, e.g. `my app`) falls through to the
compartment that owns the hub's parent. Decided (the spec allows labelling by the logical path or refusing the
link; we keep the link and label logically, as `test_pending_queue_uses_project_folder_for_questions_and_answers`
already does for a linked `queue.md`):

- Keep the signature `queue_compartment(m, path)`; `root = path.parent.parent` (the queues folder `queue_files`
  walked), `name = path.parent.name`.
- `real_root, fenced = safe_resolve(c, root, os.getcwd())`. If `fenced`: return `{"compartment": "fenced"}`
  (`queue_files` already refuses a fenced root; this is belt and braces).
- **The queues folder is a hub path** (any `hub["paths"]` entry whose `safe_resolve` is not blocked and whose
  resolved path equals `real_root` under `fold(str(...))`): return
  `{"compartment": route_compartment(c["hub"], name)}`. That is the hub's own rule, by the folder's name: a route
  key gives its target, anything else `p:<name>`, odd names included (`p:my app` can't be asked for by
  `--compartment`, so only an unscoped run prints it).
- **Otherwise:** label `real_root / name` without following `name` itself: add a keyword
  `follow_last=True` to `path_compartment`. When `resolve` is true and `follow_last` is false, compute
  `parent, parent_fenced = safe_resolve(c, raw.parent, cwd)` (where `raw = absolute_path(path, cwd)`),
  `real = parent / raw.name`, and `fenced = parent_fenced or matches_fence(Path(os.path.abspath(raw)),
  lexical_fences(c, cwd)) or matches_fence(real, fenced_forms(c, cwd))`; then carry on exactly as the resolve branch
  does with `forms = (real,)`. No other caller changes; the default keeps today's behaviour.
- `queue_files` is unchanged: it still skips a folder or `queue.md` whose resolved path is fenced, so a link into a
  fence is still never read.
- **Red first** (in `Compartments`, queues under the hub `~/.hub/projects` built in the harness home):
  (a) a hub folder `client-stack` that is a symlink to `~/projects/work/q` (a real folder holding `queue.md` with
  one `CONTROL-Q`) is labelled `client` (today: `work`); (b) a hub folder `my app` (a real folder) is labelled
  `p:my app` (today: `work`, the hub's owner); (c) a hub folder `client-stack` whose `queue.md` links to
  `~/projects/work/queue.md` is labelled `client` (passes today; keep it as a pin); (d) the same link layouts under a queues folder that is **not** a
  hub (`<home>/queues`, with `<home>/queues/app` declared in `client`): a linked folder `app → ~/projects/work/q` is
  labelled `client`. And: a hub folder linking into `~/garden-club/x` (the fence) is not printed at all and nothing
  under the fence is read (assert the item is absent; the existing `chmod(0)` trick in
  `test_pending_compartments_and_fenced_queues` shows how to prove no read).
- Update the direct call near the end of the hub tests
  (`self.c.queue_compartment(m, Path("~/.HUB/PROJECTS/CLIENT-STACK/queue.md"))` → still `{"compartment": "client"}`).

## 4. `live_record` keeps the line's compartment

`record` writes `"compartment"` into each live line when a map is configured, but `live_record` (used by
`sources mine` for a `live` source) drops it, so every live record falls back to `record_compartment`'s
`p:live` and `similar --compartment X` never sees it.

- In `live_record`: when `line.get("compartment")` is a non-empty string, set `record["compartment"]` to it
  (insert it right after `"project"` in the dict). Otherwise leave the record exactly as today.
- **Red first:** a `live_record` unit test: a line with `"compartment": "work"` gives a record whose
  `compartment` is `work` and whose `record_compartment(c, record)` is `work`; a line with none gives the same
  dict as today (no `compartment` key).

## Usage text

In the module docstring's `pending` entry, after "--compartment prints and marks seen only that compartment's items;",
say that an unknown name exits 2, and that seen is kept per compartment. Keep it to the docstring's style.

## Rules

- Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
  `~/.codex/skills/security-and-hardening/SKILL.md` (this is a privacy filter: never stat, list or open anything
  inside a fenced folder; a new code path that touches the filesystem must go through `safe_resolve` or
  `path_compartment` first).
- Red first for each of the four, and note each red failure message. Match the surrounding code: no new helpers
  beyond the constant and the keyword, no comments that restate the code.
- Run: `python3 tests/crew_decisions_test.py`, `python3 tests/crew_hardening_test.py`,
  `python3 tests/crew_sources_test.py`, `python3 tests/crew_extra_sources_test.py`. All must pass.
- Public repo: invented fixtures only, no real person's name or data, no fixed paths. Commit nothing. Summarise:
  each red failure, the green runs, and anything you couldn't do as written.
