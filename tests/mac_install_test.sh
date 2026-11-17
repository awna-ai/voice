#!/usr/bin/env bash
set -euo pipefail

if [ "$(uname -s)" != Darwin ] || ! command -v cc >/dev/null 2>&1 || ! command -v codesign >/dev/null 2>&1; then
  echo 'mac_install_test: skipped (needs macOS with cc and codesign)'
  exit 0
fi

here="$(cd "$(dirname "$0")/.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }
check() { grep -Fq -- "$2" "$1" || { cat "$1" >&2; fail "$3"; }; }

if [ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" = 1 ]; then
  arch=arm64
  other_arch=x86_64
else
  arch=x86_64
  other_arch=arm64
fi

fake_app() {  # path architecture [radio's architecture]
  mkdir -p "$1/Contents/MacOS"
  cat > "$1/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>voice-app</string>
<key>CFBundleIdentifier</key><string>dev.voice.test</string>
<key>CFBundlePackageType</key><string>APPL</string>
</dict></plist>
PLIST
  printf 'int main(void){return 0;}\n' | cc -arch "$2" -x c - -o "$1/Contents/MacOS/voice-app"
  # The stand-in voice records each run's arguments in $HOME/voice-ran and exits with $FAKE_VOICE_EXIT.
  cc -arch "$2" -x c - -o "$1/Contents/MacOS/voice" <<'C'
#include <stdio.h>
#include <stdlib.h>
int main(int argc, char **argv) {
  char path[4096];
  const char *home = getenv("HOME"), *exit_code = getenv("FAKE_VOICE_EXIT");
  int code = exit_code ? atoi(exit_code) : 0;
  snprintf(path, sizeof path, "%s/voice-ran", home ? home : "/nonexistent");
  FILE *ran = fopen(path, "a");
  if (ran) {
    for (int i = 1; i < argc; i++) fprintf(ran, "%s\n", argv[i]);
    fputs("--\n", ran);
    fclose(ran);
  }
  if (code) fputs("voice: the stand-in Radio failed\n", stderr); else puts("the stand-in Radio is installed");
  return code;
}
C
  printf 'int main(void){return 0;}\n' | cc -arch "${3:-$2}" -x c - -o "$1/Contents/MacOS/radio"
  codesign -s - --force "$1/Contents/MacOS/voice" >/dev/null 2>&1
  codesign -s - --force "$1/Contents/MacOS/radio" >/dev/null 2>&1
  codesign -s - --force --deep "$1" >/dev/null 2>&1
}

kit() {  # case architecture (empty for no app)
  kit_dir="$scratch/kit folder/$1"
  home="$scratch/home/$1"
  installed="$home/Applications/Voice.app"
  mkdir -p "$kit_dir" "$home"
  cp "$here/builder/install/install.sh" "$kit_dir/install.sh"
  if [ -n "$2" ]; then
    fake_app "$kit_dir/Voice.app" "$2"
  fi
}

run() {
  status=0
  HOME="$home" bash "$kit_dir/install.sh" "$@" > "$scratch/out" 2> "$scratch/err" || status=$?
}

not_installed() {
  [ ! -e "$installed" ] || fail 'a refused copy was installed'
  [ ! -e "$home/voice-ran" ] || fail "a refused copy ran its voice"
}

# Installs, leaving the downloaded original alone and cleaning its staging folder.
kit installs "$arch"
source_app="$kit_dir/Voice.app"
xattr -w com.apple.quarantine '0081;00000000;Test;' "$source_app"
xattr -w com.apple.quarantine '0081;00000000;Test;' "$source_app/Contents/MacOS/voice-app"
run --no-open
[ "$status" = 0 ] || { cat "$scratch/err" >&2; fail 'install failed'; }
[ -d "$installed" ] || fail 'installed app is missing'
attributes=$(xattr -lr "$installed")
[[ "$attributes" != *com.apple.quarantine* ]] || fail 'installed copy is quarantined'
codesign --verify --deep --strict "$installed" || fail 'installed signature is invalid'
xattr -p com.apple.quarantine "$source_app" >/dev/null || fail 'source quarantine was removed'
xattr -p com.apple.quarantine "$source_app/Contents/MacOS/voice-app" >/dev/null || fail 'source executable quarantine was removed'
check "$scratch/out" "Voice is installed in your Applications folder: $installed" 'installed path not printed'
[ -z "$(find "$home/Applications" -name '.voice-install.*' -print)" ] || fail 'staging folder remains'
# The installed copy's own voice installs Radio, once.
ran=$(cat "$home/voice-ran" 2>/dev/null || true)
[ "$ran" = "$(printf 'radio\ninstall\n--')" ] || fail "Radio was not installed once through the installed voice: ${ran:-it never ran}"
check "$scratch/out" 'the stand-in Radio is installed' "voice's Radio sentence not printed"
echo 'mac_install_test: installs passed'

# Voice works without Radio, so a Radio that fails to install leaves Voice installed.
kit radio-fails "$arch"
FAKE_VOICE_EXIT=1 run --no-open
[ "$status" = 0 ] || { cat "$scratch/err" >&2; fail 'a failing Radio failed the install'; }
[ -d "$installed" ] || fail 'a failing Radio left Voice uninstalled'
ran=$(cat "$home/voice-ran" 2>/dev/null || true)
[ "$ran" = "$(printf 'radio\ninstall\n--')" ] || fail "Radio was not tried once: ${ran:-it never ran}"
check "$scratch/err" 'voice: the stand-in Radio failed' "voice's failure sentence not printed"
check "$scratch/err" 'Voice is installed and works without Radio. It will try to install Radio again each time you open it.' 'works-without-Radio line missing'
echo 'mac_install_test: a failing Radio passed'

# Replaces an old app, including files that the new app no longer contains.
kit replaces "$arch"
run --no-open
[ "$status" = 0 ] || fail 'first install before replacement failed'
printf 'old\n' > "$installed/old-marker"
run --no-open
[ "$status" = 0 ] || fail 'replacement failed'
[ ! -e "$installed/old-marker" ] || fail 'old copy was not replaced'
[ -x "$installed/Contents/MacOS/voice-app" ] || fail 'new executable is missing'
echo 'mac_install_test: replaces passed'

# Another app with the same name, or one without an identifier, must stay untouched.
collision_failed=false
for identity in different missing; do
  kit "collision-$identity" "$arch"
  fake_app "$installed" "$arch"
  if [ "$identity" = different ]; then
    /usr/libexec/PlistBuddy -c 'Set :CFBundleIdentifier dev.other.test' "$installed/Contents/Info.plist"
  else
    /usr/libexec/PlistBuddy -c 'Delete :CFBundleIdentifier' "$installed/Contents/Info.plist"
  fi
  ditto "$installed" "$scratch/before-$identity.app"
  run --no-open
  if [ "$status" != 1 ]; then
    echo "FAIL: $identity identifier was not refused" >&2
    collision_failed=true
  fi
  if ! grep -Fq "There's already an app called Voice in your Applications folder, and it isn't this one. Move it somewhere else, then run the installer again." "$scratch/err"; then
    echo "FAIL: $identity identifier explanation missing" >&2
    collision_failed=true
  fi
  if ! diff -r "$scratch/before-$identity.app" "$installed"; then
    echo "FAIL: $identity identifier app was changed" >&2
    collision_failed=true
  fi
done
"$collision_failed" && fail 'app-name collision checks failed'
echo 'mac_install_test: app-name collisions passed'

kit wrong-chip "$other_arch"
run --no-open
[ "$status" = 1 ] || fail 'wrong chip was not refused'
check "$scratch/err" 'Ask for a copy made for this Mac' 'wrong-chip explanation missing'
not_installed
echo 'mac_install_test: wrong chip passed'

# Re-sign each incomplete copy so the command check, not a broken signature, refuses it.
kit missing-command "$arch"
rm "$kit_dir/Voice.app/Contents/MacOS/voice"
codesign -s - --force --deep "$kit_dir/Voice.app" >/dev/null 2>&1
run --no-open
[ "$status" = 1 ] || fail 'missing command was not refused'
check "$scratch/err" 'This copy of Voice looks damaged. Ask for a new copy.' 'missing-command explanation missing'
not_installed
echo 'mac_install_test: missing command passed'

kit symlinked-command "$arch"
rm "$kit_dir/Voice.app/Contents/MacOS/voice"
ln -s voice-app "$kit_dir/Voice.app/Contents/MacOS/voice"
codesign -s - --force --deep "$kit_dir/Voice.app" >/dev/null 2>&1
run --no-open
[ "$status" = 1 ] || fail 'symlinked command was not refused'
check "$scratch/err" 'This copy of Voice looks damaged. Ask for a new copy.' 'symlinked-command explanation missing'
not_installed
echo 'mac_install_test: symlinked command passed'

# Radio ships inside the app; a copy without a usable one is damaged.
for broken in missing symlinked not-executable wrong-chip; do
  kit "radio-$broken" "$arch"
  radio="$kit_dir/Voice.app/Contents/MacOS/radio"
  case "$broken" in
    missing) rm "$radio" ;;
    symlinked) rm "$radio"; ln -s voice "$radio" ;;
    not-executable) chmod 644 "$radio" ;;
    wrong-chip) fake_app "$kit_dir/Voice.app" "$arch" "$other_arch" ;;
  esac
  codesign -s - --force --deep "$kit_dir/Voice.app" >/dev/null 2>&1
  run --no-open
  [ "$status" = 1 ] || fail "a $broken radio was not refused"
  check "$scratch/err" 'This copy of Voice looks damaged. Ask for a new copy.' "$broken-radio explanation missing"
  not_installed
