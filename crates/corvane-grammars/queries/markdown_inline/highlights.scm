; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/markdown_inline/).
; Source: nvim-treesitter@728e031f6b11 queries/markdown_inline (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
; From MDeiml/tree-sitter-markdown
(code_span) @markup.raw @nospell

(emphasis) @markup.italic

(strong_emphasis) @markup.strong

(strikethrough) @markup.strikethrough

(shortcut_link
  (link_text) @nospell)

; Conceal backslash in backslash escapes
((backslash_escape) @conceal
  
  (#set! conceal ""))

; Conceal backslash in hard line breaks
((hard_line_break
  "\\" @conceal)
  (#set! conceal ""))

; Conceal codeblock and text style markers
([
  (code_span_delimiter)
  (emphasis_delimiter)
] @conceal
  (#set! conceal ""))

; Conceal inline links
(inline_link
  [
    "["
    "]"
    "("
    (link_destination)
    ")"
  ] @markup.link
  (#set! conceal ""))

[
  (link_label)
  (link_text)
  (link_title)
  (image_description)
] @markup.link.label

((inline_link
  (link_destination) @_url) @_label
  )

((image
  (link_destination) @_url) @_label
  )

; Conceal image links
(image
  [
    "!"
    "["
    "]"
    "("
    (link_destination)
    ")"
  ] @markup.link
  (#set! conceal ""))

; Conceal full reference links
(full_reference_link
  [
    "["
    "]"
    (link_label)
  ] @markup.link
  (#set! conceal ""))

; Conceal collapsed reference links
(collapsed_reference_link
  [
    "["
    "]"
  ] @markup.link
  (#set! conceal ""))

; Conceal shortcut links
(shortcut_link
  [
    "["
    "]"
  ] @markup.link
  (#set! conceal ""))

[
  (link_destination)
  (uri_autolink)
  (email_autolink)
] @markup.link.url @nospell

((uri_autolink) @_url
  
  )

(entity_reference) @nospell

; Replace common HTML entities.
((entity_reference) @character.special
  (#eq? @character.special "&nbsp;")
  (#set! conceal " "))

((entity_reference) @character.special
  (#eq? @character.special "&lt;")
  (#set! conceal "<"))

((entity_reference) @character.special
  (#eq? @character.special "&gt;")
  (#set! conceal ">"))

((entity_reference) @character.special
  (#eq? @character.special "&amp;")
  (#set! conceal "&"))

((entity_reference) @character.special
  (#eq? @character.special "&quot;")
  (#set! conceal "\""))

((entity_reference) @character.special
  (#any-of? @character.special "&ensp;" "&emsp;")
  (#set! conceal " "))
