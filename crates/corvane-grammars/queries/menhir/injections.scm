; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/menhir/).
; Source: nvim-treesitter@728e031f6b11 queries/menhir (Apache-2.0)
([
  (comment)
  (line_comment)
  (ocaml_comment)
] @injection.content
  (#set! injection.language "comment"))

((ocaml) @injection.content
  (#set! injection.language "ocaml"))
