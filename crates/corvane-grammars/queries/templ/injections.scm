; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/templ/).
; Source: nvim-treesitter@728e031f6b11 queries/templ (Apache-2.0)
; dropped 3 pattern(s) using raw_string_literal_content, interpreted_string_literal_content (not in the pinned grammar)
; inherited from go
((comment) @injection.content
  (#set! injection.language "comment"))

((comment) @injection.content
  (#match? @injection.content "/\\*!([a-zA-Z]+:)?re2c")
  (#set! injection.language "re2c"))

((element_comment) @injection.content
  (#set! injection.language "comment"))

((script_block_text) @injection.content
  (#set! injection.language "javascript"))

((script_element_text) @injection.content
  (#set! injection.language "javascript"))

((style_element_text) @injection.content
  (#set! injection.language "css"))
