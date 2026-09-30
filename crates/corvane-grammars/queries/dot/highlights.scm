; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/dot/).
; Source: nvim-treesitter@728e031f6b11 queries/dot (Apache-2.0)
(identifier) @type

[
  "strict"
  "graph"
  "digraph"
  "subgraph"
  "node"
  "edge"
] @keyword

(string_literal) @string

(number_literal) @number

[
  (edgeop)
  (operator)
] @operator

[
  ","
  ";"
] @punctuation.delimiter

[
  "{"
  "}"
  "["
  "]"
  "<"
  ">"
] @punctuation.bracket

(subgraph
  id: (id
    (identifier) @module))

(attribute
  name: (id
    (identifier) @variable.member))

(attribute
  value: (id
    (identifier) @constant))

(comment) @comment @spell

(preproc) @keyword.directive
