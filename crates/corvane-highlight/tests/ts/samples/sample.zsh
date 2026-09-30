#!/usr/bin/env zsh
# Interactive helpers: prompt, completion and a few utility functions.

setopt extended_glob prompt_subst no_beep
autoload -Uz compinit vcs_info && compinit -C

typeset -gA ALIASES=(
  gs 'git status -sb'
  gl 'git log --oneline -20'
)
typeset -ga PROJECT_DIRS=(~/src/*(N/) ~/work/*(N/))
integer -g MAX_HISTORY=50000
export HISTSIZE=$MAX_HISTORY SAVEHIST=$MAX_HISTORY

for name cmd in ${(kv)ALIASES}; do
  alias $name=$cmd
done

zstyle ':vcs_info:git:*' formats ' %F{magenta}(%b)%f'
precmd() { vcs_info }
PROMPT='%F{cyan}%~%f${vcs_info_msg_0_} %(?.%F{green}.%F{red})%#%f '

# Jump to a project by fuzzy name.
proj() {
  local match=${PROJECT_DIRS[(r)*$1*]}
  if [[ -z $match ]]; then
    print -u2 "no project matching '$1'"
    return 1
  fi
  cd -- $match && print -P "%B${match:t}%b"
}

function extract {
  case ${1:e} in
    (gz|tgz) tar xzf $1 ;;
    zip)     unzip -q $1 ;;
    *)       echo "unsupported: ${1:e}" >&2; return 2 ;;
  esac
}

(( $+commands[bat] )) && alias cat='bat --paging=never'
bindkey '^R' history-incremental-search-backward
repeat 2 { print -n "." }; print
