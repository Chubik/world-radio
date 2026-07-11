#!/bin/sh
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
b="$here/bump-version.sh"

check() {
  got="$("$b" next "$2" "$3")"
  [ "$got" = "$1" ] || { echo "FAIL: next $2 $3 => $got (want $1)"; exit 1; }
  echo "ok: next $2 $3 => $got"
}

check 1.4.6 1.4.5 patch
check 1.5.0 1.4.5 minor
check 2.0.0 1.4.5 major
check 1.4.6 v1.4.5 patch      # tolerates leading v
echo "ALL PASS"
