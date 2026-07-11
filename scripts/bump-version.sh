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

stamp() {
  ver="$(echo "$1" | sed 's/^v//')"
  root="$(cd "$(dirname "$0")/.." && pwd)"
  # workspace version (source of truth)
  sed -i.bak -E "s/^(version = \")[0-9]+\.[0-9]+\.[0-9]+(\")/\1${ver}\2/" "$root/Cargo.toml"
  rm -f "$root/Cargo.toml.bak"
  # android versionName + bump versionCode
  gradle="$root/android/app/build.gradle.kts"
  sed -i.bak -E "s/(versionName = \")[0-9]+\.[0-9]+\.[0-9]+(\")/\1${ver}\2/" "$gradle"
  code="$(grep -oE 'versionCode = [0-9]+' "$gradle" | grep -oE '[0-9]+')"
  newcode=$((code + 1))
  sed -i.bak -E "s/versionCode = [0-9]+/versionCode = ${newcode}/" "$gradle"
  rm -f "$gradle.bak"
  echo "stamped ${ver} (versionCode ${newcode})"
}

case "$cmd" in
  next)  next "${2:?current}" "${3:?level}" ;;
  stamp) stamp "${2:?version}" ;;
  *) echo "usage: bump-version.sh next <current> <level> | stamp <version>" >&2; exit 1 ;;
esac
