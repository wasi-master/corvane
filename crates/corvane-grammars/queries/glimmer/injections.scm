; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/glimmer/).
; Source: nvim-treesitter@728e031f6b11 queries/glimmer (Apache-2.0)
; comments
((comment_statement) @injection.content
  (#set! injection.language "comment"))

; <style> tags
((style_element
  (raw_text) @injection.content)
  (#set! injection.language "css"))

; <script> tags
((script_element
  (raw_text) @injection.content)
  (#set! injection.language "javascript")
  (#set! injection.include-children))
