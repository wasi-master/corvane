; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/bash/).
; Source: nvim-treesitter@728e031f6b11 queries/bash (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
((comment) @injection.content
  (#set! injection.language "comment"))

((regex) @injection.content
  (#set! injection.language "regex"))

(heredoc_redirect
  (heredoc_body) @injection.content
  (heredoc_end) @injection.language)

; printf 'format'
((command
  name: (command_name) @_command
  .
  argument: [
    (string) @injection.content
    (concatenation
      (string) @injection.content)
    (raw_string) @injection.content
    (concatenation
      (raw_string) @injection.content)
  ])
  (#eq? @_command "printf")
  
  (#set! injection.include-children)
  (#set! injection.language "printf"))

; printf -v var 'format'
((command
  name: (command_name) @_command
  argument: (word) @_arg
  .
  (_)
  .
  argument: [
    (string) @injection.content
    (concatenation
      (string) @injection.content)
    (raw_string) @injection.content
    (concatenation
      (raw_string) @injection.content)
  ])
  (#eq? @_command "printf")
  (#eq? @_arg "-v")
  
  (#set! injection.include-children)
  (#set! injection.language "printf"))

; printf -- 'format'
((command
  name: (command_name) @_command
  argument: (word) @_arg
  .
  argument: [
    (string) @injection.content
    (concatenation
      (string) @injection.content)
    (raw_string) @injection.content
    (concatenation
      (raw_string) @injection.content)
  ])
  (#eq? @_command "printf")
  (#eq? @_arg "--")
  
  (#set! injection.include-children)
  (#set! injection.language "printf"))

((command
  name: (command_name) @_command
  .
  argument: [
    (string)
    (raw_string)
  ] @injection.content)
  (#eq? @_command "bind")
  
  (#set! injection.include-children)
  (#set! injection.language "readline"))

((command
  name: (command_name) @_command
  .
  argument: [
    (string)
    (raw_string)
  ] @injection.content)
  (#eq? @_command "trap")
  
  (#set! injection.include-children)
  (#set! injection.self))
