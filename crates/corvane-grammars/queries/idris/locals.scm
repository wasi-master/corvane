; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/idris/).
; Source: nvim-treesitter@728e031f6b11 queries/idris (Apache-2.0)
(signature
  name: (loname)) @local.definition.var

(signature
  name: (caname)) @local.definition.type

(function
  (lhs
    (funvar
      subject: (loname)))) @local.definition.function

(function
  (lhs
    (funvar
      subject: (caname)))) @local.definition.function

(type_var
  (loname)) @local.definition.type

(pat_name
  (loname)) @local.definition.var

(pat_name
  (caname)) @local.definition.var

(exp_name
  (loname)) @local.reference

(exp_name
  (caname)) @local.reference

(function
  (rhs) @local.scope)
