; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/git_rebase/).
; Source: nvim-treesitter@728e031f6b11 queries/git_rebase (Apache-2.0)
((comment) @injection.content
  (#set! injection.language "comment"))

((operation
  (command) @_command
  (message) @injection.content)
  (#set! injection.language "bash")
  (#any-of? @_command "exec" "x"))
