# VF17: the timestamp parity tests hold on Python 3.15

Card `L9GqfFTp`. Spec: lane V's spec file § VF17. Code and tests. Codex was at its usage limit, so V1
implemented this brief directly; the tests went into `tests/crew_hardening_test.py`, which already
imports `crew_decisions` and holds the input checks (step 2 below).

## Why

`builder/ingest/tests/claudeai_utc.rs` checks that Voice's Rust `claudeai::utc` and the Python
`utc()` in `scripts/crew_decisions.py` (a thin wrapper on `datetime.fromisoformat`) accept and
refuse the same timestamps. How `fromisoformat` reads some forms depends on the Python version,
probed on Mimi with each list in the test:

| Python | Differs from the test's expectation |
|---|---|
| 3.9.6, 3.10.22 | refuses six `BOTH` forms (fractions of 1, 7, 9 or 12 digits) and eight `PYTHON_ONLY` forms: only 3.11 and newer read full ISO 8601 |
| 3.12.14 | nothing |
| 3.15.0 | **accepts** `2026-03-01T24:00:00` (reads it as `2026-03-02T00:00:00`); **refuses** `2026-03-01T12:00:00.Z` and `2026-03-01T12:00.5` |

So with Homebrew's 3.15 first on `PATH`, `both_refuse_the_same_bad_forms` and
`forms_only_python_reads_are_refused` fail, on `main` too.

## What to change

### 1. `scripts/crew_decisions.py`: `utc()` refuses hour 24 on every Python

Python 3.15 turns hour 24 into the next day's midnight, so after parsing it can only be seen as a
midnight whose day is not the day the text starts with. Add, after the `tzinfo` lines and before
the `return`:

```python
    if parsed.time() == time() and not when.startswith(day_forms(parsed.date())):
        raise Failed(f"{when!r} is not a timestamp: the export's shape changed, run claudeai inspect")
```

(`time` joins the existing `from datetime import ...` line; keep that line's names sorted.) Add the
helper just above `utc()`:

```python
def day_forms(day):
    """Every way fromisoformat writes a day at the start of a timestamp."""
    year, week, weekday = day.isocalendar()
    forms = (day.isoformat(), f"{day.year:04d}{day.month:02d}{day.day:02d}",
             f"{year:04d}-W{week:02d}-{weekday}", f"{year:04d}W{week:02d}{weekday}")
    return forms + ((f"{year:04d}-W{week:02d}", f"{year:04d}W{week:02d}") if weekday == 1 else ())
```

Nothing else in `crew_decisions.py` changes.

### 2. `tests/crew_hardening_test.py`: pin the hour-24 rule

Add one test class, `Timestamps`, after the file's other `unittest.TestCase` classes, calling
`crew.utc` (the file already imports `crew_decisions as crew`):

- `test_hour_24_is_refused`: `utc("2026-03-01T24:00:00")`, `utc("2026-03-01T24:00")` and
  `utc("2026-03-01T24:00:00Z")` each raise `crew_decisions.Failed`.
- `test_a_real_midnight_is_kept`: `utc("2026-03-02T00:00:00")` returns
  `"2026-03-02T00:00:00.000Z"`, and `utc("2026-03-02T00:00:00+01:00")` returns
  `"2026-03-01T23:00:00.000Z"`.
- `test_every_day_form_at_midnight_is_kept`, decorated
  `@unittest.skipIf(sys.version_info < (3, 11), "only 3.11 and newer read these forms")`: each of
  `"20260302T000000"`, `"2026-W10-1T00:00:00"`, `"2026W101T000000"`, `"2026-W10T00:00"`,
  `"2026-W10-3T00:00"` and `"0001-01-01T00:00:00"` is accepted, and the first five give
  `"2026-03-02T00:00:00.000Z"` except `"2026-W10-3T00:00"`, which gives `"2026-03-04T00:00:00.000Z"`;
  `"0001-01-01T00:00:00"` gives `"0001-01-01T00:00:00.000Z"`. Use `subTest` per form.

Literal expected values only; never compute them with `datetime`.

### 3. `builder/ingest/tests/claudeai_utc.rs`: no form whose reading changed between Pythons

- Move `"2026-03-01T12:00:00.Z"` and `"2026-03-01T12:00.5"` out of `PYTHON_ONLY` into a new list
  placed right after it:

  ```rust
  /// Forms Voice refuses that some Python versions read and others don't (3.15 refuses both), so only
  /// Voice's refusal is checked.
  const PYTHON_VARIES: &[&str] = &["2026-03-01T12:00:00.Z", "2026-03-01T12:00.5"];
  ```

- Add a test `forms_python_reads_differently_are_refused` that, with no Python involved, asserts
  `utc(when) == None` for every `PYTHON_VARIES` entry, with the form in the message as the other
  tests do. It never skips.
- Add a helper below `python()`:

  ```rust
  /// Only Python 3.11 and newer read full ISO 8601; older ones refuse forms Voice shares with them.
  fn python_reads_full_iso() -> bool {
      let out = Command::new("python3")
          .args(["-I", "-c", "import sys; print(sys.version_info >= (3, 11))"])
          .output()
          .unwrap();
      let full = String::from_utf8_lossy(&out.stdout).trim() == "True";
      if !full {
          eprintln!("skipped: python3 is older than 3.11, which reads only part of ISO 8601");
      }
      full
  }
  ```

- In `shared_forms_agree_with_python`, `both_refuse_the_same_bad_forms` and
  `forms_only_python_reads_are_refused`, the guard becomes
  `if !parity_tools_available() || !python_reads_full_iso() { return; }`. It does not panic under
  `VOICE_REQUIRE_PARITY=1`: 3.9 is still a supported Python (README, CONTRIBUTING).
- Leave `"2026-03-01T24:00:00"` in `NEITHER`: after step 1 every Python refuses it.
- No other list entry, test or assertion changes.

## What not to change

Any other file. `README.md` and `CONTRIBUTING.md` keep "Python 3.9 or newer".

## Checks (all must pass; run each yourself and report each exit code)

`P315=/opt/homebrew/bin`, `P312=$HOME/.pyenv/versions/3.12.14/bin`, `P39=/usr/bin` (each has a
`python3`).

```sh
cargo fmt --check
for d in "$P315" "$P312" "$P39"; do
  PATH="$d:$PATH" VOICE_REQUIRE_PARITY=1 cargo test --locked -p voice-ingest --test claudeai_utc -- --test-threads=2 || echo "FAIL $d"
  PATH="$d:$PATH" python3 tests/crew_hardening_test.py || echo "FAIL py $d"
done
cargo clippy -p voice-ingest --all-targets --locked -- -D warnings
```

Under `P39` the three parity tests print the `skipped:` line and pass. Do not commit; leave the
changes in the working tree.
