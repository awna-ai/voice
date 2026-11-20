# VF40: a member config that is not a readable JSON object exits 2 on every Python

Worktree: `.claude/worktrees/VF40-attended` in the voice checkout, branch `claude/VF40-nonobject-exit`, off
`origin/main` `34c4e1a`. Work only in this worktree. Do not commit; Claude reviews and commits.

Skills to follow: `~/.codex/skills/tdd-skill/SKILL.md` (test first, red before green) and
`~/.codex/skills/code-quality/SKILL.md`. Read the repo's `AGENTS.md` and `.ai/PROJECT.md` first (no real people
in any file; Python standard library only; Python 3.9+).

## The bug (measured)

`tests/crew_decisions_test.py` → `Compartments.test_member_and_private_bad_encoding_or_nesting` writes
`member.json` (then `private.json`) as a list nested 10,000 deep and expects `where` to exit 2.

- On Python 3.9, 3.10 and 3.12, `json.loads` raises `RecursionError`; `read_json` (in `scripts/crew_decisions.py`,
  about line 865) turns it into `MemberConfigError`, and `run_main` returns 2.
- On Python 3.14 and 3.15 the decoder parses it, so `validate_config` raises plain `Failed` ("must be a JSON
  object") and `run_main` returns 1. Both subtests fail.

So the exit code depends on the decoder's recursion limit. The same split also hits a deep list *inside* an object
(`{"filters": [[[...]]]}`): 3.12 exits 2, 3.15 exits 1 or even 0.

## The rule to implement

**Exit 2 means Voice cannot read the member's config file as a JSON object:** not UTF-8, not JSON, nested deeper
than `MAX_CONFIG_DEPTH`, or a top level that is not an object. Those raise `MemberConfigError`. A readable object
with a wrong or missing value (a missing key, a bad regex, `filters` that is not an object, a bad `pasted_prefixes`,
a bad `queue_answerer`, `sources` in `member.json`) keeps exiting 1 (`Failed`) as today; do not change those.
`todo_command` and compartment-map refusals already exit 2; leave them.

## Changes in `scripts/crew_decisions.py`

1. Add a module constant near `REQUIRED`: `MAX_CONFIG_DEPTH = 64` (real member configs are 4 deep, measured).
2. Add a function `json_depth(text)` that returns the deepest nesting of `[` / `{` in a JSON text, **without
   recursion**, ignoring brackets inside strings (track `"` and backslash escapes). One linear pass. It must not
   validate JSON (`json.loads` still does that); it is only a bound. Stop early is fine (`json_too_deep(text, limit)`
   returning bool is an acceptable alternative name), but keep it one small helper.
3. `read_json(path)`:
   - Read the text as now (`path.read_text()`; a `UnicodeDecodeError` stays `MemberConfigError`).
   - If the depth exceeds `MAX_CONFIG_DEPTH`, raise
     `MemberConfigError(f"member config: {path.name}: nested deeper than {MAX_CONFIG_DEPTH} levels")` **before**
     calling `json.loads`.
   - Keep catching `RecursionError` as `MemberConfigError` (defence in depth; keep the message
     `member config: {path.name}: cannot read JSON file`).
   - A `ValueError` from `json.loads` ("is not JSON") becomes `MemberConfigError` too, keeping its current message
     text exactly (`member config: {path} is not JSON ({exc})`), so the stderr tests still match.
4. `validate_config(config, source)`: the top-level `not isinstance(config, dict)` check raises
   `MemberConfigError` instead of `Failed`, same message. The other checks in it stay `Failed`.
5. Do not touch `cmd_privacy_check`'s own `member.json must be a JSON object` check (it reports on a folder, not
   the loaded member; it stays exit 1), nor any other JSON reader in the file (compartments, seen files, logs).
6. Keep the stderr message one line in every case (the tests assert one line).

## Tests in `tests/crew_decisions_test.py` (write these first and watch them fail)

1. In `Compartments.test_member_and_private_bad_encoding_or_nesting`, keep both existing inputs (the 10,000-deep
   list and `b"\xff"`) and add, for both `member.json` and `private.json`:
   - `b"[]"` and `b"0"` (valid JSON, not an object);
   - `b"{not json"` (not JSON);
   - an object holding a deep list: `b'{"filters": ' + b"[" * 10000 + b"0" + b"]" * 10000 + b"}"`;
   - a list exactly `MAX_CONFIG_DEPTH + 1` deep (`b"[" * 65 + b"]" * 65`, built from `self.c.MAX_CONFIG_DEPTH`).
   Every one expects `where` to exit 2, one line of stderr containing `member config:`. Use `subTest` with a short
   label per input (not the bytes themselves; `data[:1]` collides for the two deep cases).
2. A test that a config exactly `MAX_CONFIG_DEPTH` deep is not refused for depth: write a valid `private.json`
   whose one extra key holds a list nested so the file's total depth is exactly `MAX_CONFIG_DEPTH`, and assert
   `where` does not exit 2 for depth (it should exit 0 as an ordinary member does; check the harness's normal
   `where` result in the same class and match it).
3. A unit test of `json_depth` (or `json_too_deep`): brackets inside strings and escaped quotes do not count
   (`'{"a": "[[[\\"[["}'` is depth 1), `"[]"` is 1, `"0"` is 0, `"[{}]"` is 2, a 100,000-deep text returns
   without `RecursionError`.
4. Update the two existing exit-1 assertions that the new rule moves to exit 2, and nothing else:
   - `test_a_broken_overlay_is_named` (`private.json` = `{not json`): exit 2, message unchanged.
   - `test_an_overlay_of_the_wrong_shape_is_named`: the `[1, 2]` case expects 2; the
     `filters.personal must be a string` case stays 1. Split the loop so each case states its own code.
   - Grep the other test files (`tests/*_test.py`) for any assertion on "is not JSON" or "must be a JSON object"
     through the member loader and move them by the same rule; `test_a_member_json_that_is_not_an_object_is_named`
     (privacy-check `--dir`) stays 1.

## `.ai/DECISIONS.md`

Append a section at the end, matching the file's style (a `## 2026-11-20: …` heading, bold-led bullets, full
sentences, no real people):

`## 2026-11-20: a member config Voice cannot read as a JSON object exits 2, on every Python (VF40)`

- **The rule:** what exits 2 and what exits 1, as in "The rule to implement" above.
- **Why a depth cap:** the JSON decoder's recursion limit differs between Python versions (3.12 raises on a
  10,000-deep list; 3.14 and 3.15 parse it), so the cap of 64 levels is checked on the text before parsing and
  the answer no longer depends on the interpreter. Real configs are 4 levels deep.

## Run (report each exit code)

Run the touched suite under every interpreter on this Mac, red first (before the code change), then green:

```
for py in /usr/bin/python3 ~/.pyenv/versions/3.10.22/bin/python3 ~/.pyenv/versions/3.12.14/bin/python3 /opt/homebrew/bin/python3.14 /opt/homebrew/bin/python3.15; do $py tests/crew_decisions_test.py > /tmp/vf40-$($py -c 'import sys;print("%d%d"%sys.version_info[:2])').log 2>&1; echo "$py $?"; done
```

Report each interpreter's exit code and the last 3 lines of its log. Then run `shellcheck` only if you touched a
shell file (you should not). Do not run `scripts/ci-local.sh`; Claude runs the gate.
