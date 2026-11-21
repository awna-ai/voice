#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
tmp=$(mktemp -d)
cleanup() {
  git -C "$repo" worktree remove --force "$tmp/wt" >/dev/null 2>&1 || true
  rm -rf "$tmp"
}
trap cleanup EXIT
real_cargo=$(command -v cargo || true)
git -C "$repo" worktree add --quiet --detach "$tmp/wt" HEAD
cp "$repo/scripts/ci-local.sh" "$tmp/wt/scripts/ci-local.sh"
cd "$tmp/wt"
commit() { git -c core.hooksPath=/dev/null -c commit.gpgsign=false -c user.name=t -c user.email=t@example.invalid commit --allow-empty -qm "$1"; }
# Stub every suite in the disposable checkout; scan the caller's real sources below.
for suite in tests/*_test.py tests/*_test.sh; do
  # The child stub, not this test, expands the marker variable.
  # shellcheck disable=SC2016
  case "$suite" in
    *.py) printf 'import os\nwith open(os.environ["CI_LOCAL_MARKER"], "a") as marker: marker.write("%s\\n")\n' "$suite" > "$suite" ;;
    *.sh) printf '#!/bin/bash\nprintf "%%s\\n" "%s" >> "$CI_LOCAL_MARKER"\n' "$suite" > "$suite" ;;
  esac
done
git add scripts/ci-local.sh tests
commit base
mkdir "$tmp/bin"
export CI_LOCAL_MARKER="$tmp/marker" CI_LOCAL_REAL_CARGO="$real_cargo"
for tool in shellcheck pnpm; do
  cat > "$tmp/bin/$tool" <<'STUB'
#!/bin/bash
printf '%s\n' "${0##*/}" >> "$CI_LOCAL_MARKER"
STUB
  chmod +x "$tmp/bin/$tool"
done
if [ -n "$real_cargo" ]; then
  cat > "$tmp/bin/cargo" <<'STUB'
#!/bin/bash
if [ "${1:-}" = metadata ]; then exec "$CI_LOCAL_REAL_CARGO" "$@"; fi
printf 'cargo %s\n' "$*" >> "$CI_LOCAL_MARKER"
STUB
  chmod +x "$tmp/bin/cargo"
else
  echo 'ci_local_test: cargo missing, skipping cargo stub; checking Rust skip notes'
fi
export PATH="$tmp/bin:$PATH"
fail() { echo "ci_local_test: $*" >&2; exit 1; }
run_list() {
  : > "$CI_LOCAL_MARKER"
  status=0
  "${test_bash:-bash}" scripts/ci-local.sh "$@" > "$tmp/out" 2> "$tmp/err" || status=$?
  [ ! -s "$CI_LOCAL_MARKER" ] || fail '--list executed a step'
}
list() {
  run_list "$@"
  [ "$status" = 0 ] || { cat "$tmp/err" >&2; fail "--list exited $status"; }
  if [ -z "$real_cargo" ]; then
    grep -Fx 'ci-local: cargo not found, skipping the Rust checks (install Rust from https://rustup.rs)' "$tmp/err" >/dev/null || fail 'missing cargo skip note'
  fi
}
has() { grep -Fx -- "$1" "$tmp/out" >/dev/null || fail "missing step: $1"; }
no_tests() { if grep -F 'cargo test ' "$tmp/out"; then fail 'unexpected cargo test'; fi; }
change() {
  mkdir -p "$(dirname "$1")"
  printf '\n// ci-local test\n' >> "$1"
  git add -- "$1"
  commit change
}
reset() { git reset -q --hard HEAD~1; }
touched() { list --tier touched --base HEAD~1 --list; }
usage='Usage: scripts/ci-local.sh [--tier full|touched] [--base <rev>] [--list]'
# Start with usage so the old script fails before any Rust work can run.
for args in '--tier bogus' '--base x' '--unknown' '--tier' '--tier touched --base'; do
  # The arguments here are fixed test literals, never paths.
  # shellcheck disable=SC2086
  run_list $args --list
  [ "$status" = 2 ] || { cat "$tmp/err" >&2; fail "usage: expected exit 2, got $status"; }
  grep -Fx "$usage" "$tmp/err" >/dev/null || fail 'missing usage'
