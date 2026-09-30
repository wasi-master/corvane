; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/gitcommit/).
; Source: nvim-treesitter@728e031f6b11 queries/gitcommit (Apache-2.0)
((diff) @injection.content
  (#set! injection.language "diff"))

((rebase_command) @injection.content
  (#set! injection.language "git_rebase"))
