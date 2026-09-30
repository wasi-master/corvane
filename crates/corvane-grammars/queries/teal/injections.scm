; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/teal/).
; Source: nvim-treesitter@728e031f6b11 queries/teal (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
((function_call
  (index
    (identifier) @_cdef_identifier)
  (arguments
    (string) @injection.content))
  (#eq? @_cdef_identifier "cdef")
  (#match? @injection.content "^[\"']")
  
  (#set! injection.language "c"))

((function_call
  (index
    (identifier) @_cdef_identifier)
  (arguments
    (string) @injection.content))
  (#eq? @_cdef_identifier "cdef")
  (#match? @injection.content "^\\[\\[")
  
  (#set! injection.language "c"))

; string.format('...')
((function_call
  (index
    (identifier) @_base
    key: (identifier) @_entry)
  (arguments
    .
    (string) @injection.content))
  (#eq? @_base "string")
  (#eq? @_entry "format")
  (#set! injection.language "printf"))

; ('...'):format()
((function_call
  (method_index
    (parenthesized_expression
      (string) @injection.content)
    key: (identifier) @_func))
  (#eq? @_func "format")
  (#set! injection.language "printf"))

((comment) @injection.content
  (#set! injection.language "comment"))
