; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/bass/).
; Source: nvim-treesitter@728e031f6b11 queries/bass (Apache-2.0)
; Scopes
[
  (list)
  (scope)
  (cons)
] @local.scope

; References
(symbol) @local.reference

; Definitions
((list
  .
  (symbol) @_fnkw
  .
  (symbol) @local.definition.function
  (symbol)? @local.definition.parameter)
  (#any-of? @_fnkw "def" "defop" "defn" "fn"))

((cons
  .
  (symbol) @_fnkw
  .
  (symbol) @local.definition.function
  (symbol)? @local.definition.parameter)
  (#any-of? @_fnkw "def" "defop" "defn" "fn"))
