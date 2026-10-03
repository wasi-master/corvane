; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/udev/).
; Source: nvim-treesitter@728e031f6b11 queries/udev (Apache-2.0)
; labels
(assignment
  key: "LABEL"
  (value
    (content) @local.definition))

(assignment
  key: "GOTO"
  (value
    (content) @local.reference))

; env vars
(assignment
  key: "ENV"
  (env_var) @local.definition.var)

(match
  key: "ENV"
  (env_var) @local.reference)

(var_sub
  (env_var) @local.reference)

; misc
[
  (attribute)
  (kernel_param)
  (seclabel)
] @local.reference
