#!/bin/sh
# Corvane command line tool (GitHub Desktop's static/darwin/github.sh):
#   corvane [path]   open the repository at <path> (default: the current
#                    directory) in Corvane
# Installed by Corvane › Install Command Line Tool… as a symlink to
# Corvane.app/Contents/Resources/corvane.

# The least terrible way to resolve a symlink to its real path.
realpath() {
  /usr/bin/perl -e 'use Cwd; print Cwd::abs_path($ARGV[0])' "$1"
}

CONTENTS="$(dirname "$(dirname "$(realpath "$0")")")"
APP="$(dirname "$CONTENTS")"

TARGET="${1:-.}"
if ! TARGET="$(cd "$TARGET" 2>/dev/null && pwd -P)"; then
  echo "corvane: $1: no such directory" >&2
  exit 1
fi

exec /usr/bin/open -a "$APP" --args --open-repo "$TARGET"
