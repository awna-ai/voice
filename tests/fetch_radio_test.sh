#!/usr/bin/env bash
set -euo pipefail

if [ "$(uname -s)" != Darwin ] || ! command -v cc >/dev/null 2>&1 || ! command -v lipo >/dev/null 2>&1; then
  echo 'fetch_radio_test: skipped (needs macOS with cc and lipo)'
  exit 0
fi

here="$(cd "$(dirname "$0")/.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }
check() { grep -Fq -- "$2" "$1" || { cat "$1" >&2; fail "$3"; }; }

if [ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" = 1 ]; then
  arch=arm64
else
  arch=x86_64
fi
tag=v9.8.7
releases="$scratch/releases"
mkdir -p "$releases" "$scratch/bin"

# A stand-in for curl that serves invented releases, so the test never touches the network.
cat > "$scratch/bin/curl" <<'CURL'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "$FAKE_CURL_LOG"
out=''
while [ "$#" -gt 1 ]; do
  if [ "$1" = -o ]; then out="$2"; shift; fi
  shift
done
url="$1"
prefix="https://github.com/awna-ai/radio/releases/download/$FAKE_TAG/"
[ "${url#"$prefix"}" != "$url" ] || exit 22
[ -f "$FAKE_RELEASES/${url#"$prefix"}" ] || exit 22
cp "$FAKE_RELEASES/${url#"$prefix"}" "$out"
CURL
chmod 755 "$scratch/bin/curl"

release() {  # architecture binary-architecture [no-radio|symlink|no-license]
  folder="$scratch/make/radio-$tag-macos-$1"
  rm -rf "$scratch/make"
  mkdir -p "$folder"
  case "${3-}" in
    no-radio) ;;
    symlink) ln -s LICENSE "$folder/radio" ;;
    *) printf 'int main(void){return 0;}\n' | cc -arch "$2" -x c - -o "$folder/radio" ;;
  esac
  [ "${3-}" = no-license ] || printf 'MIT License\n\nCopyright (c) Radio test\n' > "$folder/LICENSE"
  tar -czf "$releases/radio-$tag-macos-$1.tar.gz" -C "$scratch/make" "radio-$tag-macos-$1"
}

sum() { shasum -a 256 "$releases/radio-$tag-macos-$1.tar.gz" | cut -d ' ' -f 1; }

pin() {  # writes the pin file from the releases as they are now
  {
    echo '# The Radio release Voice ships.'
    echo "tag $tag"
    echo "arm64 $(sum arm64)"
    echo "x86_64 $(sum x86_64)"
  } > "$scratch/pin"
}

run() {
  status=0
  PATH="$scratch/bin:$PATH" FAKE_CURL_LOG="$scratch/curl.log" FAKE_TAG="$tag" FAKE_RELEASES="$releases" \
    bash "$here/scripts/fetch-radio.sh" "$@" > "$scratch/stdout" 2> "$scratch/err" || status=$?
}

refused() {  # message what
  [ "$status" = 1 ] || { cat "$scratch/err" >&2; fail "$2 was not refused"; }
  check "$scratch/err" "$1" "$2 explanation missing"
}

release arm64 arm64
release x86_64 x86_64
pin

# One architecture: the binary and Radio's licence, downloaded over HTTPS only.
: > "$scratch/curl.log"
run "$scratch/pin" "$scratch/fetched" "$arch"
[ "$status" = 0 ] || { cat "$scratch/err" >&2; fail 'fetch failed'; }
[ -f "$scratch/fetched/radio" ] && [ -x "$scratch/fetched/radio" ] || fail 'radio was not fetched'
[ "$(lipo -archs "$scratch/fetched/radio")" = "$arch" ] || fail 'radio has the wrong architecture'
check "$scratch/fetched/LICENSE" 'MIT License' "Radio's licence was not fetched"
check "$scratch/curl.log" "--proto =https" 'the download was not limited to HTTPS'
check "$scratch/curl.log" "https://github.com/awna-ai/radio/releases/download/$tag/radio-$tag-macos-$arch.tar.gz" 'the pinned tag was not downloaded'
echo 'fetch_radio_test: one architecture passed'