done
echo 'ci_local_test: usage passed'

python_suites='crew_decisions crew_sources crew_extra_sources crew_labels crew_hardening crew_documents crew_rounds slack_mine slack_select crew_personas growth_loop_spec claude_sandbox_probe read_fence_probe'
shellcheck_line='shellcheck scripts/ci-local.sh bootstrap/install.sh tests/crew_install_test.sh scripts/build-mac-app.sh builder/install/install.sh tests/mac_install_test.sh scripts/fetch-radio.sh tests/fetch_radio_test.sh tests/build_mac_app_test.sh tests/ci_local_test.sh scripts/verify-main.sh scripts/release-check.sh tests/verify_main_test.sh'
prefix='VOICE_REQUIRE_PARITY=1 '
if [ "$(uname -s)" = Darwin ]; then prefix+='VOICE_REQUIRE_SANDBOX=1 '; fi
cat > "$tmp/trees" <<'GOLDEN'
cargo tree --locked --target all -p voice-cli -e features,normal,build,dev -i voice-agent
cargo tree --locked --target all -p voice-cli -e features,normal,build -i voice-agent
cargo tree --locked --target all -p voice-app -e features,normal,build -i voice-agent
cargo tree --locked --target all -p voice-cli -e features,normal,build -i voice-agent
cargo tree --locked --target all -p voice-app -e features,normal,build -i voice-agent
cargo tree --locked --target all -p voice-app --features claude-subscription -e features,normal,build -i voice-agent
GOLDEN
{
  for suite in $python_suites; do echo "python3 tests/${suite}_test.py"; done
  echo 'bash tests/crew_install_test.sh'
  echo 'bash tests/mac_install_test.sh'
  echo 'bash tests/fetch_radio_test.sh'
  echo 'bash tests/build_mac_app_test.sh'
  echo 'bash tests/ci_local_test.sh'
  echo 'bash tests/verify_main_test.sh'
  echo "$shellcheck_line"
} > "$tmp/prefix"
cp "$tmp/prefix" "$tmp/golden"
if [ -n "$real_cargo" ]; then
  {
    echo 'cargo fmt --check'
    echo 'cargo clippy --all-targets --locked -- -D warnings'
    echo "${prefix}cargo test --locked -p voice-agent -p voice-api -p voice-cli --features claude-subscription"
    echo "${prefix}cargo test --locked"
    cat "$tmp/trees"
    echo 'cargo clippy --locked -p voice-agent -p voice-api -p voice-cli -p voice-app --all-targets --features claude-subscription -- -D warnings'
    echo 'cargo clippy --locked -p voice-cli -p voice-app -- -D warnings'
    echo 'git diff --exit-code -- builder/ui/src/types/generated'
    for target in x86_64-pc-windows-msvc x86_64-unknown-linux-gnu; do
      if rustup target list --installed 2>/dev/null | grep -qx "$target"; then
        echo "cargo check --locked -p voice-keystore --all-targets --target $target"
      fi
    done
  } >> "$tmp/golden"
fi
for step in 'install --frozen-lockfile' typecheck test build; do echo "pnpm -C builder/ui $step"; done >> "$tmp/golden"
list --list
cp "$tmp/out" "$tmp/full"
list --tier full --list
grep -Fx 'bash tests/build_mac_app_test.sh' "$tmp/out" >/dev/null || fail 'RED full gate: missing bash tests/build_mac_app_test.sh'
cmp "$tmp/full" "$tmp/out" || fail 'explicit full differs'
if [ "$(uname -s)" = Darwin ] && [ -n "$real_cargo" ]; then
  diff -u "$tmp/golden" "$tmp/out" || fail 'full gate order changed'
else
  head -n 20 "$tmp/out" > "$tmp/actual-prefix"
  cmp "$tmp/prefix" "$tmp/actual-prefix" || fail 'full gate prefix changed'
fi
echo 'ci_local_test: full golden list passed'

# Off a Mac, a gate holding a cargo test refuses with 75 before any step runs; a Mac, or a gate
# with no cargo test, runs its steps.
mkdir "$tmp/os"
# The screen version probe builds the step list; only screen steps mark execution.
cat > "$tmp/os/pnpm" <<'STUB'
#!/bin/bash
if [ "${1:-}" = --version ]; then
  echo 10.0.0
