#!/bin/sh
# single source of version arithmetic + file stamping for the project.
set -eu

cmd="${1:-}"

next() {
  cur="$(echo "$1" | sed 's/^v//')"
  level="$2"
  maj="$(echo "$cur" | cut -d. -f1)"
  min="$(echo "$cur" | cut -d. -f2)"
  pat="$(echo "$cur" | cut -d. -f3)"
  case "$level" in
    major) maj=$((maj + 1)); min=0; pat=0 ;;
    minor) min=$((min + 1)); pat=0 ;;
    patch) pat=$((pat + 1)) ;;
    *) echo "unknown level: $level" >&2; exit 1 ;;
  esac
  echo "${maj}.${min}.${pat}"
}

# android only ever compares versionCode, and only accepts an update whose code
# grew. deriving it from the version keeps it tied to the one source of truth —
# the tag — instead of a counter in a file main never receives.
code() {
  ver="$(echo "$1" | sed 's/^v//')"
  maj="$(echo "$ver" | cut -d. -f1)"
  min="$(echo "$ver" | cut -d. -f2)"
  pat="$(echo "$ver" | cut -d. -f3)"
  for part in "$min" "$pat"; do
    [ "$part" -lt 100 ] || {
      echo "version component $part >= 100 would carry into the next field: $ver" >&2
      exit 1
    }
  done
  echo $((maj * 10000 + min * 100 + pat))
}

stamp() {
  ver="$(echo "$1" | sed 's/^v//')"
  root="$(cd "$(dirname "$0")/.." && pwd)"
  # workspace version (source of truth)
  sed -i.bak -E "s/^(version = \")[0-9]+\.[0-9]+\.[0-9]+(\")/\1${ver}\2/" "$root/Cargo.toml"
  rm -f "$root/Cargo.toml.bak"
  # android versionName + bump versionCode
  gradle="$root/android/app/build.gradle.kts"
  sed -i.bak -E "s/(versionName = \")[0-9]+\.[0-9]+\.[0-9]+(\")/\1${ver}\2/" "$gradle"
  newcode="$(code "$ver")"
  sed -i.bak -E "s/versionCode = [0-9]+/versionCode = ${newcode}/" "$gradle"
  rm -f "$gradle.bak"
  # tauri keeps its own version field; leaving it stale ships a bundle whose
  # about-box disagrees with the release it came from.
  tauri="$root/crates/r4dio-macos/tauri.conf.json"
  sed -i.bak -E "s/(\"version\": \")[0-9]+\.[0-9]+\.[0-9]+(\")/\1${ver}\2/" "$tauri"
  rm -f "$tauri.bak"
  echo "stamped ${ver} (versionCode ${newcode})"
}

case "$cmd" in
  next)  next "${2:?current}" "${3:?level}" ;;
  code)  code "${2:?version}" ;;
  stamp) stamp "${2:?version}" ;;
  *) echo "usage: bump-version.sh next <current> <level> | code <version> | stamp <version>" >&2; exit 1 ;;
esac
