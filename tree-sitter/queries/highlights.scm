; heading
(atx_heading
  (atx_h6_marker) @markup.heading.marker) @markup.heading.6

; code block
[
  (fenced_code_block)
  (indented_code_block)
] @markup.raw.block

(info_string) @label

(fenced_code_block_delimiter) @punctuation.bracket

; link
(link_destination) @markup.link.url

(link_label) @markup.link.label

; list
[
  (list_marker_minus)
  (list_marker_plus)
  (list_marker_star)
] @markup.list.unnumbered

[
  (list_marker_dot)
  (list_marker_parenthesis)
] @markup.list.numbered

(task_list_marker_checked) @markup.list.checked

(task_list_marker_unchecked) @markup.list.unchecked

; thematic break
(thematic_break) @punctuation.special

; quote
[
  (block_continuation)
  (block_quote_marker)
] @punctuation.special

(backslash_escape) @string.escape

(block_quote) @markup.quote
