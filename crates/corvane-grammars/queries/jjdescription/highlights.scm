; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/jjdescription/).
; Source: nvim-treesitter@728e031f6b11 queries/jjdescription (Apache-2.0)
[
  (comment)
  (generated_comment)
] @comment

(comment_content) @spell

(subject) @markup.heading

(type) @keyword

(scope) @variable.parameter

(change_id) @constant

(filepath) @string.special.path

((rest) @comment
  (#not-match? @comment "^diff"))

"JJ: ignore-rest" @keyword.directive

[
  "("
  ")"
] @punctuation.bracket

":" @punctuation.delimiter

"!" @punctuation.special

[
  "A"
  "C"
] @diff.plus

"D" @diff.minus

[
  "M"
  "R"
] @diff.delta
