#!/bin/sh
# Corvane command line tool for Linux (GitHub Desktop's app/src/cli/main.ts;
# packaging/corvane.sh is the macOS twin):
#   corvane                            open the current directory
#   corvane open [path]                open the provided path
#   corvane clone [-b branch] <url>    clone the repository by url or
#                                      owner/name (ex torvalds/linux),
#                                      optionally checking out the branch
# Each command becomes an x-corvane:// URL handed to the Corvane binary,
# which passes it to the running Corvane (single instance) or starts one.
# Installed as <prefix>/lib/corvane/bin/corvane next to the binary
# (<prefix>/lib/corvane/corvane); Help › Install Command Line Tool… links it
# into ~/.local/bin. CORVANE_BIN overrides the binary (AppImage, tests).

# Percent-encode everything but unreserved characters and "/" (UTF-8 bytes).
urlencode() {
  perl -e '$_ = $ARGV[0]; s/([^A-Za-z0-9\-._~\/])/sprintf("%%%02X", ord($1))/ge; print' "$1"
}

# A remote URL goes into the URL path as is, except what would end it.
encode_remote() {
  perl -e '$_ = $ARGV[0]; s/([%?# ])/sprintf("%%%02X", ord($1))/ge; print' "$1"
}

usage() {
  cat >&2 <<'USAGE'
Corvane CLI usage:
  corvane                            Open the current directory
  corvane open [path]                Open the provided path
  corvane clone [-b branch] <url>    Clone the repository by url or name/owner
                                     (ex torvalds/linux), optionally checking out
                                     the branch
USAGE
  exit "$1"
}

if [ -z "$CORVANE_BIN" ]; then
  HERE="$(dirname "$(readlink -f "$0")")"
  CORVANE_BIN="$(dirname "$HERE")/corvane"
fi

send() {
  if [ -n "$CORVANE_CLI_DRY_RUN" ]; then
    printf '%s\n' "$1"
    exit 0
  fi
  # detached from the terminal, like GitHub Desktop's CLI
  nohup "$CORVANE_BIN" "$1" </dev/null >/dev/null 2>&1 &
  exit 0
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
    if printf '%s' "$URL" | grep -Eq '^[^/]+/[^/]+$'; then
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