else
  printf '%s\n' pnpm >> "$CI_LOCAL_MARKER"
fi
STUB
chmod +x "$tmp/os/pnpm"
gate_on() { # <what uname -s prints> <ci-local args...>
  printf '#!/bin/bash\necho %s\n' "$1" > "$tmp/os/uname"
  chmod +x "$tmp/os/uname"
  shift
  : > "$CI_LOCAL_MARKER"
  status=0
  PATH="$tmp/os:$PATH" bash scripts/ci-local.sh "$@" > "$tmp/out" 2> "$tmp/err" || status=$?
}
if [ -n "$real_cargo" ]; then
  gate_on Linux
  [ "$status" = 75 ] || fail "off-Mac full gate: expected exit 75, got $status"
  [ ! -s "$CI_LOCAL_MARKER" ] || fail 'off-Mac full gate executed a step'
  grep -Fx "ci-local: cargo test needs a Mac (the agent tests run Claude Code in the Mac's own sandbox) and this host is Linux; exit 75 tells remote-gate.sh to try the next host" "$tmp/err" >/dev/null || fail 'off-Mac full gate missing refusal note'

  gate_on Linux --list
  [ "$status" = 0 ] || fail "off-Mac list: expected exit 0, got $status"
  grep -F 'cargo test --locked' "$tmp/out" >/dev/null || fail 'off-Mac list missing cargo test'

  gate_on Darwin
  [ "$status" != 75 ] || fail 'Mac full gate refused with exit 75'
  [ -s "$CI_LOCAL_MARKER" ] || fail 'Mac full gate ran no steps'

  change tests/slack_mine_test.py
  gate_on Linux --tier touched --base HEAD~1
  reset
  [ "$status" != 75 ] || fail 'off-Mac touched gate without crates refused with exit 75'
  [ -s "$CI_LOCAL_MARKER" ] || fail 'off-Mac touched gate without crates ran no steps'

  change builder/agent/src/egress.rs
  gate_on Linux --tier touched --base HEAD~1
  reset
  [ "$status" = 75 ] || fail "off-Mac touched gate with a crate: expected exit 75, got $status"
  [ ! -s "$CI_LOCAL_MARKER" ] || fail 'off-Mac touched gate with a crate executed a step'
  echo 'ci_local_test: off-Mac refusal passed'
else
  echo 'ci_local_test: cargo missing, skipping the off-Mac refusal'
fi

