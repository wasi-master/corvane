; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/wing/).
; Source: nvim-treesitter@728e031f6b11 queries/wing (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
((comment) @injection.content
  (#set! injection.language "comment"))

(call
  (reference
    (nested_identifier
      object: (reference) @_ref
      property: (member_identifier) @_ident))
  (argument_list
    (positional_argument
      (string) @injection.content))
  (#eq? @_ref "regex")
  (#eq? @_ident "compile")
  
  (#set! injection.language "regex"))
