; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/udev/).
; Source: nvim-treesitter@728e031f6b11 queries/udev (Apache-2.0)
((comment) @injection.content
  (#set! injection.language "comment"))

((match
  key: "PROGRAM"
  (value
    (content) @injection.content))
  (#set! injection.language "bash"))

((assignment
  key: "RUN"
  (value
    (content) @injection.content))
  (#set! injection.language "bash"))
