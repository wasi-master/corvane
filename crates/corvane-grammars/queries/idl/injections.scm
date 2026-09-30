; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/idl/).
; Source: nvim-treesitter@728e031f6b11 queries/idl (Apache-2.0)
((comment) @injection.content
  (#match? @injection.content "/[*/][!*/]<?[^a-zA-Z]")
  (#set! injection.language "doxygen"))

((comment) @injection.content
  (#not-match? @injection.content "/[*/][!*/]<?[^a-zA-Z]")
  (#not-match? @injection.content "//@[a-zA-Z]")
  (#set! injection.language "comment"))
