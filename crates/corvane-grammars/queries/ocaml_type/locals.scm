; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/ocaml_type/).
; Source: tree-sitter-ocaml 0.26.0 (MIT): queries/locals.scm
; Scopes
;-------

[
  (let_binding)
  (class_binding)
  (class_function)
  (method_definition)
  (fun_expression)
  (object_expression)
  (for_expression)
  (match_case)
  (attribute_payload)
] @local.scope

; Definitions
;------------

(value_pattern) @local.definition

; References
;-----------

(value_path . (value_name) @local.reference)
