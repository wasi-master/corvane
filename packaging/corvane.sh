#!/bin/sh
# Corvane command line tool (GitHub Desktop's app/src/cli/main.ts and
# static/darwin/github.sh):
#   corvane                            open the current directory
#   corvane open [path]                open the provided path
#   corvane clone [-b branch] <url>    clone the repository by url or
#                                      owner/name (ex torvalds/linux),
#                                      optionally checking out the branch
# Each command becomes an x-corvane:// URL handed to Corvane.app, so a
# running Corvane receives it instead of a second instance starting.
# Installed by Corvane › Install Command Line Tool… as a symlink to
# Corvane.app/Contents/Resources/corvane.

# The least terrible way to resolve a symlink to its real path.
realpath() {
  /usr/bin/perl -e 'use Cwd; print Cwd::abs_path($ARGV[0])' "$1"
}

# Percent-encode everything but unreserved characters and "/" (UTF-8 bytes).
urlencode() {
  /usr/bin/perl -e '$_ = $ARGV[0]; s/([^A-Za-z0-9\-._~\/])/sprintf("%%%02X", ord($1))/ge; print' "$1"
}

# A remote URL goes into the URL path as is, except what would end it.
encode_remote() {
  /usr/bin/perl -e '$_ = $ARGV[0]; s/([%?# ])/sprintf("%%%02X", ord($1))/ge; print' "$1"
}

usage() {
  cat >&2 <<'EOF'
Corvane CLI usage:
  corvane                            Open the current directory
  corvane open [path]                Open the provided path
  corvane clone [-b branch] <url>    Clone the repository by url or name/owner
                                     (ex torvalds/linux), optionally checking out
                                     the branch
EOF
  exit "$1"
}

CONTENTS="$(dirname "$(dirname "$(realpath "$0")")")"
APP="$(dirname "$CONTENTS")"

send() {
  exec /usr/bin/open -a "$APP" "$1"
}

case "$1" in
  -h|--help|help)
    usage 0
    ;;
  clone)
    shift
    URL=""
    BRANCH=""
    while [ $# -gt 0 ]; do
      case "$1" in
        -b|--branch)
          [ $# -ge 2 ] || usage 1
          BRANCH="$2"
          shift 2
          ;;
        --branch=*)
          BRANCH="${1#--branch=}"
          shift
          ;;
        -h|--help)
          usage 0
          ;;
        *)
          [ -z "$URL" ] || usage 1
          URL="$1"
          shift
          ;;
      esac
    done
    [ -n "$URL" ] || usage 1
    # Assume name with owner slug if it looks like it
    if printf '%s' "$URL" | /usr/bin/grep -Eq '^[^/]+/[^/]+$'; then
      URL="https://github.com/$URL"
    fi
    TARGET="x-corvane://openRepo/$(encode_remote "$URL")"
    if [ -n "$BRANCH" ]; then
      TARGET="$TARGET?branch=$(urlencode "$BRANCH")"
    fi
    send "$TARGET"
    ;;
  *)
    if [ "$1" = "open" ]; then
      shift
    fi
    DIR="${1:-.}"
    if ! DIR="$(cd "$DIR" 2>/dev/null && pwd -P)"; then
      echo "corvane: $1: no such directory" >&2
      exit 1
    fi
    send "x-corvane://openLocalRepo$(urlencode "$DIR")"
    ;;
esac
