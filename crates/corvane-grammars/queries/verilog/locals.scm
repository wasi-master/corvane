; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/verilog/).
; Source: helix@ba40e547426b queries/verilog (MPL-2.0)
; too broad, now includes types etc
(simple_identifier) @local.reference
(tf_port_item1
 (port_identifier
  (simple_identifier) @local.definition.variable.parameter))

;; TODO: fixme
;(function_declaration
 ;(function_identifier
  ;(simple_identifier) @local.definition.function))

(function_declaration
 (function_body_declaration
  (function_identifier
   (function_identifier
    (simple_identifier) @local.definition.function))))

(local_parameter_declaration
 (list_of_param_assignments
  (param_assignment
   (parameter_identifier
    (simple_identifier) @local.definition.variable.parameter))))

(parameter_declaration
 (list_of_param_assignments
  (param_assignment
   (parameter_identifier
    (simple_identifier) @local.definition.variable.parameter))))

[
  (loop_generate_construct)
  (loop_statement)
  (conditional_statement)
  (case_item)
  (function_declaration)
  (always_construct)
  (module_declaration)
] @local.scope

