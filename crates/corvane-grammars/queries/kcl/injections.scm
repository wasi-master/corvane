; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/kcl/).
; Source: nvim-treesitter@728e031f6b11 queries/kcl (Apache-2.0)
(call_expr
  function: (selector_expr
    (identifier) @_regex)
  arguments: (argument_list
    (_)
    .
    (string
      (string_content) @injection.content))
  (#eq? @_regex "regex")
  (#set! injection.language "regex"))

(call_expr
  function: (selector_expr
    (identifier) @_regex
    (select_suffix
      (identifier) @_fn
      (#eq? @_fn "compile")))
  arguments: (argument_list
    .
    (string
      (string_content) @injection.content))
  (#eq? @_regex "regex")
  (#set! injection.language "regex"))

((comment) @injection.content
  (#set! injection.language "comment"))
