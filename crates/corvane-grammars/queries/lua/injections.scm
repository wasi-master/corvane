; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/lua/).
; Source: tree-sitter-lua 0.5.0 (MIT): queries/injections.scm
((function_call
  name: [
    (identifier) @_cdef_identifier
    (_
      _
      (identifier) @_cdef_identifier)
  ]
  arguments: (arguments
    (string
      content: _ @injection.content
      (#set! injection.language "c"))))
  (#eq? @_cdef_identifier "cdef"))
