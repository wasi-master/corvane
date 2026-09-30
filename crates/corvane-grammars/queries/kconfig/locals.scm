; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/kconfig/).
; Source: nvim-treesitter@728e031f6b11 queries/kconfig (Apache-2.0)
[
  (symbol)
  (string)
] @local.reference

[
  (config)
  (menuconfig)
  (choice)
  (comment_entry)
  (menu)
  (if)
] @local.scope

(type_definition
  (string) @local.definition.var)

(type_definition
  (input_prompt
    (string) @local.definition.var))

(type_definition_default
  (expression
    (string) @local.definition.var))
