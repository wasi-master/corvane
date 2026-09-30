; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/yang/).
; Source: nvim-treesitter@728e031f6b11 queries/yang (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
((comment) @injection.content
  (#set! injection.language "comment"))

((statement
  (statement_keyword
    "pattern")
  (argument
    (string) @injection.content))
  (#set! injection.language "regex")
  )
