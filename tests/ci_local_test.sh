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
commit() { git -c user.name=t -c user.email=t@example.invalid commit --allow-empty -qm "$1"; }
git add scripts/ci-local.sh
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
shellcheck_line='shellcheck scripts/ci-local.sh bootstrap/install.sh tests/crew_install_test.sh scripts/build-mac-app.sh builder/install/install.sh tests/mac_install_test.sh scripts/fetch-radio.sh tests/fetch_radio_test.sh tests/ci_local_test.sh'
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
  echo 'bash tests/ci_local_test.sh'
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
cmp "$tmp/full" "$tmp/out" || fail 'explicit full differs'
if [ "$(uname -s)" = Darwin ] && [ -n "$real_cargo" ]; then
  diff -u "$tmp/golden" "$tmp/out" || fail 'full gate order changed'
else
  head -n 18 "$tmp/out" > "$tmp/actual-prefix"
  cmp "$tmp/prefix" "$tmp/actual-prefix" || fail 'full gate prefix changed'
fi
echo 'ci_local_test: full golden list passed'

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
# Resolve literal cross-crate reads independently of the gate's path table.
python3 -I - <<'PY' > "$tmp/reads"
import pathlib
import re
root = pathlib.Path.cwd()
skipped = 0
reads = set()
for crate in sorted((root / 'builder').iterdir()):
    manifest = crate / 'Cargo.toml'
    if not manifest.is_file():
        continue
    match = re.search(r'\[package\]\s*name\s*=\s*"([^"]+)"', manifest.read_text())
    if not match:
        continue
    for folder in ('src', 'tests'):
        for source in sorted((crate / folder).rglob('*.rs')):
            text = source.read_text()
            targets = [(source.parent, p) for p in re.findall(r'include_(?:str|bytes)!\(\s*"(\.\./[^"]+)"', text)]
            targets += [(crate, p) for p in re.findall(r'env!\("CARGO_MANIFEST_DIR"\)[^\n]*?\.join\("(\.\./[^"]+)"\)', text)]
            for base, target in targets:
                path = (base / target.split('{', 1)[0]).resolve()
                if crate == path or crate in path.parents:
                    continue
                if not path.exists() or (root != path and root not in path.parents):
                    skipped += 1
                    continue
                if path.is_dir():
                    path /= 'ci-local-drift-probe.txt'
                reads.add((str(path.relative_to(root)), match[1]))
for path, name in sorted(reads):
    print(path + '\t' + name)
print('ci_local_test: cross-crate reads skipped (missing): ' + str(skipped), file=__import__('sys').stderr)
PY
count=0
while IFS=$'\t' read -r path crate; do
  change "$path"
  touched
  if ! cmp -s "$tmp/full" "$tmp/out" && [ -n "$real_cargo" ]; then
    grep -E "cargo test .* -p $crate( |$)" "$tmp/out" >/dev/null || fail "cross-crate read lost $crate: $path"
  fi
  reset
  count=$((count + 1))
done < "$tmp/reads"
echo "ci_local_test: cross-crate reads checked: $count"
echo 'ci_local_test: all passed'
