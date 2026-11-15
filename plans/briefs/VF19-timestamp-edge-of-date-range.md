# VF19: a timestamp at the edge of the date range gets the plain error, not a traceback

Card `NK93XOuu`. Spec: lane V's spec file § VF19. Code and tests. Codex was at its usage limit, so V1
implemented this brief directly. It builds on VF17, which changed the same function.

## Why

`scripts/crew_decisions.py` `utc()` turns an export timestamp into UTC with `astimezone`. Near the
ends of the date range that raises `OverflowError`: `0001-01-01T00:00:00+01:00` (UTC is the year 0)
and `9999-12-31T23:00:00-05:00` (UTC is the year 10000), on Python 3.9, 3.12, 3.14 and 3.15 (probed
on Mimi). `main()`
catches `Failed`, `OSError`, `ValueError`, `KeyError` and `RecursionError`, not `OverflowError`, so
`claudeai titles` on such an export prints a traceback instead of the plain "is not a timestamp"
error. Voice's Rust reader refuses both forms already: they are in the parity test's `NEITHER` list,
whose Python helper catches `OverflowError` itself, which hid this.

## What to change

### 1. `scripts/crew_decisions.py`: `utc()` refuses an out-of-range timestamp

Move the whole conversion inside the existing `try`, add `OverflowError` to the caught errors, and
let the hour-24 check (VF17) raise `ValueError` so the message is written once:

```python
    try:
        parsed = datetime.fromisoformat(when.replace("Z", "+00:00"))
        if parsed.tzinfo is None:
            parsed = parsed.replace(tzinfo=timezone.utc)
        if parsed.time() == time() and not when.startswith(day_forms(parsed.date())):
            raise ValueError("hour 24")
        return parsed.astimezone(timezone.utc).isoformat(timespec="milliseconds").replace("+00:00", "Z")
    except (AttributeError, TypeError, ValueError, OverflowError):
        raise Failed(f"{when!r} is not a timestamp: the export's shape changed, run claudeai inspect")
```

Don't widen `main()`'s handler: an `OverflowError` anywhere else is a bug that should still show.

### 2. Tests, red first

- `tests/crew_hardening_test.py`, class `Timestamps`: `test_a_timestamp_past_the_date_range_is_refused`
  checks both forms raise `Failed` with "is not a timestamp".
- `tests/crew_sources_test.py`, class `Review`: `test_a_timestamp_past_the_date_range_is_a_plain_error`
  runs `claudeai titles` on an invented export whose `created_at` is `9999-12-31T23:00:00-05:00`, and
  expects exit 1, one `crew_decisions:` error line and no script frame (`assert_cli_error`), "not a
  timestamp" in stderr, and no review file written.

## Done when

- Both new tests fail before step 1 (`OverflowError`, exit status) and pass after it, on Python 3.9,
  3.12, 3.14 and 3.15.
- Taking `OverflowError` back out of the tuple turns them red.
- `bash scripts/ci-local.sh` passes.
