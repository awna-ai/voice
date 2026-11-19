# VF30 fix-up 2: the Verifier's should-fixes

Branch `lane-v1/VF30`, on top of `d0523d7`. The Verifier passed `cc20fb9` + fix-up 1 with should-fixes. Two files:
`scripts/crew_decisions.py`, `tests/crew_decisions_test.py`. Every change below is red first unless it says a test
pins today's behaviour.

## 1. A linked queue folder under a non-hub root is labelled logically (pin, no code change)

Decided: a queue folder that is a link, under a queues folder that is **not** a hub path, takes the compartment of
where it sits (its logical path), never where the link points, as the hub rule and a linked `queue.md` already do.
The fence is the hard boundary and still holds: a link into a fence is skipped by `queue_files` and labelled `fenced`.

- **Pin it** (in `Compartments`, beside `test_pending_queue_labels_logical_folders_and_skips_fenced_links`, same
  `HOME` patch style): queues at `<home>/projects/work/queues` (inside `work`'s declared folder, not a hub path), a
  folder `app` that is a link to `<home>/projects/client/q` (holding `queue.md` with one `CONTROL-Q`).
  `pending --compartment work` prints it labelled `work`; `--compartment client` prints nothing.
- **Mutant to kill:** in `queue_compartment`, change `follow_last=False` to `follow_last=True`; the new test must
  fail. Note the failure message.

## 2. A run with no compartments map keeps the seen file as `main` did (S3, N5)

Today, with `m.compartments is None`, a seen file that isn't a list exits 1 (`main` read `{}` as empty), and the write
drops any `[compartment, id]` entries.

- With no map, read the file exactly as `main` did: `data = json.loads(seen_path.read_text())` with no new error
  handling, then: if `data` is a list, `seen` is the set of its entries that are not lists, and `pairs` is the list
  of its entries that are lists, kept in file order; otherwise `seen = set(data)` and `pairs = []` (so `{}` is empty,
  as on `main`). Pairs are never counted as seen with no map.
- With no map, write `json.dumps(sorted(seen | {i["id"] for i in new}) + pairs)`. With no pairs this is byte for byte
  what `main` writes (`test_pending_without_compartments_preserves_output_bytes` must pass unchanged).
- With a map, keep today's read, checks and write exactly.
- **Red first:** no map, seen `{}` → exit 0, prints the item, file becomes the sorted id list. No map, seen
  `["x:1", ["work", "a:t"]]` → the item `a:t` is printed (a pair doesn't hide it), and the file afterwards is
  `["a:t", "x:1", ["work", "a:t"]]`.

## 3. A deeply nested seen file is "not readable" (N6)

With a map, `json.loads` of `"[" * 100000 + "]" * 100000` raises `RecursionError`, which today escapes. Catch
`RecursionError` beside `OSError, ValueError` in the map branch's read, so it raises
`Failed("pending: the seen file is not readable")`. **Red first:** that file → exit 1, stderr exactly
`crew_decisions: pending: the seen file is not readable\n`, file unchanged.

## 4. `pending` never prints an item labelled `fenced` (N8)

In `cmd_pending`'s `new` filter add `and i.get("compartment") != "fenced"`. **Red first:** with a map, an unscoped
run where `queue_compartment` is patched (`patch.object(self.c, "queue_compartment", return_value={"compartment":
"fenced"})`) and the queues hold one `CONTROL-Q` → prints nothing and `seen.json` is not written.

## 5. Kill the surviving mutants (S4)

Add a test for each, run the mutant by hand (`cp` the file aside, edit, run, restore with `cp`, check with `cmp`;
never `git checkout --`), and note the failure. If you can prove a mutant is equivalent (no input tells it apart),
leave the code as it is, never remove a guard, and say why in your summary.

- (a) `PROJECT_COMPARTMENT.fullmatch` → `.match` in `cmd_pending`: `--compartment "p:x y"` → exit 2, the unknown
  message, `seen.json` untouched. Add it to `test_pending_compartment_refusals_do_not_touch_seen`'s cases.
- (b) The unknown-name refusal moved after the seen read: seen file holding `{}` + `--compartment nope` → exit 2
  (not 1), the unknown message.
- (c) The fence check in `path_compartment`'s `follow_last=False` branch, one case per clause, calling
  `self.c.path_compartment(m.compartments, <path>, os.getcwd(), follow_last=False)` under the patched `HOME`:
  (i) `<home>/garden-club/x` (the parent is fenced) → `"fenced"`; (ii) `<home>/garden-club` itself (only the lexical
  check sees it) → `"fenced"`; (iii) a fence configured through a link: add `<home>/alias/secret` to the map's
  `fenced`, make `<home>/alias` a link to `<home>/real`, and ask for `<home>/real/secret` (only the resolved fence
  form sees it) → `"fenced"`. Each clause's mutant (drop that clause) fails its case.
- (d) The fenced-root check in `queue_compartment` (`if fenced: return {"compartment": "fenced"}`): call
  `queue_compartment` with a queues root inside the fence. If the next check makes this mutant equivalent, say so.
- (e) In `queue_compartment`'s hub detection, `not blocked` dropped, and `fold(...)` dropped on both sides:
  - `fold`: configure the hub path as `<home>/.HUB/PROJECTS` and read queues from `<home>/.hub/projects`, folder
    `my app` (a real folder) → labelled `p:my app` (without `fold` it falls to the hub owner's compartment). This
    must hold on a case-sensitive filesystem too (a missing path resolves lexically).
  - `not blocked`: find an input that tells it apart, or prove it equivalent.
- (f) Make the fence probe spies stronger. In `test_pending_queue_labels_logical_folders_and_skips_fenced_links` and
  `test_queue_compartment_refuses_swapped_link_into_fence`, wrap both `os.lstat` and `os.stat`, and assert no
  call's path, made absolute, is under the fence's absolute path or under `os.path.realpath(fence)` (computed
  before the spies start).

## Rules

- Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
  `~/.codex/skills/security-and-hardening/SKILL.md`. This is a privacy filter: never stat, list or open anything
  inside a fenced folder.
- Match the surrounding code: no new helpers, no comments that restate the code. Touch nothing else.
- Run with Python 3.12 (`~/.pyenv/versions/3.12.14/bin/python3`): `tests/crew_decisions_test.py`,
  `tests/crew_hardening_test.py`, `tests/crew_sources_test.py`, `tests/crew_extra_sources_test.py`. At most 2
  processes at a time.
- Public repo: invented fixtures only, no real person's name or data, no fixed paths. Commit nothing. Summarise:
  each red failure, each mutant and its failing test (or why it is equivalent), the green runs, and anything you
  couldn't do as written.
