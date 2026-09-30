; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/d/).
; Source: nvim-treesitter@728e031f6b11 queries/d (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
((comment) @injection.content
  (#set! injection.language "comment"))

((call_expression
  (type) @_printf
  (named_arguments
    "("
    .
    (named_argument
      (expression
        (string_literal) @injection.content))))
  (#eq? @_printf "printf")
  
  (#set! injection.language "printf"))

; TODO: uncomment when asm is added
; ((asm_inline) @injection.content
;   (#set! injection.language "asm")
;   (#set! injection.combined))
