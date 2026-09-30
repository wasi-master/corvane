; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/wxml/).
; Source: nvim-treesitter@728e031f6b11 queries/wxml (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
; Comments
(comment) @comment @spell

; Text
(text) @spell

; Tag names
(tag_name) @tag

((tag_name) @tag.builtin
  (#any-of? @tag.builtin "wxs" "template" "import" "include" "slot" "block"))

; Attributes
(attribute_name) @tag.attribute

(attribute_value) @string

(quoted_attribute_value) @string

; WeChat specific attributes
((attribute_name) @keyword.directive
  (#match? @keyword.directive "^wx:"))

((attribute_name) @keyword.conditional
  (#any-of? @keyword.conditional "wx:if" "wx:elif" "wx:else"))

((attribute_name) @keyword.repeat
  (#any-of? @keyword.repeat "wx:for" "wx:for-index" "wx:for-item"))

((attribute_name) @keyword
  (#match? @keyword "^bind"))

((attribute_name) @keyword
  (#match? @keyword "^catch"))

((attribute_name) @keyword
  (#match? @keyword "^mut\\-bind"))

((attribute_name) @keyword
  (#match? @keyword "^model:"))

((attribute_name) @keyword
  (#match? @keyword "^data*?"))

((attribute
  (attribute_name) @_attr
  (quoted_attribute_value) @string.special.url)
  (#any-of? @_attr "href" "src")
  )

; Entity references
(entity) @character.special

; Interpolation delimiters
(interpolation_start) @punctuation.special

(interpolation_end) @punctuation.special

[
  "<"
  ">"
  "</"
  "/>"
] @tag.delimiter

"=" @operator
