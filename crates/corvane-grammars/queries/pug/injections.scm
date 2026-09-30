; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/pug/).
; Source: nvim-treesitter@728e031f6b11 queries/pug (Apache-2.0)
((comment) @injection.content
  (#set! injection.language "comment"))

((javascript) @injection.content
  (#set! injection.language "javascript"))

((attribute_name) @_attribute_name
  (quoted_attribute_value
    (attribute_value) @injection.content
    (#set! injection.language "javascript"))
  (#match? @_attribute_name "^(:|v-bind|v-|\\@)"))
