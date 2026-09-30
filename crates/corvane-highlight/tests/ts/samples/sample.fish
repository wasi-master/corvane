#!/usr/bin/env fish
# Project helper functions for an interactive fish session.

set -g PROJECT_ROOT ~/src/project
set -gx EDITOR nvim
set -l retries 3

function mkcd --description 'Create a directory and enter it'
    if test (count $argv) -ne 1
        echo "usage: mkcd DIR" >&2
        return 1
    end
    mkdir -p -- $argv[1]; and cd $argv[1]
end

function git_branch_summary
    set -l branch (git rev-parse --abbrev-ref HEAD 2>/dev/null)
    or return
    set -l dirty (git status --porcelain | count)
    printf '%s (%d changed)\n' $branch $dirty
end

function retry -a times
    for i in (seq $times)
        eval $argv[2..-1]; and return 0
        echo "attempt $i of $times failed" 1>&2
        sleep (math "$i * 0.5")
    end
    return 1
end

switch (uname)
    case Darwin
        set -gx BROWSER open
    case Linux '*BSD'
        set -gx BROWSER xdg-open
    case '*'
        echo "unknown platform"
end

abbr --add gs 'git status --short'
while not test -d $PROJECT_ROOT/.git; and test $retries -gt 0
    set retries (math $retries - 1)
end
string match -qr '^v\d+' -- (cat VERSION 2>/dev/null); or echo "no version"
