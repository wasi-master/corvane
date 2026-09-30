; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/typoscript/).
; Source: nvim-treesitter@728e031f6b11 queries/typoscript (Apache-2.0)
((multiline_line
  (identifier) @_id
  .
  (multiline_value
    (multiline_value_content) @injection.content))
  (#match? @_id "_CSS_DEFAULT_STYLE$")
  (#set! injection.language "css"))

((comment) @injection.content
  (#set! injection.language "comment"))

((single_line_comment) @injection.content
  (#set! injection.language "comment"))
