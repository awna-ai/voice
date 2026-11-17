#!/usr/bin/env bash
set -euo pipefail

universal=false
testers=false
for argument in "$@"; do
  case "$argument" in
    --universal) universal=true ;;
    --testers) testers=true ;;
    *) echo 'Usage: bash scripts/build-mac-app.sh [--universal] [--testers]' >&2; exit 2 ;;
  esac
done

cd "$(dirname "$0")/.."
repo="$PWD"
fail() { echo "build-mac-app: $*" >&2; exit 1; }
[ "$(uname -s)" = Darwin ] || fail 'macOS only'
command -v cargo >/dev/null 2>&1 || fail 'cargo not found (install Rust from https://rustup.rs)'
command -v pnpm >/dev/null 2>&1 || fail 'pnpm not found (install Node and pnpm: https://pnpm.io/installation)'
command -v python3 >/dev/null 2>&1 || fail 'python3 not found (install Python from https://www.python.org/downloads/)'
pnpm_version=$(pnpm --version)
pnpm_major=${pnpm_version%%.*}
if [[ "$pnpm_major" == *[!0-9]* || -z "$pnpm_major" ]]; then
  fail "couldn't read pnpm's version (update pnpm: https://pnpm.io/installation)"
elif [ "$pnpm_major" -lt 10 ]; then
  fail "pnpm $pnpm_version is older than 10 (update pnpm: https://pnpm.io/installation)"
fi

build_args=(build --bundles app)
cli_args=(build --release --locked -p voice-cli)
if "$testers"; then
  build_args+=(--features claude-subscription)
  cli_args+=(--features claude-subscription)
fi
bundle_dir=release/bundle/macos
if "$universal"; then
  label=universal
  targets=$(rustup target list --installed 2>/dev/null || true)
  if ! grep -qx aarch64-apple-darwin <<< "$targets" || ! grep -qx x86_64-apple-darwin <<< "$targets"; then
    fail '--universal needs both Mac targets: rustup target add aarch64-apple-darwin x86_64-apple-darwin'
  fi
  build_args+=(--target universal-apple-darwin)
  bundle_dir="universal-apple-darwin/release/bundle/macos"
else
  label=$(uname -m)
fi

version=$(python3 -I -c 'import json; print(json.load(open("builder/app/tauri.conf.json"))["version"])')
if ! source_status=$(git status --porcelain -- builder crew scripts Cargo.toml Cargo.lock); then
  fail 'build from a git checkout of the repo, so uncommitted changes can be checked'
fi
if [ -n "$source_status" ]; then
  fail 'commit or set aside your changes under builder/, crew/, scripts/ and the Cargo files first'
fi
name="voice-$version-mac-$label"
if "$testers"; then
  name+=-testers
fi
mkdir -p "$repo/dist"
staging="$repo/dist/$name"
trap 'rm -rf "$staging"' EXIT
rm -rf "$staging"
mkdir "$staging"
# Radio first, so a bad pin or no network stops the build before the long compile.
if "$universal"; then
  radio_archs=(arm64 x86_64)
else
  radio_archs=("$label")
fi
bash scripts/fetch-radio.sh builder/install/radio-release.txt "$staging/radio" "${radio_archs[@]}"
pnpm -C builder/ui install --frozen-lockfile
pnpm -C builder/ui tauri "${build_args[@]}"
target_dir=$(CARGO_NET_OFFLINE=true cargo metadata --format-version 1 --no-deps | python3 -I -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
app="$target_dir/$bundle_dir/Voice.app"
[ -d "$app" ] || fail "app not found: $app"
codesign --verify --deep --strict "$app" >/dev/null 2>&1 || fail "signature check failed: $app"

package="$staging/Voice"
mkdir -p "$package"

if "$universal"; then
  cli_binaries=()
  for target in aarch64-apple-darwin x86_64-apple-darwin; do
    CARGO_NET_OFFLINE=true cargo "${cli_args[@]}" --target "$target"
    binary="$target_dir/$target/release/voice"
    [ -f "$binary" ] || fail 'the voice command was not built'
    cli_binaries+=("$binary")
  done
  # Its own folder: on a case-insensitive disk "$staging/voice" is the "$staging/Voice" package.
  mkdir "$staging/cli"
  staging_cli="$staging/cli/voice"
  lipo -create -output "$staging_cli" "${cli_binaries[@]}"
else
  CARGO_NET_OFFLINE=true cargo "${cli_args[@]}"
  staging_cli="$target_dir/release/voice"
fi
[ -f "$staging_cli" ] || fail 'the voice command was not built'

ditto "$app" "$package/Voice.app"
staged_app="$package/Voice.app"
staged_cli="$staged_app/Contents/MacOS/voice"
cp "$staging_cli" "$staged_cli"
chmod 755 "$staged_cli"
codesign --force --sign - "$staged_cli"
staged_radio="$staged_app/Contents/MacOS/radio"
cp "$staging/radio/radio" "$staged_radio"
chmod 755 "$staged_radio"
# Radio's release is already signed (ad hoc, identifier ai.awna.radio); shipping it unchanged keeps
# its bytes matching the pin.
mkdir -p "$staged_app/Contents/Resources/radio"
cp "$staging/radio/LICENSE" "$staged_app/Contents/Resources/radio/LICENSE"
codesign --force --sign - --preserve-metadata=identifier,entitlements,requirements,flags,runtime "$staged_app"
codesign --verify --deep --strict "$staged_app" >/dev/null 2>&1 || fail "signature check failed: $staged_app"

executable=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$staged_app/Contents/Info.plist")
architectures=$(lipo -archs "$staged_app/Contents/MacOS/$executable")
read -r -a arch_list <<< "$architectures"
covers_app() {  # binary, what it is: it must run on every Mac the app runs on
  local binary_architectures arch binary_arch compatible
  local -a binary_arch_list
  binary_architectures=$(lipo -archs "$1") || fail "$2 was built for other Macs than the app"
  read -r -a binary_arch_list <<< "$binary_architectures"
  for arch in "${arch_list[@]}"; do
    compatible=false
    for binary_arch in "${binary_arch_list[@]}"; do
      if [ "$arch" = "$binary_arch" ]; then
        compatible=true
      fi
    done
    "$compatible" || fail "$2 was built for other Macs than the app"
  done
}
covers_app "$staged_cli" 'the voice command'
covers_app "$staged_radio" 'Radio'

cp builder/install/install.sh "$package/install.sh"
chmod 755 "$package/install.sh"
cp 'builder/install/How to install.txt' "$package/How to install.txt"
zip="$repo/dist/$name.zip"
rm -f "$zip"
ditto -c -k --norsrc --noextattr --keepParent "$package" "$zip"
cd "$repo/dist"
shasum -a 256 "$name.zip" > "$name.zip.sha256"
echo "$zip"
cat "$name.zip.sha256"