# Report every new regression before failing, so each missing mapping has a red.
regressions=0
expect_step() {
  if ! grep -Fx -- "$1" "$tmp/out" >/dev/null; then
    echo "ci_local_test: RED $case_name: missing $1" >&2
    regressions=$((regressions + 1))
  fi
}
expect_crate() {
  if [ -n "$real_cargo" ] && ! grep -E "cargo test .* -p $1( |$)" "$tmp/out" >/dev/null; then
    echo "ci_local_test: RED $case_name: missing crate $1" >&2
    regressions=$((regressions + 1))
  fi
}
for path in builder/agent/src/sandbox.rs builder/agent/sandbox/claude-code.sb bootstrap/install.sh builder/install/radio-release.txt; do
  case_name=$path
  change "$path"
  touched
  case "$path" in
    builder/agent/*)
      expect_step 'python3 tests/claude_sandbox_probe_test.py'
      expect_step 'python3 tests/read_fence_probe_test.py'
      expect_crate voice-agent ;;
    bootstrap/*) expect_step 'bash tests/crew_install_test.sh'; expect_crate voice-core ;;
    builder/install/*) expect_step 'bash tests/mac_install_test.sh'; expect_step 'bash tests/fetch_radio_test.sh'; expect_step 'bash tests/build_mac_app_test.sh' ;;
  esac
  reset
done
for path in scripts/verify-main.sh scripts/release-check.sh tests/verify_main_test.sh scripts/slack_select.py scripts/fetch-radio.sh scripts/build-mac-app.sh tests/build_mac_app_test.sh tests/slack_mine_test.py docs/spec/growth-loop.md builder/grow/tests/golden/case.json; do
  case_name=$path
  change "$path"
  touched
  if cmp -s "$tmp/full" "$tmp/out"; then
    echo "ci_local_test: RED $case_name: unexpected full gate" >&2
    regressions=$((regressions + 1))
  fi
  case "$path" in
    scripts/verify-main.sh|scripts/release-check.sh)
      expect_step 'bash tests/verify_main_test.sh'
      expect_step 'bash tests/crew_install_test.sh'
      no_tests ;;
    tests/verify_main_test.sh)
      expect_step 'bash tests/verify_main_test.sh'
      no_tests ;;
    scripts/slack_select.py)
      expect_step 'bash tests/crew_install_test.sh'
      for suite in $python_suites; do expect_step "python3 tests/${suite}_test.py"; done ;;
    scripts/fetch-radio.sh)
      expect_step 'bash tests/fetch_radio_test.sh'
      expect_step 'bash tests/crew_install_test.sh'
      expect_step 'bash tests/build_mac_app_test.sh' ;;
    scripts/build-mac-app.sh|tests/build_mac_app_test.sh)
      expect_step 'bash tests/build_mac_app_test.sh' ;;
    tests/slack_mine_test.py)
      expect_step 'python3 tests/slack_mine_test.py'
      if grep -Fx 'bash tests/crew_install_test.sh' "$tmp/out" >/dev/null; then
        echo "ci_local_test: RED $case_name: unexpected crew installer test" >&2
        regressions=$((regressions + 1))
      fi ;;
    docs/spec/*|builder/grow/tests/golden/*) expect_step 'python3 tests/growth_loop_spec_test.py' ;;
  esac
  reset
done
case_name='clean tree'
list --tier touched --base HEAD --list
expect_step 'bash tests/ci_local_test.sh'
short_base=$(git rev-parse --short HEAD)
if ! grep -Fx "ci-local: no changed paths since $short_base, running only the repo-wide scans" "$tmp/err" >/dev/null; then
  echo 'ci_local_test: RED clean tree: missing no-changed-paths note' >&2
  regressions=$((regressions + 1))
fi
[ "$regressions" = 0 ] || fail "$regressions fixup assertions failed"
echo 'ci_local_test: fixup mappings, always self-test and empty change set passed'

panel_test() {
  change builder/panel-runner/src/lib.rs
  touched
  if [ -n "$real_cargo" ]; then
    has "${prefix}cargo test --locked -p voice-api -p voice-app -p voice-cli -p voice-panel-runner"
    if grep -F -- '-p voice-core' "$tmp/out"; then fail 'picked unrelated voice-core'; fi
  fi
  reset
}
panel_test
change builder/app/build.rs
touched
if [ -n "$real_cargo" ]; then
  has "${prefix}cargo test --locked -p voice-app"
  has 'cargo clippy --locked -p voice-app -- -D warnings'
fi
reset
change builder/keystore/src/lib.rs
touched
if [ -n "$real_cargo" ]; then
  for target in x86_64-pc-windows-msvc x86_64-unknown-linux-gnu; do
    line="cargo check --locked -p voice-keystore --all-targets --target $target"
    if ! grep -Fx "$line" "$tmp/out" >/dev/null; then
      grep -Fx "ci-local: the $target target is missing, skipping its check (rustup target add $target)" "$tmp/err" >/dev/null || fail 'missing target check or skip'
    fi
  done
fi
reset
change .ai/ci-local-test.md
touched
{
  echo "$shellcheck_line"
  echo 'bash tests/ci_local_test.sh'
  if [ -n "$real_cargo" ]; then
    echo 'cargo fmt --check'
    cat "$tmp/trees"
    echo 'git diff --exit-code -- builder/ui/src/types/generated'
  fi
} > "$tmp/always"
cmp "$tmp/always" "$tmp/out" || fail 'always scans differ'
reset
change builder/ui/src/App.vue
touched
for step in 'install --frozen-lockfile' typecheck test build; do has "pnpm -C builder/ui $step"; done
no_tests
reset
for path in newpart/x.txt Cargo.toml crew/_template/member.json scripts/ci-local.sh; do
  change "$path"
  touched
  cmp "$tmp/full" "$tmp/out" || fail "gap did not run full: $path"
  grep -F "$path" "$tmp/err" >/dev/null || fail 'fallback did not name path'
  if [ "$path" = scripts/ci-local.sh ]; then
    grep -F 'is the gate itself' "$tmp/err" >/dev/null || fail 'gate fallback reason'
  fi
  reset
done
# This file name deliberately contains literal shell syntax.
# shellcheck disable=SC2016
change 'builder/core/src/a b;$(touch PWNED).rs'
touched
if ! cmp -s "$tmp/full" "$tmp/out" && [ -n "$real_cargo" ]; then
  grep -E 'cargo test .* -p voice-core( |$)' "$tmp/out" >/dev/null || fail 'strange path lost core'
fi
[ -z "$(find . -name PWNED -print)" ] || fail 'path executed as shell code'
reset
for path in tests/slack_select_test.py scripts/crew_decisions.py; do
  change "$path"
  touched
  for suite in $python_suites; do has "python3 tests/${suite}_test.py"; done
  if [ "$path" = scripts/crew_decisions.py ]; then
    has 'bash tests/crew_install_test.sh'
    if [ -n "$real_cargo" ]; then
      grep -E 'cargo test .* -p voice-core( |$)' "$tmp/out" >/dev/null || fail 'crew parity lost core'
    fi
  else
    no_tests
  fi
  reset
done
printf '\n// uncommitted\n' >> builder/app/build.rs
list --tier touched --base HEAD --list
if [ -n "$real_cargo" ]; then has "${prefix}cargo test --locked -p voice-app"; fi
git restore builder/app/build.rs
printf '// untracked\n' > builder/app/src/new.rs
list --tier touched --base HEAD --list
if [ -n "$real_cargo" ]; then has "${prefix}cargo test --locked -p voice-app"; fi
rm builder/app/src/new.rs
if /bin/bash -c '[ "${BASH_VERSINFO[0]}" = 3 ]'; then
  test_bash=/bin/bash
  panel_test
  unset test_bash
  echo 'ci_local_test: Bash 3.2 passed'
fi
# Scan the caller's sources: the disposable checkout contains inert test stubs.
python3 -I - "$repo" <<'PY' > "$tmp/reads" || fail 'drift guard: unresolved reads (see above)'
import pathlib
import re
import subprocess
import sys
root = pathlib.Path(sys.argv[1]).resolve()
tracked = subprocess.check_output(['git', '-C', str(root), 'ls-files', '-z']).decode().split('\0')
reads = set()
unresolved = set()
ALLOWED_UNRESOLVED = {
    # Scratch folders in core's install tests, not repo files.
    ('rust', 'voice-core', 'crew/ari'),
    ('rust', 'voice-core', 'crew/mo-lee'),
    ('rust', 'voice-core', 'crew/panels'),
    # PROFILE.parent.parent resolves this to the sandbox helper, covered by its own case.
    ('python', 'claude_sandbox_probe_test', 'src/sandbox.rs'),
}

def add(base, target, kind, reader):
    path = (base / target.split('{', 1)[0]).resolve()
    if path == root:
        return
    if root not in path.parents:
        unresolved.add((kind, reader, target))
        return
    if kind == 'rust':
        crate = crate_dirs[reader]
        if path == crate or crate in path.parents:
            return
    if not path.exists():
        unresolved.add((kind, reader, target))
        return
    if path.is_dir():
        prefix = str(path.relative_to(root)) + '/'
        # Python folder reads import or run .py files; without tracked Python, they read data.
        files = [p for p in tracked if p.startswith(prefix) and (root / p).is_file()
                 and not (root / p).is_symlink()]
        if kind == 'python':
            python_files = [p for p in files if p.endswith('.py')]
            if python_files:
                files = python_files
        if not files:
            unresolved.add((kind, reader, target))
        for file in files:
            reads.add((file, kind, reader))
        return
    reads.add((str(path.relative_to(root)), kind, reader))

crate_dirs = {}
for manifest in sorted((root / 'builder').glob('*/Cargo.toml')):
    match = re.search(r'\[package\]\s*name\s*=\s*"([^"]+)"', manifest.read_text())
    if match:
        crate_dirs[match[1]] = manifest.parent
