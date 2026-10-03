; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/squirrel/).
; Source: nvim-treesitter@728e031f6b11 queries/squirrel (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
((comment) @injection.content
  (#set! injection.language "comment"))

((verbatim_string) @injection.content
  (#match? @injection.content "^@\"<html")
  (#set! injection.language "html")
  )

((verbatim_string) @injection.content
  (#match? @injection.content "@\"<!DOCTYPE html>")
  (#set! injection.language "html")
  )
