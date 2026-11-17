#!/usr/bin/env bash
set -euo pipefail

if [ "$(uname -s)" != Darwin ] || ! command -v cc >/dev/null 2>&1 || ! command -v lipo >/dev/null 2>&1 \
  || ! command -v codesign >/dev/null 2>&1; then
  echo 'build_mac_app_test: skipped (needs macOS with cc, lipo and codesign)'
  exit 0
fi

here="$(cd "$(dirname "$0")/.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }
check() { grep -Fq -- "$2" "$1" 2>/dev/null || { cat "$1" >&2 2>/dev/null || true; fail "$3"; }; }

# A copy of the files the build reads, as its own git checkout, so the build runs end to end with
# stand-ins for cargo, pnpm, rustup and curl and leaves the real checkout alone.
work="$scratch/repo"
mkdir -p "$work/scripts" "$work/builder/install" "$work/builder/app" "$scratch/bin" "$scratch/releases"
cp "$here/scripts/build-mac-app.sh" "$here/scripts/fetch-radio.sh" "$work/scripts/"
cp "$here/builder/install/install.sh" "$here/builder/install/How to install.txt" "$work/builder/install/"
printf '{"version": "9.9.9"}\n' > "$work/builder/app/tauri.conf.json"

tag=v9.8.7
for arch in arm64 x86_64; do
  folder="$scratch/make/radio-$tag-macos-$arch"
  mkdir -p "$folder"
  printf 'int main(void){return 0;}\n' | cc -arch "$arch" -x c - -o "$folder/radio"
  codesign --force --sign - "$folder/radio" 2>/dev/null
  printf 'MIT License\n\nCopyright (c) Radio test\n' > "$folder/LICENSE"
  tar -czf "$scratch/releases/radio-$tag-macos-$arch.tar.gz" -C "$scratch/make" "radio-$tag-macos-$arch"
done
{
  echo '# The Radio release Voice ships.'
  echo "tag $tag"
  for arch in arm64 x86_64; do
    echo "$arch $(shasum -a 256 "$scratch/releases/radio-$tag-macos-$arch.tar.gz" | cut -d ' ' -f 1)"
  done
} > "$work/builder/install/radio-release.txt"
(
  cd "$work"
  git init -q
  git add -A
  git -c user.name=test -c user.email=test@example.invalid commit -q -m 'build test copy'
)

# Each stand-in makes a real Mach-O for the architectures it is asked for, so lipo, codesign and
# PlistBuddy in the build check real files.
printf 'int main(void){return 0;}\n' > "$scratch/main.c"
cat > "$scratch/bin/make-binary" <<'MAKE'
#!/usr/bin/env bash
set -euo pipefail
out="$1"
shift
mkdir -p "$(dirname "$out")"
args=()
for arch in "$@"; do args+=(-arch "$arch"); done
cc "${args[@]}" "$FAKE_SOURCE" -o "$out"
MAKE
cat > "$scratch/bin/cargo" <<'CARGO'
#!/usr/bin/env bash
set -euo pipefail
if [ "$1" = metadata ]; then
  printf '{"target_directory": "%s"}\n' "$FAKE_TARGET"
  exit 0
fi
target=''
while [ "$#" -gt 0 ]; do
  if [ "$1" = --target ]; then target="$2"; shift; fi
  shift
done
case "$target" in
  '') make-binary "$FAKE_TARGET/release/voice" "$(uname -m)" ;;
  aarch64-apple-darwin) make-binary "$FAKE_TARGET/$target/release/voice" arm64 ;;
  x86_64-apple-darwin) make-binary "$FAKE_TARGET/$target/release/voice" x86_64 ;;
  *) exit 3 ;;
esac
CARGO
cat > "$scratch/bin/pnpm" <<'PNPM'
#!/usr/bin/env bash
set -euo pipefail
[ "$1" != --version ] || { echo 10.0.0; exit 0; }
[ "$3" = tauri ] || exit 0
case " $* " in
  *' --target universal-apple-darwin '*) bundle="$FAKE_TARGET/universal-apple-darwin/release/bundle/macos"; archs=(arm64 x86_64) ;;
  *) bundle="$FAKE_TARGET/release/bundle/macos"; archs=("$(uname -m)") ;;
esac
app="$bundle/Voice.app"
make-binary "$app/Contents/MacOS/voice-app" "${archs[@]}"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>voice-app</string>
<key>CFBundleIdentifier</key><string>invalid.example.voice-test</string>
</dict></plist>
PLIST
codesign --force --sign - "$app" 2>/dev/null
PNPM
cat > "$scratch/bin/rustup" <<'RUSTUP'
#!/usr/bin/env bash
printf 'aarch64-apple-darwin\nx86_64-apple-darwin\n'
RUSTUP
cat > "$scratch/bin/curl" <<'CURL'
#!/usr/bin/env bash
set -euo pipefail
out=''
while [ "$#" -gt 1 ]; do
  if [ "$1" = -o ]; then out="$2"; shift; fi
  shift
done
prefix="https://github.com/awna-ai/radio/releases/download/$FAKE_TAG/"
[ "${1#"$prefix"}" != "$1" ] || exit 22
cp "$FAKE_RELEASES/${1#"$prefix"}" "$out"
CURL
chmod 755 "$scratch/bin/"*

build() {  # label, then the build's arguments
  local label="$1"
  shift
  status=0
  # Only the stand-ins and the system's own tools, so no real cargo, pnpm or curl can run.
  PATH="$scratch/bin:/usr/bin:/bin:/usr/sbin:/sbin" FAKE_SOURCE="$scratch/main.c" FAKE_TARGET="$scratch/target-$label" FAKE_TAG="$tag" \
    FAKE_RELEASES="$scratch/releases" bash "$work/scripts/build-mac-app.sh" "$@" \
    > "$scratch/stdout" 2> "$scratch/err" || status=$?
  [ "$status" = 0 ] || { cat "$scratch/err" >&2; fail "the $label build failed"; }
}

shipped() {  # zip, then the architectures each command in Voice.app must hold
  local zip="$1" unpacked="$scratch/unpacked" command archs arch
  shift
  [ -f "$zip" ] || fail "no package at $zip"
  rm -rf "$unpacked"
  mkdir "$unpacked"
  ditto -x -k "$zip" "$unpacked"
  [ -x "$unpacked/Voice/install.sh" ] || fail "$zip has no install.sh"
  for command in voice-app voice radio; do
    archs=" $(lipo -archs "$unpacked/Voice/Voice.app/Contents/MacOS/$command") "
    for arch in "$@"; do
      [[ "$archs" == *" $arch "* ]] || fail "$command in $zip holds$archs, not $arch"
    done
  done
  check "$unpacked/Voice/Voice.app/Contents/Resources/radio/LICENSE" 'MIT License' "$zip has no Radio licence"
}

build single
shipped "$work/dist/voice-9.9.9-mac-$(uname -m).zip" "$(uname -m)"
echo 'build_mac_app_test: one architecture passed'

# The universal build joins two voice commands; on a stock Mac's case-insensitive disk the joined
# file must not share a name with the Voice package folder beside it.
build universal --universal
shipped "$work/dist/voice-9.9.9-mac-universal.zip" arm64 x86_64
echo 'build_mac_app_test: universal passed'

[ -z "$(cd "$work" && git status --porcelain --untracked-files=no)" ] || fail 'the build changed tracked files'
echo 'build_mac_app_test: all passed'
