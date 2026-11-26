#!/bin/sh
# Runs the everyday commands of a built cprofile in an empty, throwaway home
# and prints how each one ends. Nothing here touches a real Claude login.
#
#   scripts/smoke.sh target/release/cprofile
#
# It exists to see what works on a system we cannot try by hand (Windows):
# every command runs even when an earlier one fails, and the script fails at
# the end if any did.
set -u

bin="${1:?usage: scripts/smoke.sh <path to cprofile>}"
home="$(mktemp -d)"
export CLAUDE_PROFILES_HOME="$home/claude-profiles"
failed=0

try() {
    echo
    echo "\$ cprofile $*"
    # stdin is closed so that no command waits for an answer.
    "$bin" "$@" </dev/null
    code=$?
    echo "-> exit $code"
    [ "$code" -eq 0 ] || failed=$((failed + 1))
}

try --version
try setup
try add work
try add client --keep-days 90
try list
try default work
try status
try retention
try disk
try rule add --path "$home/projects" --profile work
try rule list
try remove client --yes

# doctor reports the machine itself (PATH, Claude Code installed): on a bare
# runner it has problems to list, so its exit code is shown but not counted.
echo
echo "\$ cprofile doctor"
"$bin" doctor </dev/null
echo "-> exit $? (not counted)"

rm -rf "$home"
echo
echo "$failed command(s) failed"
[ "$failed" -eq 0 ]
