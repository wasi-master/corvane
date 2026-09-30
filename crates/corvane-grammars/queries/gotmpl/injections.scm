; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/gotmpl/).
; Source: nvim-treesitter@728e031f6b11 queries/gotmpl (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
((comment) @injection.content
  (#set! injection.language "comment"))

; {{"put" | printf "%s%s" "out" | printf "%q"}}
(function_call
  function: (identifier) @_function
  arguments: (argument_list
    .
    (interpreted_string_literal) @injection.content)
  (#eq? @_function "printf")
  (#set! injection.language "printf"))

; {{ js "var a = 1 + 1" }}
(function_call
  function: (identifier) @_function
  arguments: (argument_list
    .
    (interpreted_string_literal) @injection.content)
  (#eq? @_function "js")
  
  (#set! injection.language "javascript"))

; {{ html "<h1>hello</h1>" }}
(function_call
  function: (identifier) @_function
  arguments: (argument_list
    .
    (interpreted_string_literal) @injection.content)
  (#eq? @_function "html")
  
  (#set! injection.language "html"))
