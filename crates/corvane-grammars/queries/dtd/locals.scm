; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/dtd/).
; Source: nvim-treesitter@728e031f6b11 queries/dtd (Apache-2.0)
(elementdecl
  (Name) @local.definition.type)

(elementdecl
  (contentspec
    (children
      (Name) @local.reference)))

(AttlistDecl
  .
  (Name) @local.reference)
