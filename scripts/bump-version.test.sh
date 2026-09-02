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

# versionCode must come from the version itself. deriving it by incrementing
# the file failed silently: main never carries the bump (branch protection), so
# every build read the same number and 1.22.6 through 1.22.10 all shipped
# versionCode 10 — android refuses to install an update whose code did not grow.
codecheck() {
  got="$("$b" code "$2")"
  [ "$got" = "$1" ] || { echo "FAIL: code $2 => $got (want $1)"; exit 1; }
  echo "ok: code $2 => $got"
}

codecheck 10405 1.4.5
codecheck 12210 1.22.10
codecheck 12211 1.22.11
codecheck 12300 1.23.0
codecheck 20000 2.0.0
codecheck 12210 v1.22.10      # tolerates leading v

# strictly greater than everything already published, or the update is refused
[ "$("$b" code 1.22.11)" -gt 10 ] || { echo "FAIL: new code must beat the shipped 10"; exit 1; }
echo "ok: new code beats the shipped 10"

# monotonic across a release sequence
prev=0
for v in 1.22.6 1.22.9 1.22.10 1.22.11 1.23.0 2.0.0; do
  c="$("$b" code "$v")"
  [ "$c" -gt "$prev" ] || { echo "FAIL: $v code $c did not grow past $prev"; exit 1; }
  prev="$c"
done
echo "ok: codes grow monotonically"

# a component at 100 would carry into the next field and break ordering
if "$b" code 1.100.0 >/dev/null 2>&1; then
  echo "FAIL: minor=100 must be rejected, it collides with the next major"; exit 1
fi
echo "ok: rejects a component that would overflow its field"

echo "ALL PASS"
