; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/xml/).
; Source: nvim-treesitter@728e031f6b11 queries/xml (Apache-2.0)
; tags
(elementdecl
  (Name) @local.definition.type)

(elementdecl
  (contentspec
    (children
      (Name) @local.reference)))

(AttlistDecl
  .
  (Name) @local.reference)

(STag
  (Name) @local.reference)

(ETag
  (Name) @local.reference)

(EmptyElemTag
  (Name) @local.reference)

; attributes
(AttDef
  (Name) @local.definition.field)

(Attribute
  (Name) @local.reference)

; entities
(GEDecl
  (Name) @local.definition.macro)

(EntityRef
  (Name) @local.reference)
