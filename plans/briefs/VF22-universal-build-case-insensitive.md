# VF22: the universal Mac build works on a case-insensitive disk

Built Claude-direct under the fallback rule (Codex is at its usage limit until 2026-11-18); this brief
records the decisions it was built from. Filed from PB8.9c1's delta Verifier (should-fix 1).

`scripts/build-mac-app.sh` makes the package folder `$staging/Voice`, then the `--universal` path wrote
the joined `voice` command to `$staging/voice` with `lipo -create -output`. On a stock Mac
(case-insensitive APFS) those are one path, so `lipo` stopped every universal build with
`open() failed, errno=21 (Is a directory)`.

## Decisions

1. **The joined command gets its own folder:** `$staging/cli/voice`. It stays inside `$staging`, so the
   build's `EXIT` trap still removes it. The single-architecture path is unchanged.
2. **A test that runs the whole build, `tests/build_mac_app_test.sh`,** run by `scripts/ci-local.sh` and
   checked by its shellcheck line. It copies the files the build reads into a scratch git checkout and
   runs the real script with stand-ins on `PATH`: `cargo` (builds and `metadata`), `pnpm` (`--version`,
   and a `tauri build` that makes a signed `Voice.app`), `rustup` (both targets installed) and `curl`
   (serves an invented Radio release whose pin the test writes). Each stand-in makes a real Mach-O with
   `cc` for the asked architectures, so `lipo`, `codesign` and `PlistBuddy` check real files. Both builds,
   one architecture and `--universal`, must produce a zip whose `Voice.app` holds the app, `voice` and
   `radio` for every architecture, `install.sh`, and Radio's licence, and must leave the tracked files
   unchanged. It skips off macOS or without `cc`, `lipo` and `codesign`, like `fetch_radio_test.sh`.

## Verification

Red first: on the old script the universal build failed with `lipo: open() failed, errno=21`. Green
after decision 1. Mutations (each on a `cp` copy, restored and `cmp`-checked): joining only one
architecture into `voice` fails the build's own architecture check; dropping Radio's licence copy fails
the test's licence check.
