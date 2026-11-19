# VF31 brief: refuse a compartment map whose fenced folder is a link

`scripts/crew_decisions.py` keeps a member's fenced folders unread. `fenced_forms` resolves only a
fence's parent, never the fence path itself, so when a fenced folder is a symbolic link, the folder
it points to is not fenced. A declared folder that links to that target can then be listed:

- `fenced: ["~/secret"]`, with `~/secret -> ~/real` and `~/real/inner/` present;
- `~/q/noor -> ~/real`, declared as compartment `noor` at `~/q/noor`;
- `folder_compartment(c, encode(~/q/noor/inner))` lists `~/real` and returns `noor`.

The owner ruled (V-Q28): **refuse the map.** One `lstat` of each fence path itself, never anything
inside it; if a fence path is a link, the member does not load, and the message names the fix. It
fails closed. On top, when the member config names a TODO command, a refusal adds one TODO item per
linked fence, and never a second while an open item for that same link exists.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md`, `CONTRIBUTING.md`. Code to read before
writing any: `scripts/crew_decisions.py` (the docstring, `check_compartments`, `read_compartments`,
`walk_resolve`, `fenced_forms`, `safe_resolve`, `folder_compartment`, `load_member`, `read_member`,
`run_main`, the `Failed` / `MemberConfigError` / `CompartmentConfigError` classes), and in
`tests/crew_decisions_test.py` the `Harness`, `tripwire_crew` and class `Compartments`
(`test_folder_compartment_never_lists_declared_link_into_fence` is the closest existing test).

Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`.

This is a public repo: invented names and paths only (`~/secret`, `~/real`, `~/q/noor`), no real
person, no fixed paths. **Commit nothing; leave every change in the working tree.**

## What to build (every choice below is decided; do not re-open one)

### 1. The refusal, in `load_member`

- New exception `FenceLinkError(CompartmentConfigError)` with an attribute `links`: the linked
  fences' expanded paths (`Path`), in config order, duplicates removed.
- New function `refuse_fence_links(c)`, called by `load_member` right after `compartments` is set
  and before the `Member` is built (so every caller, the CLI through `run_main` and any script that
  calls `load_member`, is covered). For each value in `c["fenced"]`:
  - `path = Path(value).expanduser()`. `Path` drops a trailing slash, which matters: `lstat` on
    `"~/secret/"` would follow the link. Do not use `os.path.abspath`/`resolve`/`realpath`.
  - Exactly one `os.lstat(path)`. Nothing else touches the fence: no `stat`, `readlink`,
    `scandir`, `listdir`, `open`, `exists`, `is_dir` or `resolve` on it or anything inside it.
  - `stat.S_ISLNK(mode)`: collect it. `FileNotFoundError` or `NotADirectoryError`: not a link, go
    on. Any other `OSError`: raise `CompartmentConfigError(f"compartments: cannot check whether the
    fenced folder {value} is a link")` at once (fail closed; no TODO for this case).
- If any were collected, raise `FenceLinkError` with the message, one clause per link joined by
  `"; "`: `compartments: the fenced folder {value} is a link; put the folder it points to in
  "fenced" instead, or remove the link`. `{value}` is the configured spelling. The message never
  includes the link's target (it is never read).
- A fence whose **parent** is a link (`~/links/secret`, `~/links -> ~/elsewhere`, `secret` a real
  folder) is not refused: `fenced_forms` already fences both spellings of that.
- `check_compartments`, `read_compartments` and `fenced_forms` stay as they are (no filesystem
  access added to validation).
- `run_main` already turns a `MemberConfigError` from loading into exit 2 with the message on
  stderr. Keep that; a refused map exits 2 for every command, `where` included.

### 2. The TODO item, through an optional member key `todo_command`

- `todo_command` (member.json or private.json; read from the dict `load_member` gets): a non-empty
  list of non-empty strings with no `"\x00"` or `"\n"`, whose first element starts with `/` or
  `~/`. Validate it in `load_member` whenever it is present, link or not; a bad value raises
  `MemberConfigError("member config: todo_command must be a list of strings whose first is an
  absolute or ~/ path")`. Do **not** add it to `validate_config` (the Rust member reader in
  `builder/core/src/member.rs` mirrors `validate_config` byte for byte, and this key is Python-only).
- Each element starting with `~/` is expanded with `os.path.expanduser`. Run it with
  `subprocess.run(argv, stdin=DEVNULL, capture_output=True, text=True, timeout=10, check=False)`,
  never a shell.
- The contract, written in the docstring: `<todo_command> list --json` prints a JSON object whose
  `"open"` is a list of objects with a string `"text"`; `<todo_command> add <text>` adds one open
  item (the text is one argv element).
- In `load_member`, catch `FenceLinkError` only; if `todo_command` is set, call
  `post_fence_link_todos(argv, exc.links)`, then re-raise the same exception (add a note to its
  message as below). Without `todo_command`, run nothing.
