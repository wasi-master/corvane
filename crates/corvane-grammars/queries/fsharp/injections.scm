; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/fsharp/).
; Source: nvim-treesitter@728e031f6b11 queries/fsharp (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
([
  (line_comment)
  (block_comment_content)
] @injection.content
  (#set! injection.language "comment"))

((line_comment) @injection.content
  (#match? @injection.content "^///")
  
  (#set! injection.language "xml")
  (#set! injection.combined))
