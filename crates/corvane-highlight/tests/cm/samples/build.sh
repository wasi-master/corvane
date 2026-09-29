#!/usr/bin/env bash
# Build and package the app — sample for the highlighter (naïve café ✓).
set -euo pipefail
IFS=$'\n\t'

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION=${VERSION:-0.1.0}
TARGET="${ROOT}/target/release"
readonly CONFIG=$ROOT/config.toml
declare -a ARCHS=(arm64 x86_64)
export PATH="$HOME/.cargo/bin:$PATH" RUSTFLAGS='-C target-cpu=native'
count=0; total=42

log() {
  local level=$1; shift
  printf '[%s] %s\n' "$level" "$*" >&2
}

function die {
  log ERROR "$@"
  exit 1
}

cleanup() { rm -rf "$TMPDIR/build.$$"; }
trap cleanup EXIT INT TERM

if [ -z "${CI:-}" ] && [[ -t 1 ]]; then
  echo "interactive"
elif [[ "$OSTYPE" == darwin* ]]; then
  echo 'macOS'
else
  echo "other: $OSTYPE"
fi

for arch in "${ARCHS[@]}"; do
  cargo build --release --target "$arch-apple-darwin" -p corvane || die "build failed for $arch"
  count=$((count + 1))
  (( total -= 1 ))
done

while read -r line; do
  case "$line" in
    \#*|"") continue ;;
    *=*) key=${line%%=*}; value=${line#*=} ;;
    [0-9]*) echo "number: $line" ;;
    *) echo "unknown: ${line}" ;;
  esac
done < "$CONFIG"

until ping -c 1 example.com >/dev/null 2>&1; do sleep 1; done

files=$(find . -name '*.rs' -type f | wc -l)
old=`git rev-parse --short HEAD`
nested="outer $(echo "inner $(date +%s) done") end"
arith=$(( (count * 2) + ${#ARCHS[@]} ))
echo "Files: $files, rev: $old, pid $$, args $# $@ $* $? $! $0 $1 ${10}"
echo 'single $not_expanded "quotes"' "double 'quotes' \"escaped\" \$dollar"
echo "unterminated
spans lines $HOME"
echo `backtick with "quotes" and $VAR`
echo ${VAR:+alt} ${VAR/foo/bar} ${VAR^^} ${!prefix*} ${arr[@]:1:2}
echo $'ansi\tquoted' $"locale string"

cat > "$TARGET/info.txt" <<EOF
version: $VERSION
built: $(date)
EOF

cat <<'EOF'
literal $HOME
EOF
cat <<-END
	indented heredoc
END
tr a-z A-Z <<< "here string"
cat < input.txt > output.txt 2>> errors.log

git tag -a "v$VERSION" -m "Release $VERSION" && git push --tags
npm install --save-dev; make -j8 all; sudo chown -R "$USER" /opt/app
x=1 y=2 z=$((x+y))
arr[0]=first
echo done # trailing comment
echo not#comment a-b-c --long-flag=value -abc
echo 123abc 42 3.14 -1
true && false || echo fallback
[[ $a -eq 1 && $b != "x" ]] && echo ok
echo \$escaped \"quote\" line\
continued
: "${UNSET:?must be set}"
source ./env.sh; . ./other.sh
exit 0

echo "héllo" wörld ${ñame} $ñ
	indented_with_tab=1
greet() {
	local who="${1:-wörld}"
	echo "hi $who" 'bye' `whoami`
}