- `post_fence_link_todos(argv, links)`:
  - One `list --json` call. If it fails (cannot run, a non-zero exit, a timeout, not JSON, `"open"`
    not a list), add nothing and return the note `could not add a TODO item: <reason>`, where
    `<reason>` is one of `cannot run todo_command`, `todo_command list exited <n>`,
    `todo_command list timed out`, `todo_command list printed no open list`. Never put the tool's
    stdout or stderr in any message: it holds the person's own TODO list.
  - For each link, the item text is `Voice refused a compartment map: the fenced folder {link} is a
    link. Put the folder it points to in "fenced" instead, or remove the link.` with `{link}` the
    expanded absolute path (so two spellings of one link give one item). The dedupe marker is
    `the fenced folder {link} is a link`. Compare with runs of whitespace collapsed to one space on
    both sides (a TODO tool may collapse whitespace). Skip the link when any open item's text
    contains the marker. Only open items count: a done item for the link does not stop a new one.
  - Otherwise one `add <text>` call; a failure there gives the note
    `could not add a TODO item: todo_command add exited <n>` (or `timed out` / `cannot run`), and
    the next link is still tried.
  - When a note comes back, the re-raised error's message ends with ` (<note>)`.

### 3. Docs

- The module docstring: after the paragraph on member config, two sentences: a fenced folder that
  is itself a link refuses the whole compartment map (exit 2), and the optional `todo_command`
  (its argv form and the `list --json` / `add <text>` contract) gets one TODO item per linked
  fence, never two open at once.

## Tests first (red before green), in `tests/crew_decisions_test.py`

Add a class `FenceLinks` (setUp like `Compartments`; build every fixture under a temporary
`home` and patch `HOME`). Write each test, watch it fail on the assertion, then build.

1. **The card's fixture is refused.** `fenced: ["~/secret"]`, `~/secret -> ~/real`,
   `~/real/inner/`, `~/q/noor -> ~/real` declared as `noor`. `load_member` raises
   `FenceLinkError` whose message names `~/secret` and `put the folder it points to`, and whose
   `links` is `[home/"secret"]`. While it runs, with `os.lstat`, `os.stat`, `os.readlink` and
   `os.scandir` wrapped by `patch.object(..., wraps=...)`: `readlink` and `scandir` are never
   called, and the only `lstat` of a path under `home/"secret"` or `home/"real"` is one call with
   `home/"secret"`.
2. **Through the CLI:** `where --cwd <home>/q/noor/inner` (and `similar --query x`) exits 2, stderr
   carries the refusal, stdout is empty. Use `tripwire_crew` with `home/"real"` as the fence so any
   read of the target kills the child, and assert exit 2 (not a SIGKILL).
3. **A trailing slash** (`"~/secret/"`) is refused the same way.
4. **Not refused:** a fence that is a real folder; a fence that does not exist; a fence under a
   linked parent (`~/links/secret`, `~/links -> ~/elsewhere`, `~/elsewhere/secret/` a real folder).
   Each loads, and `where` in an unfenced folder exits 0.
5. **Fails closed on an unreadable fence:** `os.lstat` patched to raise `PermissionError` for the
   fence path only: `CompartmentConfigError` naming `cannot check`, and it is not a `FenceLinkError`.
6. **Two fences, both links**, one message with both clauses, `links` has both, in order; the same
   link spelled twice (`~/secret` and its absolute path) appears once in `links`.
7. **`todo_command` validation:** absent is fine; `"x"`, `[]`, `[""]`, `["rel/tool"]`, `[1]`,
   `["/bin/x", "a\nb"]` each raise `MemberConfigError` with the message above, even with no link.
8. **The TODO item**, with an invented fake tool: a Python script written in the test's temp dir,
   run as `todo_command: [sys.executable, "<script>"]`, keeping its items in a JSON file next to it
   (`{"todos":[{"text":…, "done":bool}]}`), implementing `list --json` (`{"open":[…], "done":[…]}`)
   and `add <text>` (collapsing whitespace like a real tool would), and failing on demand when a
   marker file exists. Seed it with one unrelated invented item (`water the invented ferns`).
   - Two refusals of one link leave one item for it (two `load_member` calls, or two CLI runs).
   - Two links in one map leave two items, one per link, after one refusal.
   - Two spellings of one link leave one item.
   - A **done** item for the link does not stop a new open one.
   - The tool failing on `list`: the refusal still raises (exit 2 through the CLI), no item is
     added, the message ends with `(could not add a TODO item: todo_command list exited 1)`.
   - Neither stdout nor stderr of any CLI run ever contains `water the invented ferns`.
   - Without `todo_command`, `subprocess.run` is never called during the refusal (patch it).
   - A fence that is not a link never runs the tool (load succeeds; the store is unchanged).

Then a mutation check per claim (copy the file aside, mutate, run the one test, restore with `cp`,
`cmp` it): drop the `S_ISLNK` check; `os.stat` instead of `os.lstat`; `str(value)` instead of
`Path(value)` (the trailing slash); dedupe on every item instead of open ones; no whitespace
collapse in the dedupe; print the tool's stderr in the note. Each must turn a test red. List the
results in your final message.

## Done when

- `python3 tests/crew_decisions_test.py`, `python3 tests/crew_documents_test.py` and
  `python3 tests/crew_hardening_test.py` pass (use Python 3.12 if `python3` is older; say which).
- `shellcheck` is untouched (no shell changed). `git diff --stat` shows only
  `scripts/crew_decisions.py` and `tests/crew_decisions_test.py`.
- Your final message lists each test, its red run, and the mutation results.
