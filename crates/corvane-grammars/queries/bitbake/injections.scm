; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/bitbake/).
; Source: nvim-treesitter@728e031f6b11 queries/bitbake (Apache-2.0)
(call
  function: (attribute
    object: (python_identifier) @_re)
  arguments: (argument_list
    (python_string
      (string_content) @injection.content) @_string)
  (#eq? @_re "re")
  (#match? @_string "^r.*")
  (#set! injection.language "regex"))

((shell_content) @injection.content
  (#set! injection.language "bash"))

((comment) @injection.content
  (#set! injection.language "comment"))