for reader, crate in sorted(crate_dirs.items()):
    sources = list(crate.glob('build.rs'))
    for folder in ('src', 'tests'):
        sources += sorted((crate / folder).rglob('*.rs'))
    for source in sources:
        text = source.read_text()
        targets = [(source.parent, p) for p in re.findall(
            r'include_(?:str|bytes)!\(\s*"(\.\./[^"]+)"', text)]
        for include, module in re.findall(
                r'include!\(\s*"([^"]+)"|#\[path\s*=\s*"([^"]+)"', text):
            targets.append((source.parent, include or module))
        targets += [(crate, p) for p in re.findall(
            r'env!\("CARGO_MANIFEST_DIR"\)[\s\S]{0,100}?\.join\(\s*(?:format!\(\s*)?"(\.\./[^"]+)"', text)]
        targets += [(crate, p) for p in re.findall(
            r'\.join\(\s*format!\(\s*"(\.\./[^"]+)"', text)]
        if 'repo()' in text or re.search(r'\.join\(\s*"../.."\s*\)', text):
            targets += [(root, p) for p in re.findall(
                r'"((?:scripts|bootstrap|crew|docs|tests|builder)/[^"\n]*|\.gitignore)"', text)]
        for base, target in targets:
            add(base, target, 'rust', reader)