done
echo 'mac_install_test: radio passed'

# The two signature checks and the quarantine re-check are belt and braces, so breaking only one of them does not fail this test.
kit damaged "$arch"
printf x >> "$kit_dir/Voice.app/Contents/MacOS/voice-app"
run --no-open
[ "$status" = 1 ] || fail 'damaged app was not refused'
check "$scratch/err" 'looks damaged' 'damage explanation missing'
not_installed
mkdir -p "$installed"
printf 'keep\n' > "$installed/old-marker"
run --no-open
[ "$status" = 1 ] || fail 'damaged app replaced an earlier app'
check "$scratch/err" 'looks damaged' 'damage explanation missing on replacement'
[ "$(cat "$installed/old-marker")" = keep ] || fail 'earlier app was changed'
[ ! -e "$installed/Contents" ] || fail 'damaged app was copied into earlier app'
echo 'mac_install_test: damaged passed'

kit no-app ''
run --no-open
[ "$status" = 1 ] || fail 'missing app was not refused'
check "$scratch/err" "isn't next to this installer" 'missing-app explanation missing'
not_installed
echo 'mac_install_test: no app passed'

kit symlinked ''
fake_app "$scratch/real app/Voice.app" "$arch"
ln -s "$scratch/real app/Voice.app" "$kit_dir/Voice.app"
run --no-open
[ "$status" = 1 ] || fail 'symlinked app was not refused'
check "$scratch/err" "isn't next to this installer" 'symlink explanation missing'
not_installed
echo 'mac_install_test: symlinked app passed'

kit bad-argument ''
run --open-now
[ "$status" = 2 ] || fail 'bad argument did not exit 2'
check "$scratch/err" 'Usage:' 'usage explanation missing'
not_installed
echo 'mac_install_test: bad argument passed'

# Symlink out of the bundle: refuse before quarantine removal can reach the outside file.
kit symlink-outside "$arch"
outside="$scratch/outside"
printf 'keep\n' > "$outside"
xattr -w com.apple.quarantine '0081;00000000;Test;' "$outside"
mkdir -p "$kit_dir/Voice.app/Contents/Resources"
ln -s "$outside" "$kit_dir/Voice.app/Contents/Resources/outside"
codesign -s - --force --deep "$kit_dir/Voice.app" >/dev/null 2>&1
run --no-open
[ "$status" = 1 ] || fail 'symlink out of the bundle was not refused'
check "$scratch/err" 'looks damaged' 'outside-symlink damage explanation missing'
not_installed
xattr -p com.apple.quarantine "$outside" >/dev/null || fail 'outside file quarantine was removed'
echo 'mac_install_test: symlink out of the bundle passed'
