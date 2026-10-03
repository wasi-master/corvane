; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/superhtml/).
; Source: nvim-treesitter@728e031f6b11 queries/superhtml (Apache-2.0)
((script_element
  (raw_text) @injection.content)
  (#set! injection.language "javascript"))

((style_element
  (raw_text) @injection.content)
  (#set! injection.language "css"))
