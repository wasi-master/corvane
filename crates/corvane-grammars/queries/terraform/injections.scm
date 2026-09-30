; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/terraform/).
; Source: nvim-treesitter@728e031f6b11 queries/terraform (Apache-2.0)
; inherited from hcl
((comment) @injection.content
  (#set! injection.language "comment"))

(heredoc_template
  (template_literal) @injection.content
  (heredoc_identifier) @injection.language)