for source in sorted((root / 'tests').glob('*.py')):
    reader = source.stem if source.name.endswith('_test.py') else '*'
    for target in re.findall(
            r'''(?:\bREPO\s*/|\bROOT\s*/|parents\[1\]\s*/|parent\.parent\s*/)\s*['"]([^'"]+)['"]''',
            source.read_text()):
        add(root, target, 'python', reader)
for source in sorted((root / 'tests').glob('*_test.sh')):
    for target in re.findall(r'"\$(?:here|repo)/([^"$]+)', source.read_text()):
        add(root, target, 'shell', source.stem)
for path, kind, reader in sorted(reads):
    print(path + '\t' + kind + '\t' + reader)
for kind, reader, target in sorted(unresolved):
    print(f'ci_local_test: unresolved {kind} {reader} {target}', file=sys.stderr)
stale = ALLOWED_UNRESOLVED - unresolved
for kind, reader, target in sorted(stale):
    print(f'ci_local_test: stale allowed unresolved {kind} {reader} {target}', file=sys.stderr)
if unresolved - ALLOWED_UNRESOLVED or stale:
    sys.exit(1)
print(f'ci_local_test: unresolved reads: {len(unresolved)}, all allowed', file=sys.stderr)
PY
rust_count=0 python_count=0 shell_count=0 full_count=0
last_path=
while IFS=$'\t' read -r path kind reader; do
  # One listing per target, even when several readers consume it.
  if [ "$path" != "$last_path" ]; then
    change "$path"
    touched
    reset
    last_path=$path
  fi
  result=picked
  if cmp -s "$tmp/full" "$tmp/out"; then
    result=full
    full_count=$((full_count + 1))
  else
    case "$kind" in
      rust)
        if [ -n "$real_cargo" ]; then
          grep -E "cargo test .* -p $reader( |$)" "$tmp/out" >/dev/null || fail "cross-crate read lost $reader: $path"
        else
          result=cargo-missing
        fi ;;
      python)
        if [ "$reader" = '*' ]; then
          grep -E '^python3 tests/.*_test.py$' "$tmp/out" >/dev/null || fail "Python helper lost all suites: $path"
        else
          has "python3 tests/$reader.py"
        fi ;;
      shell) has "bash tests/$reader.sh" ;;
    esac
  fi
  case "$kind" in
    rust) rust_count=$((rust_count + 1)) ;;
    python) python_count=$((python_count + 1)) ;;
    shell) shell_count=$((shell_count + 1)) ;;
  esac
  echo "ci_local_test: read $kind $reader <- $path: $result"
done < "$tmp/reads"
echo "ci_local_test: cross-crate reads checked: rust=$rust_count python=$python_count shell=$shell_count full=$full_count"
echo 'ci_local_test: all passed'
