; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/desktop/).
; Source: nvim-treesitter@728e031f6b11 queries/desktop (Apache-2.0)
((comment) @injection.content
  (#set! injection.language "comment"))

((entry
  key: (identifier) @_exec
  value: (string) @injection.content)
  (#eq? @_exec "Exec")
  (#set! injection.language "bash"))
