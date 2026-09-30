; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/sparql/).
; Source: nvim-treesitter@728e031f6b11 queries/sparql (Apache-2.0)
(group_graph_pattern
  (triples_block) @local.scope)

((sub_select
  (select_clause
    (var) @local.definition.var))
  (#set! definition.var.scope "parent"))

((select_query
  (select_clause
    (var) @local.definition.var))
  (#set! definition.var.scope "parent"))

(var) @local.reference
