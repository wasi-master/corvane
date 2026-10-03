; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/javadoc/).
; Source: nvim-treesitter@728e031f6b11 queries/javadoc (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
; @value tags without double-quotes
((bare_format_string) @injection.content
  (#set! injection.language "printf"))

; @value tags with double quotes
((literal_format_string) @injection.content
  
  (#set! injection.language "printf"))

; injected code snippets
((snippet_tag
  (attributes
    (attribute
      name: (identifier) @_attribute_key
      value: (attribute_value
        [
          (identifier) @injection.language
          (string_literal
            (quoted_value) @injection.language)
        ])))
  body: (description) @injection.content)
  (#eq? @_attribute_key "lang"))

; html content
((description) @injection.content
  (#set! injection.language "html"))

; markdown content
((markdown_description) @injection.content
  (#set! injection.language "markdown_inline"))
