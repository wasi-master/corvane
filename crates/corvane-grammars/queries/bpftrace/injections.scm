; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/bpftrace/).
; Source: nvim-treesitter@728e031f6b11 queries/bpftrace (Apache-2.0)
([
  (c_struct)
  (c_preproc)
  (c_preproc_block)
] @injection.content
  (#set! injection.language "c"))

([
  (line_comment)
  (block_comment)
] @injection.content
  (#set! injection.language "comment"))
