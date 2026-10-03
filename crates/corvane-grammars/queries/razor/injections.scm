; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/razor/).
; Source: nvim-treesitter@728e031f6b11 queries/razor (Apache-2.0)
; inherited from c_sharp
((comment) @injection.content
  (#set! injection.language "comment"))



([
  (html_comment)
  (razor_comment)
] @injection.content
  (#set! injection.language "comment"))

((element) @injection.content
  (#set! injection.language "html")
  (#set! injection.combined))
