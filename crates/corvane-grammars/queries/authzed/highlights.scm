; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/authzed/).
; Source: nvim-treesitter@728e031f6b11 queries/authzed (Apache-2.0)
(identifier) @function

(block
  (relation
    (relation_literal) @function.builtin
    (identifier) @constant))

(block
  (permission
    (permission_literal) @variable.builtin
    (identifier) @type))

; relations
(rel_expression
  (identifier) @property)

(relation
  (rel_expression
    (hash_literal)
    .
    (identifier) @constant))

; permissions
(perm_expression
  (identifier) @property)

(call_expression
  function: (selector_expression
    operand: (identifier) @constant
    field: (field_identifier) @function.method))

(perm_expression
  (stabby) @operator
  .
  (identifier) @function)

; misc
[
  (plus_literal)
  (minus_literal)
  (amp_literal)
  (pipe_literal)
] @operator

[
  (true)
  (false)
] @boolean

(nil) @constant.builtin

[
  (caveat_literal)
  (definition_literal)
] @keyword

[
  (hash_literal)
  (comment)
] @comment
