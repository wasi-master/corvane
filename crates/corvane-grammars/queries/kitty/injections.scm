; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/kitty/).
; Source: nvim-treesitter@728e031f6b11 queries/kitty (Apache-2.0)
(launch_source_window
  value: (string) @injection.content
  (#set! injection.language "regex"))

(launch_next_to
  value: (string) @injection.content
  (#set! injection.language "regex"))

(marker_entry
  (pattern) @injection.content
  (#set! injection.language "regex"))

(color_match
  (pattern) @injection.content
  (#set! injection.language "regex"))

(color_match_tab
  (pattern) @injection.content
  (#set! injection.language "regex"))

(include
  glob: (pattern) @injection.content
  (#set! injection.language "regex"))

(filter_element
  (pattern) @injection.content
  (#set! injection.language "regex"))

(comment
  (comment_content) @injection.content
  (#set! injection.language "comment"))
