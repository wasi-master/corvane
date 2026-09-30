; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/m68k/).
; Source: nvim-treesitter@728e031f6b11 queries/m68k (Apache-2.0)
(macro_definition
  name: (symbol) @local.definition.macro)

(symbol_assignment
  name: (symbol) @local.definition.var)

(label
  name: (symbol) @local.definition.constant)

(symbol_definition
  name: (symbol) @local.definition.constant)

(offset_definition
  name: (symbol) @local.definition.constant)

(register_definition
  name: (symbol) @local.definition.constant)

(register_list_definition
  name: (symbol) @local.definition.constant)

(external_reference
  symbols: (symbol_list
    (symbol) @local.definition.import))

(symbol) @local.reference