# Both architectures: one universal binary.
run "$scratch/pin" "$scratch/universal" arm64 x86_64
[ "$status" = 0 ] || { cat "$scratch/err" >&2; fail 'universal fetch failed'; }
archs=" $(lipo -archs "$scratch/universal/radio") "
[[ "$archs" == *' arm64 '* && "$archs" == *' x86_64 '* ]] || fail "universal radio holds$archs"
echo 'fetch_radio_test: universal passed'

# A tarball that isn't the pinned one is refused, and nothing is left behind.
printf 'tampered' >> "$releases/radio-$tag-macos-$arch.tar.gz"
run "$scratch/pin" "$scratch/tampered" "$arch"
refused "radio-$tag-macos-$arch.tar.gz doesn't match its pinned SHA-256" 'a tampered download'
[ ! -e "$scratch/tampered" ] || fail 'a refused fetch left its folder'
release "$arch" "$arch"
pin
echo 'fetch_radio_test: tampered passed'

other=x86_64
[ "$arch" = arm64 ] || other=arm64
release "$arch" "$other"
pin
run "$scratch/pin" "$scratch/wrong-chip" "$arch"
refused "radio in radio-$tag-macos-$arch.tar.gz isn't built for $arch" 'a binary for the other chip'
echo 'fetch_radio_test: wrong chip passed'

for broken in no-radio symlink; do
  release "$arch" "$arch" "$broken"
  pin
  run "$scratch/pin" "$scratch/$broken" "$arch"
  refused "radio-$tag-macos-$arch.tar.gz holds no radio binary" "a tarball with $broken"
done
release "$arch" "$arch" no-license
pin
run "$scratch/pin" "$scratch/no-license" "$arch"
refused "radio-$tag-macos-$arch.tar.gz holds no LICENSE" 'a tarball without a licence'
release "$arch" "$arch"
pin
echo 'fetch_radio_test: broken tarballs passed'

mv "$releases/radio-$tag-macos-$arch.tar.gz" "$scratch/away.tar.gz"
run "$scratch/pin" "$scratch/missing" "$arch"
refused "couldn't download radio-$tag-macos-$arch.tar.gz" 'a failed download'
mv "$scratch/away.tar.gz" "$releases/radio-$tag-macos-$arch.tar.gz"
echo 'fetch_radio_test: failed download passed'

grep -v '^tag ' "$scratch/pin" > "$scratch/no-tag"
run "$scratch/no-tag" "$scratch/pin-no-tag" "$arch"
refused 'the pin file has no valid tag line' 'a pin without a tag'
sed "s/^$arch .*/$arch abc/" "$scratch/pin" > "$scratch/short-sum"
run "$scratch/short-sum" "$scratch/pin-short" "$arch"
refused "the pin file has no valid SHA-256 for $arch" 'a short checksum'
{ cat "$scratch/pin"; echo "tag v0.0.1"; } > "$scratch/two-tags"
run "$scratch/two-tags" "$scratch/pin-two" "$arch"
refused 'the pin file has no valid tag line' 'a pin with two tags'
echo 'fetch_radio_test: pin file passed'

mkdir "$scratch/exists"
run "$scratch/pin" "$scratch/exists" "$arch"
refused "$scratch/exists already exists" 'an existing folder'
for args in '' 'only-a-pin' "$scratch/pin $scratch/bad-arch arm64e"; do
  # shellcheck disable=SC2086
  run $args
  [ "$status" = 2 ] || fail "bad arguments ($args) did not exit 2"
  check "$scratch/err" 'Usage:' 'usage explanation missing'
done
echo 'fetch_radio_test: arguments passed'

# The repo's own pin is well formed.
for line in tag arm64 x86_64; do
  [ "$(grep -c "^$line " "$here/builder/install/radio-release.txt")" = 1 ] || fail "the repo's pin needs one $line line"
done
grep -Eq '^tag v[0-9]+\.[0-9]+\.[0-9]+$' "$here/builder/install/radio-release.txt" || fail "the repo's pin has a bad tag"
[ "$(grep -Ec '^(arm64|x86_64) [0-9a-f]{64}$' "$here/builder/install/radio-release.txt")" = 2 ] || fail "the repo's pin has a bad checksum"
echo 'fetch_radio_test: the repo pin passed'
