; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/sparql/).
; Source: nvim-treesitter@728e031f6b11 queries/sparql (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
((comment) @injection.content
  (#set! injection.language "comment"))

(regex_expression
  pattern: (rdf_literal
    value: (string) @injection.content)
  
  (#set! injection.language "regex"))
