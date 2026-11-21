#!/usr/bin/env bash
# bash scripts/ci-local.sh                       # full gate (the default)
# bash scripts/ci-local.sh --tier full           # the same
# bash scripts/ci-local.sh --tier touched        # changed crates and their dependents
# bash scripts/ci-local.sh [--tier full|touched] --list  # list steps, run none
# bash scripts/ci-local.sh --tier touched --base <rev>  # compare with <rev>
set -euo pipefail

cd "$(dirname "$0")/.."

usage() {
  echo 'Usage: scripts/ci-local.sh [--tier full|touched] [--base <rev>] [--list]' >&2
  exit 2
}
tier=full
base=
base_given=false
list=false
while [ "$#" -gt 0 ]; do
  case "$1" in
    --tier)
      [ "$#" -ge 2 ] || usage
      case "$2" in full|touched) tier=$2 ;; *) usage ;; esac
      shift 2 ;;
    --base)
      [ "$#" -ge 2 ] && [ -n "$2" ] && [[ "$2" != --* ]] || usage
      base=$2; base_given=true; shift 2 ;;
    --list) list=true; shift ;;
    *) usage ;;
  esac
done
if $base_given && [ "$tier" != touched ]; then usage; fi

labels=()
commands=()
add() {
  labels[${#labels[@]}]=$1
  commands[${#commands[@]}]=${2:-$1}
}
note() { echo "$*" >&2; }
python_suites='crew_decisions crew_sources crew_extra_sources crew_labels crew_hardening crew_documents crew_rounds slack_mine slack_select crew_personas growth_loop_spec claude_sandbox_probe read_fence_probe'
shell_suites='crew_install mac_install fetch_radio build_mac_app ci_local verify_main'
shellcheck_line='shellcheck scripts/ci-local.sh bootstrap/install.sh tests/crew_install_test.sh scripts/build-mac-app.sh builder/install/install.sh tests/mac_install_test.sh scripts/fetch-radio.sh tests/fetch_radio_test.sh tests/build_mac_app_test.sh tests/ci_local_test.sh scripts/verify-main.sh scripts/release-check.sh tests/verify_main_test.sh'
cargo_present=false
if command -v cargo >/dev/null 2>&1; then cargo_present=true; fi
host_os=$(uname -s)
test_prefix='VOICE_REQUIRE_PARITY=1 '
if [ "$host_os" = Darwin ]; then test_prefix+='VOICE_REQUIRE_SANDBOX=1 '; fi

# Each feature assertion remains a separate cargo tree step, including equal labels.
feature_check() {
  local package=$1 edges=$2 feature=$3 expected=$4 subscription=${5:-} tree
  if [ -n "$subscription" ]; then
    tree=$(cargo tree --locked --target all -p "$package" --features claude-subscription -e "$edges" -i voice-agent)
  else
    tree=$(cargo tree --locked --target all -p "$package" -e "$edges" -i voice-agent)
  fi
  if [ "$expected" = present ]; then
    if ! grep -Fq "voice-agent feature \"$feature\"" <<< "$tree"; then
      if [ "$feature" = test-stubs ]; then
        echo 'ci-local: voice-cli test feature check did not find test-stubs'
      else
        echo "ci-local: voice-app's claude-subscription feature does not reach voice-agent"
      fi
      return 1
    fi
  elif grep -Fq "voice-agent feature \"$feature\"" <<< "$tree"; then
    echo "ci-local: $package builds voice-agent with $feature"
    return 1
  fi
}
feature_steps() {
  add 'cargo tree --locked --target all -p voice-cli -e features,normal,build,dev -i voice-agent' 'feature_check voice-cli features,normal,build,dev test-stubs present'
  local package feature
  for feature in test-stubs claude-subscription; do
    for package in voice-cli voice-app; do
      add "cargo tree --locked --target all -p $package -e features,normal,build -i voice-agent" "feature_check $package features,normal,build $feature absent"
    done
  done
  add 'cargo tree --locked --target all -p voice-app --features claude-subscription -e features,normal,build -i voice-agent' 'feature_check voice-app features,normal,build claude-subscription present subscription'
}
generated_types() {
  git diff --exit-code -- builder/ui/src/types/generated
  if [ -n "$(git status --porcelain -- builder/ui/src/types/generated)" ]; then
    echo 'ci-local: builder/ui/src/types/generated has files cargo test wrote that are not committed'
    return 1
  fi
}
keystore_steps() {
  local target
  for target in x86_64-pc-windows-msvc x86_64-unknown-linux-gnu; do
    if rustup target list --installed 2>/dev/null | grep -qx "$target"; then
      add "cargo check --locked -p voice-keystore --all-targets --target $target"
    else
      note "ci-local: the $target target is missing, skipping its check (rustup target add $target)"
    fi
  done
}
screens_steps() {
  if command -v pnpm >/dev/null 2>&1; then
    # --list must not execute pnpm, even for --version. Apply its gate only on runs.
    if ! $list; then
      local pnpm_version pnpm_major
      pnpm_version=$(pnpm --version)
      pnpm_major=${pnpm_version%%.*}
      if [[ "$pnpm_major" == *[!0-9]* || -z "$pnpm_major" ]]; then
        note "ci-local: couldn't read pnpm's version, skipping the app screens (update pnpm: https://pnpm.io/installation)"
        return
      elif [ "$pnpm_major" -lt 10 ]; then
        note "ci-local: pnpm $pnpm_version is older than 10, skipping the app screens (update pnpm: https://pnpm.io/installation)"
        return
      fi
    fi
    local step
    for step in 'install --frozen-lockfile' typecheck test build; do
      add "pnpm -C builder/ui $step"
    done
  else
    note 'ci-local: pnpm not found, skipping the app screens (install Node and pnpm: https://pnpm.io/installation)'
  fi
}
cargo_missing() {
  note 'ci-local: cargo not found, skipping the Rust checks (install Rust from https://rustup.rs)'
}

picked=()
crates=()
python_picked=' '
shell_picked=' '
screens=false
pick() { picked[${#picked[@]}]=$1; }
fallback() {
  note "ci-local: can't tell what changed ($1), running the full gate"
  tier=full
}
path_fallback() {
  note "ci-local: tier touched falls back to the full gate: $path $1"
  tier=full
}
map_path() {
  local path=$1 dir name
  case "$path" in
    # Records and prose are read by no tests.
    .ai/*|plans/*|docs/spikes/*|README.md|CONTRIBUTING.md|AGENTS.md|SECURITY.md) ;;
    # The gate and its own test must exercise the whole gate.
    scripts/ci-local.sh|tests/ci_local_test.sh) path_fallback 'is the gate itself' ;;
    # ts-rs in voice-api writes these exports, consumed by the screens.
    builder/ui/src/types/generated/*) pick voice-api; screens=true ;;
    # voice-api's building test reads the screen's keep-going helper.
    builder/ui/src/screens/build/keepGoing.ts) pick voice-api; screens=true ;;
    # The remaining UI files are consumed by the screens' checks.
    builder/ui/*) screens=true ;;
    # voice-ingest includes voice-cli's anonymizer tests.
    builder/cli/tests/anonymize.rs) pick voice-cli; pick voice-ingest ;;
    # The Python read-fence probe checks parity with core's deny list.
    builder/core/src/deny.rs) pick voice-core; python_picked+='read_fence_probe ' ;;
    # Both voice-grow and the Python spec suite read the golden files.
    builder/grow/tests/golden/*) pick voice-grow; python_picked+='growth_loop_spec ' ;;
    # The Mac installer, Radio fetch and build tests read the shipped installer files.
    builder/install/*) shell_picked+='mac_install fetch_radio build_mac_app ' ;;
    # The sandbox probes read the Rust helper and its Seatbelt profile.
    builder/agent/src/sandbox.rs|builder/agent/sandbox/*)
      pick voice-agent; python_picked+='claude_sandbox_probe read_fence_probe ' ;;
    # Other builder files belong to the nearest package at builder/<dir> or below.
    builder/*)
      dir=${path%/*}
      name=
      while [[ "$dir" == builder/* ]]; do
        if [ -f "$dir/Cargo.toml" ]; then
          name=$(awk '
            /^\[package\]/{package=1; next}
            /^\[/{package=0}
            package && /^[[:space:]]*name[[:space:]]*=/ {
              sub(/^[^"]*"/, ""); sub(/".*/, ""); print; exit
            }' "$dir/Cargo.toml")
          if [ -n "$name" ]; then break; fi
        fi
        dir=${dir%/*}
      done
      if [ -n "$name" ]; then pick "$name"; else path_fallback 'maps to no part'; fi ;;
    # voice-grow and the Python growth-loop suite consume the spec examples.
    docs/spec/*) pick voice-grow; python_picked+='growth_loop_spec ' ;;
    # Rust parity tests and the installer read the shared crew Python tools/fixtures.
    scripts/crew_*.py|tests/crew_*.py)
      pick voice-core; python_picked=" $python_suites "; shell_picked+='crew_install ' ;;
    # Python suites read the scripts; the crew installer test copies scripts/.
    scripts/*.py) python_picked=" $python_suites "; shell_picked+='crew_install ' ;;
    # Python suites import one another; the crew installer test copies scripts/, not tests/.
    tests/*.py) python_picked=" $python_suites " ;;
    # Rust install parity and the crew installer test consume the guided bootstrap.
    bootstrap/*) pick voice-core; shell_picked+='crew_install ' ;;
    # The Radio fetch and build tests read the downloader; the crew installer test copies scripts/.
    scripts/fetch-radio.sh) shell_picked+='fetch_radio crew_install build_mac_app ' ;;
    # The build test reads the build script; the crew installer test copies scripts/.
    scripts/build-mac-app.sh) shell_picked+='crew_install build_mac_app ' ;;
    # The main check and release guard test reads them; the crew installer test copies scripts/.
    scripts/verify-main.sh|scripts/release-check.sh) shell_picked+='verify_main crew_install ' ;;
    # Each shell test exercises itself.
    tests/crew_install_test.sh) shell_picked+='crew_install ' ;;
    tests/mac_install_test.sh) shell_picked+='mac_install ' ;;
    tests/fetch_radio_test.sh) shell_picked+='fetch_radio ' ;;
    tests/build_mac_app_test.sh) shell_picked+='build_mac_app ' ;;
    tests/verify_main_test.sh) shell_picked+='verify_main ' ;;
    # Unmapped inputs can affect any test, so fail safe to the full gate.
    *) path_fallback 'maps to no part' ;;
  esac
}
select_touched() {
  local path changed metadata closure name
  if $base_given; then
    if ! base=$(git rev-parse --verify --end-of-options "$base^{commit}" 2>/dev/null); then
      fallback 'invalid --base'; return
    fi
  elif ! base=$(git merge-base origin/main HEAD 2>/dev/null); then
    fallback 'no merge base with origin/main'; return
  fi
  changed=$(mktemp)
  if ! git diff --name-only -z --no-renames "$base" HEAD -- > "$changed" ||
     ! git diff --name-only -z --no-renames HEAD -- >> "$changed" ||
     ! git ls-files -z --others --exclude-standard >> "$changed"; then
    rm -f "$changed"
    fallback 'git could not list paths'; return
  fi
  if [ ! -s "$changed" ]; then
    note "ci-local: no changed paths since $(git rev-parse --short "$base"), running only the repo-wide scans"
  fi
  while IFS= read -r -d '' path; do
    map_path "$path"
    if [ "$tier" = full ]; then break; fi
  done < "$changed"
  rm -f "$changed"
  [ "$tier" = touched ] || return 0
  if $cargo_present && [ "${#picked[@]}" -gt 0 ]; then
    if ! metadata=$(cargo metadata --locked --offline --no-deps --format-version 1); then
      fallback 'cargo metadata failed'; return
    fi
    # Metadata is data: validate all workspace names before any reach command strings.
    if closure=$(python3 -I -c '
import json, re, sys
packages = json.load(sys.stdin)
members = set(packages["workspace_members"])
packages = [p for p in packages["packages"] if p["id"] in members]
names = {p["name"] for p in packages}
if any(re.fullmatch(r"[a-z0-9][a-z0-9_-]*", n) is None for n in names):
    sys.exit(3)
picked = set(sys.argv[1:])
if not picked <= names:
    sys.exit(1)
while True:
    dependents = {p["name"] for p in packages if any(
        d["name"] != p["name"] and d["name"] in picked for d in p["dependencies"])}
    expanded = picked | dependents
    if expanded == picked:
        break
    picked = expanded
print("\n".join(sorted(picked)))
' "${picked[@]+"${picked[@]}"}" <<< "$metadata"); then
      while IFS= read -r name; do crates[${#crates[@]}]=$name; done <<< "$closure"
    else
      if [ "$?" = 3 ]; then
        note 'ci-local: unexpected package name from cargo metadata'
        exit 1
      fi
      fallback 'cargo metadata graph could not be read'
    fi
  fi
}

if [ "$tier" = touched ]; then select_touched; fi
if [ "$tier" = full ]; then
  for t in $python_suites; do add "python3 tests/${t}_test.py"; done
  for t in $shell_suites; do add "bash tests/${t}_test.sh"; done
  add "$shellcheck_line"
  if $cargo_present; then
    add 'cargo fmt --check'
    add 'cargo clippy --all-targets --locked -- -D warnings'
    add "${test_prefix}cargo test --locked -p voice-agent -p voice-api -p voice-cli --features claude-subscription"
    add "${test_prefix}cargo test --locked"
    feature_steps
    add 'cargo clippy --locked -p voice-agent -p voice-api -p voice-cli -p voice-app --all-targets --features claude-subscription -- -D warnings'
    add 'cargo clippy --locked -p voice-cli -p voice-app -- -D warnings'
    add 'git diff --exit-code -- builder/ui/src/types/generated' generated_types
    keystore_steps
  else
    cargo_missing
  fi
  screens_steps
else
  # Repo-wide scans always run: shellcheck, the drift guard, fmt, feature trees, generated types.
  add "$shellcheck_line"
  add 'bash tests/ci_local_test.sh'
  for t in $python_suites; do
    if [[ "$python_picked" == *" $t "* ]]; then add "python3 tests/${t}_test.py"; fi
  done
  for t in $shell_suites; do
    if [[ "$shell_picked" == *" $t "* ]]; then add "bash tests/${t}_test.sh"; fi
  done
  if $cargo_present; then
    add 'cargo fmt --check'
    feature_steps
    all='' feature='' subscription='' public=''
    for name in "${crates[@]+"${crates[@]}"}"; do
      all+=" -p $name"
      case "$name" in voice-agent|voice-api|voice-cli) feature+=" -p $name" ;; esac
      case "$name" in voice-agent|voice-api|voice-cli|voice-app) subscription+=" -p $name" ;; esac
      case "$name" in voice-cli|voice-app) public+=" -p $name" ;; esac
    done
    if [ -n "$all" ]; then
      if [ -n "$feature" ]; then add "${test_prefix}cargo test --locked$feature --features claude-subscription"; fi
      add "${test_prefix}cargo test --locked$all"
      add "cargo clippy --locked$all --all-targets -- -D warnings"
      if [ -n "$subscription" ]; then add "cargo clippy --locked$subscription --all-targets --features claude-subscription -- -D warnings"; fi
      if [ -n "$public" ]; then add "cargo clippy --locked$public -- -D warnings"; fi
      if [[ " $all " == *' -p voice-keystore '* ]]; then keystore_steps; fi
    fi
    add 'git diff --exit-code -- builder/ui/src/types/generated' generated_types
  else
    cargo_missing
  fi
  if $screens; then screens_steps; fi
fi

if $list; then
  printf '%s\n' "${labels[@]}"
  exit 0
fi
# The agent tests need the Mac's sandbox. Off a Mac, refuse before any step if cargo is
# missing or the gate would run cargo test. Exit 75 means "this host can't run the gate",
# so a caller can move the gate to a Mac.
if [ "$host_os" != Darwin ]; then
  if ! $cargo_present; then
    note "ci-local: cargo is missing and this host is $host_os, so the Rust checks can't run here and a Mac must run the gate; exiting 75 (this host can't run the gate) before any step"
    exit 75
  fi
  for label in "${labels[@]+"${labels[@]}"}"; do
    if [[ "$label" == *'cargo test '* ]]; then
      note "ci-local: cargo test needs a Mac (the agent tests run Claude Code in the Mac's own sandbox) and this host is $host_os; exiting 75 (this host can't run the gate) before any step"
      exit 75
    fi
  done
fi
if [ "$tier" = full ]; then
  echo "ci-local: tier full, ${#labels[@]} steps"
else
  crate_note='no crates'
  if [ "${#crates[@]}" -gt 0 ]; then crate_note="crates: ${crates[*]}"; fi
  echo "ci-local: tier touched, ${#labels[@]} steps ($crate_note)"
fi
for ((i=0; i<${#labels[@]}; i++)); do
  echo "${labels[$i]}"
  # Only script literals and validated metadata package names enter these commands.
  eval "${commands[$i]}"
done
echo 'ci-local: all passed'
