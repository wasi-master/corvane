; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/zig/).
; Source: nvim-treesitter@728e031f6b11 queries/zig (Apache-2.0)
((comment) @injection.content
  (#set! injection.language "comment"))

; TODO: add when asm is added
; (asm_output_item (string) @injection.content
;   (#set! injection.language "asm"))
; (asm_input_item (string) @injection.content
;   (#set! injection.language "asm"))
; (asm_clobbers (string) @injection.content
;   (#set! injection.language "asm"))
