; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/v/).
; Source: nvim-treesitter@728e031f6b11 queries/v (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
([
  (line_comment)
  (block_comment)
] @injection.content
  (#set! injection.language "comment"))

; asm_statement if asm ever highlighted :)
; #include <...>
((hash_statement) @injection.content
  (#set! injection.language "c"))

; regex for the methods defined in `re` module
((call_expression
  name: (selector_expression
    field: (reference_expression
      (identifier) @_re))
  arguments: (argument_list
    (argument
      (literal
        (raw_string_literal) @injection.content
        ))))
  (#any-of? @_re "regex_base" "regex_opt" "compile_opt")
  (#set! injection.language "regex"))
