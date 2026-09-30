; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/t32/).
; Source: nvim-treesitter@728e031f6b11 queries/t32 (Apache-2.0)
(block) @local.scope

; Parameter definitions
(parameter_declaration
  command: (identifier)
  macro: (macro) @local.definition.parameter)

; Variable definitions
(macro_definition
  command: (identifier)
  macro: (macro) @local.definition.var)

(command_expression
  command: (identifier)
  arguments: (argument_list
    declarator: (symbol) @local.definition.var))

; Function definitions
(subroutine_block
  command: (identifier)
  subroutine: (identifier) @local.definition.function)

(labeled_expression
  label: (identifier) @local.definition.function
  (block))

; References
((subroutine_call_expression
  command: (identifier)
  subroutine: (identifier) @local.reference)
  (#set! reference.kind "function"))

[
  (macro)
  (symbol)
] @local.reference
