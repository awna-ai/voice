# VF30 fix-up 1: a declared folder that is a link still matches, and a queue folder linking into a fence is fenced

Branch `lane-v1/VF30`, on top of `cc20fb9`. Two findings from the security review. Two files:
`scripts/crew_decisions.py`, `tests/crew_decisions_test.py`.

## 1. `path_compartment(..., follow_last=False)` compares declared folders both ways (medium)

Today the `follow_last=False` branch compares the item, `real_root / name`, against each declared folder resolved
in its parent only (`safe_resolve(c, folder.parent, cwd)[0] / folder.name`). A declared folder that is itself a link
then never matches a queue folder inside it. Example: `client` declares `~/projects/client`, a link to
`~/projects/work/sub`; queues at `~/projects/client/queues` holding `app/queue.md`. On `main`, the item is labelled
`client`; on this branch it is labelled `work`, so `pending --compartment work` prints client's question.

- Fix: when `resolve` is true and `follow_last` is false, each declared folder contributes **both** roots:
  `safe_resolve(c, folder, cwd)[0]` and `safe_resolve(c, folder.parent, cwd)[0] / folder.name`. Keep the
  `follow_last=True` branch and the `resolve=False` branch exactly as they are. The match length stays
  `len(root.parts)` of the root that matched; the deepest match wins, as today.
- **Red first** (in `Compartments`, beside `test_pending_queue_labels_logical_folders_and_skips_fenced_links`, same
  `HOME` patch style): the layout above → the item is labelled `client`; `--compartment work` prints nothing.
  Also: the declared link points outside every declared folder (`~/projects/client → ~/data/client`) → still
  labelled `client`. Both fail on `cc20fb9`. The existing case (d) (a linked queue folder `app` that is itself
  declared in `client`) must still pass.

## 2. `queue_compartment` fails closed on its own (low)

`queue_compartment` labels `real_root / name` without resolving `name`; if `name` is a link into a fence (swapped
after `queue_files` checked it), `path_compartment`'s git-root fallback stats through it into the fence.

- Fix: in `queue_compartment`, after the root's `safe_resolve` and before the hub branch, return
  `{"compartment": "fenced"}` when `safe_resolve(c, real_root / name, os.getcwd())[1]` is true.
- **Red first:** call `self.c.queue_compartment(m, <queues>/swapped/queue.md)` directly, with `<queues>/swapped`
  a link to `~/garden-club/x` (the fence; build it under the patched `HOME`), once with `<queues>` a hub path and
  once not. Assert `{"compartment": "fenced"}` and that no `os.lstat`/`os.stat` call (spy with
  `patch.object(self.c.os, "lstat", wraps=os.lstat)` and the same for `stat`) lands under the fence.

## Rules

- Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
  `~/.codex/skills/security-and-hardening/SKILL.md`. Never stat, list or open anything inside a fenced folder.
- Red first for each; note each red failure. Touch nothing else.
- Run with Python 3.12 (pyenv's 3.12 under your home): `tests/crew_decisions_test.py`,
  `tests/crew_hardening_test.py`, `tests/crew_sources_test.py`. At most 2 processes at a time.
- Invented fixtures only. Commit nothing. Summarise the red failures and the green runs.
