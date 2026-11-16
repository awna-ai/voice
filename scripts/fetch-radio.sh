#!/usr/bin/env bash
# Fetches the Radio release Voice ships inside Voice.app, checked against a pin file
# (builder/install/radio-release.txt), into a new folder: radio (one binary for the given
# architectures) and Radio's LICENSE.
set -euo pipefail

usage() { echo 'Usage: bash scripts/fetch-radio.sh <pin file> <new folder> arm64|x86_64...' >&2; exit 2; }
fail() { echo "fetch-radio: $*" >&2; exit 1; }
[ "$#" -ge 3 ] || usage
pin=$1
out=$2
shift 2
seen=' '
for arch in "$@"; do
  case "$arch" in arm64|x86_64) ;; *) usage ;; esac
  [[ "$seen" != *" $arch "* ]] || usage
  seen+="$arch "
done
[ -f "$pin" ] || fail "no pin file at $pin"
[ ! -e "$out" ] && [ ! -L "$out" ] || fail "$out already exists"
[ -d "$(dirname "$out")" ] || fail "no folder to put $out in"

pinned() {  # key pattern: the key's one value in the pin file, when it matches the pattern
  local values
  values=$(awk -v key="$1" '$1 == key { n++; value = (NF == 2 ? $2 : "") } END { if (n == 1) print value }' "$pin")
  [[ "$values" =~ $2 ]] && printf '%s\n' "$values"
}
tag=$(pinned tag '^v[0-9]+\.[0-9]+\.[0-9]+$') || fail 'the pin file has no valid tag line'

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
binaries=()
for arch in "$@"; do
  sum=$(pinned "$arch" '^[0-9a-f]{64}$') || fail "the pin file has no valid SHA-256 for $arch"
  name="radio-$tag-macos-$arch"
  tarball="$work/$name.tar.gz"
  curl -fsSL --proto '=https' --tlsv1.2 --max-filesize 67108864 --max-time 600 -o "$tarball" \
    "https://github.com/awna-ai/radio/releases/download/$tag/$name.tar.gz" \
    || fail "couldn't download $name.tar.gz"
  [ "$(shasum -a 256 "$tarball" | cut -d ' ' -f 1)" = "$sum" ] || fail "$name.tar.gz doesn't match its pinned SHA-256"
  members=$(tar -tzf "$tarball") || fail "couldn't unpack $name.tar.gz"
  grep -qx "$name/radio" <<< "$members" || fail "$name.tar.gz holds no radio binary"
  grep -qx "$name/LICENSE" <<< "$members" || fail "$name.tar.gz holds no LICENSE"
  mkdir "$work/$arch"
  # Only the two named members come out, so nothing else in the tarball can land anywhere.
  tar -xzf "$tarball" -C "$work/$arch" "$name/radio" "$name/LICENSE" || fail "couldn't unpack $name.tar.gz"
  binary="$work/$arch/$name/radio"
  [ -f "$binary" ] && [ ! -L "$binary" ] || fail "$name.tar.gz holds no radio binary"
  [ -f "$work/$arch/$name/LICENSE" ] && [ ! -L "$work/$arch/$name/LICENSE" ] || fail "$name.tar.gz holds no LICENSE"
  [ "$(lipo -archs "$binary" 2>/dev/null)" = "$arch" ] || fail "radio in $name.tar.gz isn't built for $arch"
  binaries+=("$binary")
  license="$work/$arch/$name/LICENSE"
done

mkdir "$work/out"
if [ "${#binaries[@]}" -gt 1 ]; then
  lipo -create -output "$work/out/radio" "${binaries[@]}"
else
  cp "${binaries[0]}" "$work/out/radio"
fi
chmod 755 "$work/out/radio"
cp "$license" "$work/out/LICENSE"
chmod 644 "$work/out/LICENSE"
mv "$work/out" "$out"
echo "fetch-radio: radio $tag for $* in $out"
