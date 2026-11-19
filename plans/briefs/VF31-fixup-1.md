# VF31 fix-up 1: the security review's and the Verifier's findings

Read `plans/briefs/VF31-fence-link-refusal.md` first; everything there still holds except where this
says otherwise. The code is `refuse_fence_links`, `post_fence_link_todos` and `load_member` in
`scripts/crew_decisions.py`; the tests are class `FenceLinks` and the guard helpers in
`tests/crew_decisions_test.py`, plus `tests/crew_documents_test.py`. Skills as before
(`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`). Public repo, invented fixtures only. **Commit nothing.**

Every fix below fails closed and adds no filesystem access. Test first for each (red on the
assertion, then green), and a mutation per fix (copy aside, mutate, run, restore with `cp`, `cmp`).

## 1. A `..` part in a fence path is refused, before any `lstat` (must-fix)

`fenced: ["~/nope/../secret"]` with `~/nope` missing loads today: the `lstat` of the raw spelling
fails with "not there", while fence matching uses the `os.path.abspath` form `~/secret`, which is
the link. `["~/secret/inner/.."]` loads too, and its `lstat` walks through the link into the target.

- In `refuse_fence_links`, first, for every value: if `".."` is one of `Path(value).expanduser().parts`,
  raise `CompartmentConfigError(f"compartments: the fenced folder {value} has a \"..\" part; write it without \"..\"")`.
  Not a `FenceLinkError`, so no TODO item. No `lstat` of any fence happens when one value is refused.
- Tests: both spellings above are refused, with `os.lstat` patched to fail the test if called for
  any path; `where` through the CLI exits 2 with that message and empty stdout.

## 2. A fence inside another fence is refused, before any `lstat` (must-fix)

`fenced: ["~/secret", "~/secret/inner"]` with `~/secret -> ~/real` lstats `~/secret/inner`, which
reads `~/real/inner`'s metadata (killed under the tripwire). With two real folders
(`["~/real", "~/real/inner"]`) it reads metadata inside the fence `~/real`.

- In `refuse_fence_links`, after step 1 and before any `lstat`: compare every pair of values by
  `under(Path(inner).expanduser(), Path(outer).expanduser())` (the module's case- and
  NFC-folding comparison), skipping a value compared with itself and exact duplicates. If one lies
  under another, raise `CompartmentConfigError(f"compartments: the fenced folder {inner} is inside the fenced folder {outer}; fence only {outer}")`
  for the first such pair in config order. No TODO item.
- Tests: `["~/secret", "~/secret/inner"]` (the link case), `["~/real", "~/real/inner"]` and
  `["~/Real", "~/real/inner"]` (case-folded) are refused with no `os.lstat` call at all; through the
  CLI, under `tripwire_crew` with `home/"real"` as the fence and **without** `allow_root_metadata`,
  `where` exits 2 (not -9).

## 3. Non-UTF-8 output from the TODO tool, and an unencodable fence path (should-fix)

- Run the tool with bytes (drop `text=True`); decode `list`'s stdout with `decode("utf-8")` inside
  the existing parse `try`, so a `UnicodeDecodeError` gives `could not add a TODO item: todo_command
  list printed no open list`. `add`'s output is never read. Pass the `add` text as `str` as now
  (subprocess encodes argv itself).
- `os.lstat` raising `ValueError` (a lone surrogate such as `"/tmp/\ud800x"`, which JSON allows, or
  an embedded NUL) is handled like any other `OSError` there: `cannot check whether the fenced
  folder … is a link`.
- Tests: a fake `list --json` that prints the bytes `{"open": [{"text": "caf\xe9"}]}` gives the note
  and exit 2 through the CLI (no traceback in stderr); the surrogate fence gives "cannot check".

## 4. One item per link, whatever the spelling (should-fix)

Two spellings of one link (`~/secret` and `~/SECRET` on a case-insensitive volume, NFC and NFD,
`/tmp/…` and `/private/tmp/…`) give two items today.

- Dedupe the collected links by `(st_dev, st_ino)` from the `lstat` result already taken (no new
  probe). Keep the first spelling in config order for `links`, the message clause and the item.
- Tests: `["~/secret", "~/SECRET"]` gives one link and one item when the test volume is
  case-insensitive (check by `os.path.samefile` on a fixture pair you create outside the fence;
  `skipTest` otherwise); a second, real hard path spelling (`str(home.resolve() / "secret")` beside
  `"~/secret"` when they differ as strings) gives one.

## 5. The dedupe marker ends at the sentence (should-fix)

An open item for a link named `<home>/secret is a link/x` stops the item for `<home>/secret` today.
Make the marker `the fenced folder {link} is a link.` (with the full stop, as in the item text).
Test: seed the store with the item for `<home>/secret is a link/x`; a refusal for `~/secret` still
adds its own item.

## 6. The tripwire helpers stay strict about everything but the one load-time read

- `tripwire_crew(..., allow_root_metadata=True)` in `tests/crew_decisions_test.py`: keep
  `ensure_ascii=False` as the strict branch has it, and keep denying `file-read*` (xattrs included)
  everywhere under the fence; the only thing allowed is `file-read-metadata` on the fence path
  itself (`literal`). One way:
  `(deny file-read* file-write* (require-all (subpath P) (require-not (literal P))) (with send-signal SIGKILL))`
  plus `(deny file-read-data file-read-xattr file-write* (literal P) (with send-signal SIGKILL))`.
  Prove the narrower profile still kills: a test where the child reads a file inside the fence
  under `allow_root_metadata=True` gets -9.
- `tests/crew_documents_test.py::test_document_and_folder_review_kernel_tripwire` is red now (-9):
  its child loads the member, whose one `lstat` of each fence root the profile kills. Give it the
  same allowance (share one helper in `tests/crew_fixtures.py` if both files need it) and nothing
  more; it must still be killed by any read inside the fence.

## Done when

- `python3 tests/crew_decisions_test.py`, `python3 tests/crew_documents_test.py` and
  `python3 tests/crew_hardening_test.py` pass on Python 3.12 (`$(pyenv prefix 3.12)/bin/python3`
  if `python3` is older). The kernel tripwire tests may skip in your sandbox; say which ran.
- `git diff --stat` shows only `scripts/crew_decisions.py`, `tests/crew_decisions_test.py`,
  `tests/crew_documents_test.py` and, if used, `tests/crew_fixtures.py`.
- Your final message lists each new test with its red run and each mutation's result.
