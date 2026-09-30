; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/beancount/).
; Source: nvim-treesitter@728e031f6b11 queries/beancount (Apache-2.0)
(date) @variable.member

(txn) @attribute

(account) @type

(amount) @number

(incomplete_amount) @number

(compound_amount) @number

(amount_tolerance) @number

(currency) @property

(key) @label

(string) @string

(narration) @string @spell

(payee) @string @spell

(tag) @constant

(link) @constant

[
  (minus)
  (plus)
  (slash)
  (asterisk)
] @operator

(comment) @comment @spell

[
  (balance)
  (open)
  (close)
  (commodity)
  (pad)
  (event)
  (price)
  (note)
  (document)
  (query)
  (custom)
  (pushtag)
  (poptag)
  (pushmeta)
  (popmeta)
  (option)
  (include)
  (plugin)
] @keyword
