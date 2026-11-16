# PB8.9c1: Radio inside Voice.app

Built Claude-direct under the fallback rule (Codex is at its usage limit until 2026-11-18); this brief
records the decisions it was built from. Split from PB8.9c (plan
`2026-11-12-pb8-9-one-assistant-many-computers.md`, step 2.1); PB8.9c2 makes Voice use the binary.

Radio `v0.2.0` (the release R.7 made; socket protocol 1.7 with the link ops) publishes
`radio-v0.2.0-macos-arm64.tar.gz`, `radio-v0.2.0-macos-x86_64.tar.gz` and `SHA256SUMS`. Each tarball
holds `radio-v0.2.0-macos-<arch>/radio` (signed ad hoc, made to ship inside an app) and its MIT
`LICENSE`. Radio's README, *For app developers*: pin a tag, check the sums, `lipo` the two for a
universal app.

## Decisions

1. **The pin** is `builder/install/radio-release.txt`: a `tag` line, then one SHA-256 line per
   architecture, copied from the release's `SHA256SUMS`. Moving to a newer Radio is an edit to this file.
2. **`scripts/fetch-radio.sh <pin file> <new folder> arm64|x86_64...`** downloads each pinned tarball
   with `curl --proto '=https' --tlsv1.2` from the release URL, refuses one whose SHA-256 differs,
   extracts only the two named members, refuses a missing or linked `radio` or `LICENSE` and a binary
   that isn't for its architecture, and writes `radio` (joined with `lipo` when more than one
   architecture) and `LICENSE` into a folder that must not exist yet. Each refusal is one
   `fetch-radio:` line, exit 1; bad arguments exit 2.
3. **`scripts/build-mac-app.sh`** fetches for the host's architecture, or both with `--universal`,
   stages `Voice.app/Contents/MacOS/radio` (755, shipped unchanged with Radio's own ad-hoc signature: the Verifier's should-fix, `56710cb`, supersedes
   signing it like the `voice` command) and Radio's
   licence at `Contents/Resources/radio/LICENSE`, re-signs the app, and checks that both commands run
   on every architecture the app does (one helper).
4. **`builder/install/install.sh`** treats a missing, linked, non-executable or wrong-architecture
   `radio` as a damaged copy, through the same helper as the `voice` command.
5. Nothing runs Radio yet; `radio install` is PB8.9c2's, at first pairing.

## Tests

`tests/fetch_radio_test.sh` (a stand-in `curl` serves invented releases; nothing touches the network):
one architecture, universal, a tampered tarball, a binary for the other chip, a missing or linked
binary, no licence, a failed download, bad pin files, an existing folder, bad arguments, and the
repo's own pin being well formed. `tests/mac_install_test.sh`: the fake app carries a `radio`; a
missing, linked, non-executable or other-chip one is refused. Both run in `scripts/ci-local.sh`, and
both scripts are shellchecked.
